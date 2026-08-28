use crate::{pob::PathOfBuilding, pob::api::get_callback};
use anyhow::{Result, anyhow};
use mlua::{Function, Integer, IntoLuaMulti, Lua, MultiValue, Number, Value};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, TryRecvError, channel},
    thread::JoinHandle,
};

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
    current_id: u64,
    scripts: Vec<Subscript>,
}

impl SubscriptManager {
    pub fn push(
        &mut self,
        script_dir: PathBuf,
        script_text: String,
        blocking_calls: Vec<String>,
        nonblocking_calls: Vec<String>,
        arguments: NativeMultiValue,
    ) -> u64 {
        let id = self.current_id;
        self.current_id += 1;

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

    pub fn has_running_subscripts(&self) -> bool {
        !self.scripts.is_empty()
    }

    pub fn is_running(&self, subscript_id: u64) -> bool {
        self.scripts.iter().any(|ss| ss.id == subscript_id)
    }
}

enum SubscriptCall {
    Blocking {
        function_name: String,
        arguments: NativeMultiValue,
        // used to send return values of function back to thread
        return_values_sender: Sender<Result<NativeMultiValue>>,
    },
    NonBlocking {
        function_name: String,
        arguments: NativeMultiValue,
    },
}

pub struct Subscript {
    id: u64,
    handle: Option<JoinHandle<anyhow::Result<NativeMultiValue>>>,
    receiver: Receiver<SubscriptCall>,
}

// Subscripts are lua scripts that are executed in their own instance on a separate
// thread.
//
// When a subscript needs to call a function defined in the main instance, a
// `SubscriptCall` message is send over a channel. At the beginning of each frame,
// the main thread checks for messages and executes the requested function with the
// provided arguments on behalf of the subscript.
// For `BlockingCall`, the subscript waits for the main thread to send the return
// values of the function back over another channel.
// For `NonBlockingCall`, the subscript doesn't wait on any return values and keeps
// executing the script after sending the message.
// Subscripts are required to explicitly specify the names of all (non)-blocking
// function calls that appear in the script.
impl Subscript {
    pub fn new(
        id: u64,
        script_text: String,
        blocking_calls: Vec<String>,
        nonblocking_calls: Vec<String>,
        arguments: NativeMultiValue,
        script_dir: PathBuf,
    ) -> Self {
        let (tx, rx) = channel();

        let handle = std::thread::spawn(move || {
            profiling::register_thread!(format!("Subscript {} Thread", id));

            // unsafe required to load C modules (curl)
            let lua = unsafe { Lua::unsafe_new() };

            // add ./lua to package.path and package.cpath
            // TODO: this is awkward, move package path registration out of PoB?
            PathOfBuilding::register_package_paths(&lua, &script_dir)?;

            for function_name in blocking_calls {
                let thread_tx = tx.clone();
                lua.globals().set(
                    function_name.clone(),
                    lua.create_function(move |_, args: MultiValue| {
                        let (tx_return, rx_return) = channel();
                        thread_tx
                            .send(SubscriptCall::Blocking {
                                function_name: function_name.clone(),
                                arguments: args.try_into()?,
                                return_values_sender: tx_return,
                            })
                            .unwrap();
                        // this blocks until we receive return values
                        let return_values = rx_return.recv().map_err(|e| anyhow!("{e}"))??;
                        Ok(return_values)
                    })?,
                )?;
            }

            for function_name in nonblocking_calls {
                let thread_tx = tx.clone();
                lua.globals().set(
                    function_name.clone(),
                    lua.create_function(move |_, args: MultiValue| {
                        thread_tx
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
            receiver: rx,
        }
    }

    fn handle_calls(&self, lua: &Lua) {
        match self.receiver.try_recv() {
            Ok(SubscriptCall::Blocking {
                function_name,
                arguments,
                return_values_sender,
            }) => {
                let func: Result<Function, _> = get_callback(lua, "OnSubCall");
                match func {
                    Ok(func) => {
                        match func.call::<MultiValue>((function_name, arguments)) {
                            Ok(return_values) => {
                                // send return values back to thread
                                let _ = return_values_sender.send(return_values.try_into());
                            }
                            // function returned error, forward it to thread
                            Err(err) => {
                                let _ = return_values_sender.send(Err(err.into()));
                            }
                        }
                    }
                    // function not found
                    Err(err) => {
                        let _ = return_values_sender.send(Err(err.into()));
                    }
                }
            }
            Ok(SubscriptCall::NonBlocking {
                function_name,
                arguments,
            }) => {
                let func: Result<Function, _> = get_callback(lua, "OnSubCall");
                if let Ok(func) = func {
                    // we can ignore return values for non-blocking calls
                    let _ = func.call::<()>((function_name, arguments));
                }
            }
            // ignore disconnects. potential errors are handled during thread join
            Err(TryRecvError::Disconnected) => {}
            // no outstanding calls from thread
            Err(TryRecvError::Empty) => {}
        }
    }

    fn try_join(&mut self) -> Option<SubscriptResult> {
        if self.handle.as_ref().is_some_and(JoinHandle::is_finished) {
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
        } else {
            None
        }
    }
}

// used to move arguments and return values between lua instances
// some lua values are associated with their instance and using them with another
// instance is not allowed.
#[derive(Debug)]
pub struct NativeMultiValue(VecDeque<NativeValue>);

#[derive(Debug)]
pub enum NativeValue {
    Nil,
    Number(Number),
    Integer(Integer),
    Boolean(bool),
    // Lua strings may not be valid UTF-8, so use Vec<u8> instead of String
    String(Vec<u8>),
}

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
