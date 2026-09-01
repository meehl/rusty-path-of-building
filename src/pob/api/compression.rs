use flate2::{
    Compression,
    read::{ZlibDecoder, ZlibEncoder},
};
use mlua::{IntoLuaMulti, Lua, LuaString, MultiValue, Result as LuaResult, Value};
use std::io::Read;

pub fn inflate(l: &Lua, compressed: LuaString) -> LuaResult<MultiValue> {
    let compressed_bytes = &compressed.as_bytes()[..];

    // prevent decompression of input larger than 128MiB
    if compressed_bytes.len() > (128 << 20) {
        return (Value::Nil, "Input larger than 128 MiB").into_lua_multi(l);
    }

    let mut decoder = ZlibDecoder::new(compressed_bytes);
    let mut decompressed = Vec::new();
    match decoder.read_to_end(&mut decompressed) {
        Ok(_) => l.create_string(&decompressed).unwrap().into_lua_multi(l),
        Err(e) => (Value::Nil, e.to_string()).into_lua_multi(l),
    }
}

pub fn deflate(l: &Lua, uncompressed: LuaString) -> LuaResult<MultiValue> {
    let uncompressed_bytes = &uncompressed.as_bytes()[..];

    // prevent compression of input larger than 128MiB
    if uncompressed_bytes.len() > (128 << 20) {
        return (Value::Nil, "Input larger than 128 MiB").into_lua_multi(l);
    }

    let mut encoder = ZlibEncoder::new(uncompressed_bytes, Compression::fast());
    let mut compressed = Vec::new();
    match encoder.read_to_end(&mut compressed) {
        Ok(_) => l.create_string(&compressed).unwrap().into_lua_multi(l),
        Err(e) => (Value::Nil, e.to_string()).into_lua_multi(l),
    }
}
