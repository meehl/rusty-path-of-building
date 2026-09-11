//! Rust port of SimpleGraphic's `lzip`, a library for reading zip files.
//! <https://github.com/PathOfBuildingCommunity/PathOfBuilding-SimpleGraphic/blob/master/libs/LZip/lzip.cpp>

use std::{fs::File, io::Read};

use mlua::{Integer as LuaInteger, Lua, Result as LuaResult, Table, UserData, UserDataMethods};

use crate::pob::modules::luazip::archive::{ZipArchive, ZipFile};

mod archive;

pub fn create_module(lua: &Lua) -> LuaResult<Table> {
    let exports = lua.create_table()?;

    exports.set(
        "open",
        lua.create_function(|_, path: String| {
            let file = File::open(&path).map_err(mlua::Error::external)?;
            let archive = ZipArchive::new(file).map_err(mlua::Error::external)?;
            Ok(ZipArchiveHandle {
                archive: Some(archive),
            })
        })?,
    )?;

    Ok(exports)
}

fn to_index(i: LuaInteger, len: usize) -> Option<usize> {
    usize::try_from(i.checked_sub(1)?).ok().filter(|&i| i < len)
}

struct ZipArchiveHandle {
    archive: Option<ZipArchive<File>>,
}

impl ZipArchiveHandle {
    fn get(&self) -> LuaResult<&ZipArchive<File>> {
        self.archive
            .as_ref()
            .ok_or_else(|| mlua::Error::runtime("zip handle is closed"))
    }
}

impl UserData for ZipArchiveHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("Close", |_, this, ()| {
            this.archive = None;
            Ok(())
        });

        methods.add_method("GetNumFiles", |_, this, ()| Ok(this.get()?.entries().len()));

        methods.add_method("GetFileName", |lua, this, index: LuaInteger| {
            let entries = this.get()?.entries();
            let len = entries.len();
            match to_index(index, len) {
                Some(i) => Ok(Some(lua.create_string(entries[i].name())?)),
                None => Ok(None),
            }
        });

        methods.add_method("GetFileSize", |_, this, index: LuaInteger| {
            let entries = this.get()?.entries();
            let len = entries.len();
            match to_index(index, len) {
                Some(i) => Ok(Some(entries[i].uncompressed_size())),
                None => Ok(None),
            }
        });

        methods.add_method_mut("OpenFile", |_, this, index_or_name: mlua::Value| {
            let archive = this.get()?;
            let entries = archive.entries();

            let index = match index_or_name {
                mlua::Value::Integer(i) => {
                    let Some(index) = to_index(i, entries.len()) else {
                        return Err(mlua::Error::runtime("zip:OpenFile(): invalid index"));
                    };
                    index
                }
                mlua::Value::String(s) => {
                    let name = s.as_bytes();
                    let Some(index) = entries.iter().position(|e| e.name()[..] == *name) else {
                        return Ok(None);
                    };
                    index
                }
                _ => {
                    return Err(mlua::Error::runtime(
                        "Usage: zip:OpenFile(index) or zip:OpenFile(fileName)",
                    ));
                }
            };

            let file = archive.by_index(index).map_err(mlua::Error::external)?;
            Ok(Some(ZipFileHandle { file: Some(file) }))
        });
    }
}

struct ZipFileHandle {
    file: Option<ZipFile<File>>,
}

impl ZipFileHandle {
    fn get(&mut self) -> LuaResult<&ZipFile<File>> {
        self.file
            .as_ref()
            .ok_or_else(|| mlua::Error::runtime("zip file handle is closed"))
    }

    fn get_mut(&mut self) -> LuaResult<&mut ZipFile<File>> {
        self.file
            .as_mut()
            .ok_or_else(|| mlua::Error::runtime("zip file handle is closed"))
    }
}

impl UserData for ZipFileHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("Close", |_, this, ()| {
            this.file = None;
            Ok(())
        });

        methods.add_method_mut("Length", |_, this, ()| Ok(this.get()?.size()));

        methods.add_method_mut("Read", |lua, this, arg: mlua::Value| {
            let file = this.get_mut()?;
            let mut buf = Vec::new();

            match arg {
                mlua::Value::String(s) if s.as_bytes() == b"*a" => {
                    file.read_to_end(&mut buf).map_err(mlua::Error::external)?;
                }
                mlua::Value::Integer(n) if n >= 0 => {
                    if n == 0 {
                        return lua.create_string("");
                    }

                    file.take(n as u64)
                        .read_to_end(&mut buf)
                        .map_err(mlua::Error::external)?;
                }
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "zipFile:Read(): unrecognised format: {}",
                        other.to_string()?
                    )));
                }
            }

            lua.create_string(buf)
        });
    }
}
