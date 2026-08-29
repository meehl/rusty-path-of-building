use crate::{pob::PathOfBuilding, pob::api};
use anyhow::{Result, anyhow};
use mlua::{Integer, IntoLuaMulti, Lua, MultiValue, Number, Value};
use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, channel},
    thread::JoinHandle,
};

/// The outcome of a [`Subscript`] that has finished.
#[derive(Debug)]
pub enum SubscriptResult {
    SubscriptFinished {
        id: u64,
        return_values: NativeMultiValue,
    },
    SubscriptError {
        id: u64,
        error: String,
    },
}

#[derive(Default)]
pub struct SubscriptManager {
    next_id: u64,
    scripts: Vec<Subscript>,
}

impl SubscriptManager {
    /// Starts a new subscript running `script_text` on its own thread and returns the id it was
    /// assigned.
    pub fn push(
        &mut self,
        script_dir: PathBuf,
        script_text: String,
        blocking_calls: Vec<String>,
        nonblocking_calls: Vec<String>,
        arguments: NativeMultiValue,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let subscript = Subscript::new(
            id,
            script_text,
            blocking_calls,
            nonblocking_calls,
            arguments,
            script_dir,
        );
        self.scripts.push(subscript);
        id
    }

    /// Advances all running subscripts by one step.
    pub fn process(&mut self, lua: &Lua) -> Vec<SubscriptResult> {
        let mut results = vec![];

        self.scripts.retain_mut(|subscript| {
            subscript.handle_calls(lua);

            subscript.try_join().is_none_or(|event| {
                results.push(event);
                // subscript has finished or errored, remove it
                false
            })
        });

        results
    }

    /// Returns `true` if at least one subscript is running.
    pub fn has_running_subscripts(&self) -> bool {
        !self.scripts.is_empty()
    }

    /// Returns `true` if the subscript with the given id is running.
    pub fn is_running(&self, subscript_id: u64) -> bool {
        self.scripts
            .iter()
            .any(|subscript| subscript.id == subscript_id)
    }
}

/// A request sent from a subscript's thread to the main thread, asking it to invoke a function
/// defined in the main Lua instance on the subscript's behalf.
enum SubscriptCall {
    Blocking {
        function_name: String,
        arguments: NativeMultiValue,
        // used to send return values of function back to thread
        reply_tx: Sender<Result<NativeMultiValue>>,
    },
    NonBlocking {
        function_name: String,
        arguments: NativeMultiValue,
    },
}

/// A Lua script running on its own thread, in a separate `Lua` instance.
pub struct Subscript {
    id: u64,
    handle: Option<JoinHandle<anyhow::Result<NativeMultiValue>>>,
    /// Receives `SubscriptCall`s sent by the proxy functions running in the subscript's s thread.
    call_rx: Receiver<SubscriptCall>,
}

// Subscripts are lua scripts that are executed in their own instance on a separate thread.
//
// When a subscript needs to call a function defined in the main instance, a `SubscriptCall`
// message is send over a channel. At the beginning of each frame, the main thread checks for
// messages and executes the requested function with the provided arguments on behalf of the
// subscript.  For `BlockingCall`, the subscript waits for the main thread to send the return
// values of the function back over another channel.  For `NonBlockingCall`, the subscript doesn't
// wait on any return values and keeps executing the script after sending the message.  Subscripts
// are required to explicitly specify the names of all (non)-blocking function calls that appear in
// the script.
impl Subscript {
    /// Spawns a new thread, creates a fresh `Lua` instance on it, registers proxy functions for
    /// `blocking_calls` and `nonblocking_calls`, and starts running `script_text` with
    /// `arguments`.
    pub fn new(
        id: u64,
        script_text: String,
        blocking_calls: Vec<String>,
        nonblocking_calls: Vec<String>,
        arguments: NativeMultiValue,
        script_dir: PathBuf,
    ) -> Self {
        let (call_tx, call_rx) = channel();

        let handle = std::thread::spawn(move || {
            profiling::register_thread!(format!("Subscript {id} Thread"));

            // unsafe required to load C modules (curl)
            let lua = unsafe { Lua::unsafe_new() };

            // add ./lua to package.path and package.cpath
            // TODO: this is awkward, move package path registration out of PoB?
            PathOfBuilding::register_package_paths(&lua, &script_dir)?;

            // register a proxy for each allowed blocking call.
            // forwards the call to the main thread and blocks on `reply_rx` until the main thread
            // sends back the return values
            for function_name in blocking_calls {
                let call_tx = call_tx.clone();
                lua.globals().set(
                    function_name.clone(),
                    lua.create_function(move |_, args: MultiValue| {
                        let (reply_tx, reply_rx) = channel();
                        call_tx
                            .send(SubscriptCall::Blocking {
                                function_name: function_name.clone(),
                                arguments: args.try_into()?,
                                reply_tx,
                            })
                            .unwrap();
                        let return_values = reply_rx.recv().map_err(|e| anyhow!("{e}"))??;
                        Ok(return_values)
                    })?,
                )?;
            }

            // register a proxy for each allowed non-blocking call.
            // forwards the call to the main thread and returns immediately
            for function_name in nonblocking_calls {
                let call_tx = call_tx.clone();
                lua.globals().set(
                    function_name.clone(),
                    lua.create_function(move |_, args: MultiValue| {
                        call_tx
                            .send(SubscriptCall::NonBlocking {
                                function_name: function_name.clone(),
                                arguments: args.try_into()?,
                            })
                            .map_err(|e| anyhow!("{e}"))?;
                        Ok(())
                    })?,
                )?;
            }

            let result = lua.load(script_text).call::<MultiValue>(arguments)?;
            result.try_into()
        });

        Self {
            id,
            handle: Some(handle),
            call_rx,
        }
    }

    /// Handles pending `SubscriptCall`s.
    ///
    /// Does nothing if there is no pending call.
    fn handle_calls(&self, lua: &Lua) {
        let call = match self.call_rx.try_recv() {
            Ok(call) => call,
            // no pending call, or thread disconnected which is handled on join
            Err(_) => return,
        };

        match call {
            SubscriptCall::Blocking {
                function_name,
                arguments,
                reply_tx: return_values_sender,
            } => {
                let result = api::on_sub_call(lua, function_name, arguments)
                    .map_err(anyhow::Error::from)
                    .and_then(NativeMultiValue::try_from);
                // if the subscript's thread has already gone away, there's nowhere to send the
                // result, so ignore the error
                let _ = return_values_sender.send(result);
            }
            SubscriptCall::NonBlocking {
                function_name,
                arguments,
            } => {
                // return values and errors of non-blocking calls are discarded
                let _ = api::on_sub_call(lua, function_name, arguments);
            }
        }
    }

    /// If the subscript's thread has finished, joins it and returns the resulting
    /// `SubscriptResult`.
    fn try_join(&mut self) -> Option<SubscriptResult> {
        if !self.handle.as_ref().is_some_and(JoinHandle::is_finished) {
            return None;
        }

        let event = match self.handle.take().unwrap().join() {
            Ok(Ok(return_values)) => SubscriptResult::SubscriptFinished {
                id: self.id,
                return_values,
            },
            Ok(Err(err)) => SubscriptResult::SubscriptError {
                id: self.id,
                error: err.to_string(),
            },
            // the thread panicked
            Err(_) => SubscriptResult::SubscriptError {
                id: self.id,
                error: String::from("Subscript thread panicked!"),
            },
        };
        Some(event)
    }
}

/// Instance-independent stand-in for `mlua::Value`.
///
/// Values produced by one Lua instance (in particular strings) cannot be used directly with
/// another Lua instance. `NativeValue` acts as a stand-in to freely move between the main instance
/// and a subscript's instance. It only covers the types that are actually exchanged, namely `nil`,
/// booleans, numbers, and string.
#[derive(Debug)]
pub enum NativeValue {
    Nil,
    Number(Number),
    Integer(Integer),
    Boolean(bool),
    // Lua strings may not be valid UTF-8, so store as raw bytes instead of String
    String(Vec<u8>),
}

/// Instance-independent stand-in for `mlua::MultiValue`.
///
/// Used to pass function arguments and return values between the main Lua instance and a
/// subscript's Lua instance.
#[derive(Debug)]
pub struct NativeMultiValue(Vec<NativeValue>);

impl TryFrom<Value> for NativeValue {
    type Error = anyhow::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Nil => Ok(Self::Nil),
            Value::Boolean(b) => Ok(Self::Boolean(b)),
            Value::Integer(i) => Ok(Self::Integer(i)),
            Value::Number(n) => Ok(Self::Number(n)),
            Value::String(s) => Ok(Self::String(s.as_bytes().to_vec())),
            other => Err(anyhow!("Unsupported value type: {}", other.type_name())),
        }
    }
}

impl TryFrom<MultiValue> for NativeMultiValue {
    type Error = anyhow::Error;

    fn try_from(values: MultiValue) -> Result<Self, Self::Error> {
        values
            .into_iter()
            .map(NativeValue::try_from)
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

impl IntoLuaMulti for NativeMultiValue {
    fn into_lua_multi(self, lua: &Lua) -> mlua::Result<MultiValue> {
        self.0
            .into_iter()
            .map(|v| match v {
                NativeValue::Nil => Ok(Value::Nil),
                NativeValue::Boolean(b) => Ok(Value::Boolean(b)),
                NativeValue::Integer(i) => Ok(Value::Integer(i)),
                NativeValue::Number(n) => Ok(Value::Number(n)),
                NativeValue::String(s) => lua.create_string(s).map(Value::String),
            })
            .collect::<mlua::Result<Vec<_>>>()
            .map(MultiValue::from_vec)
    }
}
