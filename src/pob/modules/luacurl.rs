use mlua::{Lua, Result as LuaResult, Table};

use crate::pob::modules::luacurl::easy::EasyHandle;

mod constants;
mod easy;

pub fn create_module(lua: &Lua) -> LuaResult<Table> {
    let exports = lua.create_table()?;

    exports.set("easy", lua.create_function(|_, ()| Ok(EasyHandle::new()))?)?;

    exports.set("OPT_ACCEPT_ENCODING", constants::OPT_ACCEPT_ENCODING)?;
    exports.set("OPT_FOLLOWLOCATION", constants::OPT_FOLLOWLOCATION)?;
    exports.set("OPT_HTTPHEADER", constants::OPT_HTTPHEADER)?;
    exports.set("OPT_IPRESOLVE", constants::OPT_IPRESOLVE)?;
    exports.set("OPT_POST", constants::OPT_POST)?;
    exports.set("OPT_POSTFIELDS", constants::OPT_POSTFIELDS)?;
    exports.set("OPT_PROXY", constants::OPT_PROXY)?;
    exports.set("OPT_SSL_VERIFYHOST", constants::OPT_SSL_VERIFYHOST)?;
    exports.set("OPT_SSL_VERIFYPEER", constants::OPT_SSL_VERIFYPEER)?;
    exports.set("OPT_USERAGENT", constants::OPT_USERAGENT)?;

    exports.set("INFO_RESPONSE_CODE", constants::INFO_RESPONSE_CODE)?;
    exports.set("INFO_SIZE_DOWNLOAD", constants::INFO_SIZE_DOWNLOAD)?;
    exports.set("INFO_REDIRECT_URL", constants::INFO_REDIRECT_URL)?;

    Ok(exports)
}
