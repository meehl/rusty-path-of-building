use crate::{api::get_callback, lua::LuaInstance};
use anyhow::{Result, anyhow};
use mlua::{Function, Integer, IntoLuaMulti, Lua, MultiValue, Number, Result as LuaResult, Value};
use std::{
    cell::RefCell,
    collections::VecDeque,
    path::PathBuf,
    rc::Rc,
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

pub struct SubscriptManager {
    current_id: u64,
    scripts: Vec<Subscript>,
    script_dir: PathBuf,
}

impl SubscriptManager {
    pub fn new(script_dir: PathBuf) -> Self {
        Self {
            current_id: 0,
            scripts: Vec::new(),
            script_dir,
        }
    }

    pub fn push(
        &mut self,
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
            self.script_dir.clone(),
        );
        self.scripts.push(subscript);
        id
    }

    pub fn process(&mut self, lua: &LuaInstance) -> Vec<SubscriptResult> {
        let mut results = vec![];

        self.scripts.retain_mut(|subscript| {
            subscript.handle_calls(lua);

            if let Some(event) = subscript.try_join() {
                results.push(event);
                // subscript has finished or errored, remove it
                false
            } else {
                // subscript has not finished yet, keep it
                true
            }
        });

        results
    }

    pub fn has_running_subscripts(&self) -> bool {
        !self.scripts.is_empty()
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
            LuaInstance::register_package_paths(&lua, &script_dir)?;

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
                        let return_values = rx_return.recv().map_err(|e| anyhow!("{}", e))??;
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
                            .map_err(|e| anyhow!("{}", e))?;
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

    fn handle_calls(&mut self, lua: &Lua) {
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
        if self
            .handle
            .as_ref()
            .map(|h| h.is_finished())
            .unwrap_or(false)
        {
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

pub fn register_subscript_globals(
    lua: &Lua,
    subscripts: &Rc<RefCell<SubscriptManager>>,
) -> LuaResult<()> {
    let globals = lua.globals();

    // ssID = LaunchSubScript("<scriptText>", "<funcList>", "<subList>"[, ...])
    let subscripts_clone = Rc::clone(subscripts);
    let launch_sub_script = move |_: &Lua,
                                  (script_text, func_list, sub_list, args): (
        String,
        String,
        String,
        MultiValue,
    )| {
        let blocking_calls = func_list
            .split(',')
            .map(|s| s.trim())
            .filter(|&s| !s.is_empty())
            .map(String::from)
            .collect();

        let nonblocking_calls = sub_list
            .split(',')
            .map(|s| s.trim())
            .filter(|&s| !s.is_empty())
            .map(String::from)
            .collect();

        let arguments = args.try_into()?;
        let subscript_id = subscripts_clone.borrow_mut().push(
            script_text,
            blocking_calls,
            nonblocking_calls,
            arguments,
        );
        Ok(subscript_id)
    };

    let subscripts_clone = Rc::clone(subscripts);
    let is_subscript_running = move |_: &Lua, subscript_id: u64| {
        Ok(subscripts_clone
            .borrow()
            .scripts
            .iter()
            .any(|ss| ss.id == subscript_id))
    };

    let abort_subscript = |_: &Lua, _subscript_id: u64| -> LuaResult<()> { unimplemented!() };

    globals.set(
        "LaunchSubScript",
        lua.create_function_mut(launch_sub_script)?,
    )?;
    globals.set(
        "IsSubScriptRunning",
        lua.create_function(is_subscript_running)?,
    )?;
    globals.set("AbortSubScript", lua.create_function(abort_subscript)?)?;
    Ok(())
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
    // Lua strings may not be valid UTF-8, so store Vec<u8> instead
    String(Vec<u8>),
}

impl TryFrom<MultiValue> for NativeMultiValue {
    type Error = anyhow::Error;

    fn try_from(values: MultiValue) -> Result<Self, Self::Error> {
        let native = values
            .iter()
            .map(|v| match v.type_name() {
                "nil" => Ok(NativeValue::Nil),
                "boolean" => Ok(NativeValue::Boolean(v.as_boolean().unwrap())),
                "number" => Ok(NativeValue::Number(v.as_number().unwrap())),
                "integer" => Ok(NativeValue::Integer(v.as_integer().unwrap())),
                "string" => Ok(NativeValue::String(
                    v.as_string().unwrap().as_bytes().to_vec(),
                )),
                _ => Err(anyhow!("Unsupported value type")),
            })
            .collect::<Result<VecDeque<_>, _>>()?;

        Ok(NativeMultiValue(native))
    }
}

impl IntoLuaMulti for NativeMultiValue {
    fn into_lua_multi(self, lua: &Lua) -> mlua::Result<MultiValue> {
        let values = self
            .0
            .iter()
            .map(|v| match v {
                NativeValue::Nil => Ok(Value::Nil),
                NativeValue::Boolean(b) => Ok(Value::Boolean(*b)),
                NativeValue::Number(n) => Ok(Value::Number(*n)),
                NativeValue::Integer(n) => Ok(Value::Integer(*n)),
                NativeValue::String(s) => lua.create_string(s).map(Value::String),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MultiValue::from_vec(values))
    }
}
