use std::{cell::RefCell, rc::Rc};

use clap::Parser;
use mlua::{Lua, Result as LuaResult, Table};

use crate::{
    Game,
    api::{self, get_callback},
    args::Args,
    subscript::{NativeMultiValue, SubscriptManager, SubscriptResult, register_subscript_globals},
    util::change_working_directory,
};

pub struct LuaInstance {
    lua: Lua,
    subscript_manager: Rc<RefCell<SubscriptManager>>,
}

impl LuaInstance {
    pub fn new() -> anyhow::Result<Self> {
        let subscript_manager = Rc::new(RefCell::new(SubscriptManager::new()));

        let lua = Self::create_lua_state()?;
        register_subscript_globals(&lua, &subscript_manager)?;

        let instance = Self {
            lua,
            subscript_manager,
        };

        instance.init()?;

        Ok(instance)
    }

    fn create_lua_state() -> LuaResult<Lua> {
        // SAFETY: use `unsafe_new` to allow loading of C modules
        let lua = unsafe { Lua::unsafe_new() };

        // expose build arg to lua
        let args = Args::parse();
        let args_table = lua.create_sequence_from(std::iter::once(args.build_path))?;
        lua.globals().set("arg", args_table)?;

        // add ./lua to package.path and package.cpath
        Self::register_package_paths(&lua)?;

        // register callbacks
        api::register_globals(&lua)?;

        Ok(lua)
    }

    pub fn init(&self) -> LuaResult<()> {
        let script_dir = Game::script_dir();
        change_working_directory(script_dir.as_path())?;
        self.load(script_dir.join("Launch.lua")).exec()?;
        self.on_init()?;
        Ok(())
    }

    pub fn restart(&mut self) -> LuaResult<()> {
        self.lua = Self::create_lua_state()?;
        register_subscript_globals(&self.lua, &self.subscript_manager)?;
        self.init()?;

        Ok(())
    }

    /// Run functions for subscripts and handle their completion/failure.
    pub fn handle_subscripts(&self) {
        let subscript_events = self.subscript_manager.borrow_mut().process(self);

        // Handle finished/errored subscripts.
        for event in subscript_events {
            match event {
                SubscriptResult::SubscriptFinished { id, return_values } => {
                    self.on_sub_finished(id, return_values).unwrap();
                }
                SubscriptResult::SubscriptError { id, error } => {
                    self.on_sub_error(id, &error).unwrap();
                }
            }
        }
    }

    fn on_init(&self) -> LuaResult<()> {
        let on_init = get_callback(&self.lua, "OnInit")?;
        on_init.call::<()>(())
    }

    pub fn on_frame(&self) -> LuaResult<()> {
        profiling::scope!("on_frame");

        let on_frame = get_callback(&self.lua, "OnFrame")?;
        on_frame.call::<()>(())
    }

    pub fn on_char(&self, ch: &str) -> LuaResult<()> {
        let on_char = get_callback(&self.lua, "OnChar")?;
        on_char.call::<()>(ch)
    }

    pub fn on_key_down(&self, key: &str, is_double_click: bool) -> LuaResult<()> {
        let on_key_down = get_callback(&self.lua, "OnKeyDown")?;
        on_key_down.call::<()>((key, is_double_click))
    }

    pub fn on_key_up(&self, key: &str) -> LuaResult<()> {
        let on_key_up = get_callback(&self.lua, "OnKeyUp")?;
        on_key_up.call::<()>(key)
    }

    pub fn on_sub_finished(&self, id: u64, return_values: NativeMultiValue) -> LuaResult<()> {
        let on_sub_finished = get_callback(&self.lua, "OnSubFinished")?;
        on_sub_finished.call::<()>((id, return_values))
    }

    pub fn on_sub_error(&self, id: u64, error: &str) -> LuaResult<()> {
        let on_sub_error = get_callback(&self.lua, "OnSubError")?;
        on_sub_error.call::<()>((id, error))
    }

    pub fn on_exit(&self) -> LuaResult<()> {
        let on_exit = get_callback(&self.lua, "OnExit")?;
        on_exit.call::<()>(())
    }

    pub fn register_package_paths(lua: &Lua) -> LuaResult<()> {
        let script_dir = Game::script_dir();
        let package: Table = lua.globals().get("package")?;
        let mut package_path: String = package.get("path")?;
        package_path.push(';');
        package_path.push_str(script_dir.join("lua/?.lua").to_str().unwrap());
        package_path.push(';');
        package_path.push_str(script_dir.join("lua/?/init.lua").to_str().unwrap());
        package.set("path", package_path)?;

        Ok(())
    }
}

impl std::ops::Deref for LuaInstance {
    type Target = Lua;
    fn deref(&self) -> &Self::Target {
        &self.lua
    }
}
