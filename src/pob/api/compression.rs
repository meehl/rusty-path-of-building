use flate2::{
    Compression,
    read::{GzDecoder, GzEncoder, ZlibDecoder, ZlibEncoder},
};
use mlua::{IntoLuaMulti, Lua, LuaString, MultiValue, Result as LuaResult, Value};
use std::io::Read;

pub fn inflate(l: &Lua, compressed: LuaString) -> LuaResult<MultiValue> {
    let compressed_bytes = &compressed.as_bytes()[..];

    // prevent decompression of input larger than 128MiB
    if compressed_bytes.len() > (128 << 20) {
        return (Value::Nil, "Input larger than 128 MiB").into_lua_multi(l);
    }

    let mut decompressed = Vec::new();

    let result = if is_gzip(compressed_bytes) {
        GzDecoder::new(compressed_bytes).read_to_end(&mut decompressed)
    } else {
        ZlibDecoder::new(compressed_bytes).read_to_end(&mut decompressed)
    };

    match result {
        Ok(_) => l.create_string(&decompressed)?.into_lua_multi(l),
        Err(e) => (Value::Nil, e.to_string()).into_lua_multi(l),
    }
}

fn is_gzip(data: &[u8]) -> bool {
    data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b
}

pub fn deflate(
    l: &Lua,
    (uncompressed, use_gzip): (LuaString, Option<bool>),
) -> LuaResult<MultiValue> {
    let uncompressed_bytes = &uncompressed.as_bytes()[..];

    // prevent compression of input larger than 128MiB
    if uncompressed_bytes.len() > (128 << 20) {
        return (Value::Nil, "Input larger than 128 MiB").into_lua_multi(l);
    }

    let mut compressed = Vec::new();

    let result = if use_gzip.unwrap_or(false) {
        GzEncoder::new(uncompressed_bytes, Compression::best()).read_to_end(&mut compressed)
    } else {
        ZlibEncoder::new(uncompressed_bytes, Compression::best()).read_to_end(&mut compressed)
    };

    match result {
        Ok(_) => l.create_string(&compressed)?.into_lua_multi(l),
        Err(e) => (Value::Nil, e.to_string()).into_lua_multi(l),
    }
}
