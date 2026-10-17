#[cfg(feature = "bundled-lua-modules")]
pub mod luacurl;
#[cfg(feature = "bundled-lua-modules")]
pub mod luasocket;

use mlua::{Lua, Result as LuaResult};

/// Registers Rust implementations of the Lua modules required by PoB's Lua code.
pub fn register(lua: &Lua) -> LuaResult<()> {
    lua.register_module("lua-utf8", luautf8::create_module(lua)?)?;
    lua.register_module("socket", luasocket::create_module(lua)?)?;
    lua.register_module("lcurl.safe", luacurl::create_module(lua)?)?;
    Ok(())
}

/// Nothing is registered so Lua resolves the required modules through `package.cpath`.
#[cfg(not(feature = "bundled-lua-modules"))]
pub fn register(_lua: &Lua) -> LuaResult<()> {
    Ok(())
}
