use glob::{Paths, glob};
use mlua::{IntoLua, Lua, Result as LuaResult, UserData, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub fn new_search_handle(
    l: &Lua,
    (pattern, find_directories): (String, Option<bool>),
) -> LuaResult<Value> {
    if let Ok(paths) = glob(&pattern) {
        let directories_only = find_directories.is_some_and(|x| x);
        let mut handle = SearchHandle::new(paths, directories_only);
        // try to get the first result
        handle.next();
        // only return a handle if at least one file/directory is found
        if handle.current.is_some() {
            return handle.into_lua(l);
        }
    };
    Ok(Value::Nil)
}

pub struct SearchHandle {
    paths: Paths,
    // only yield directories if true, otherwise only files
    directories_only: bool,
    pub current: Option<PathBuf>,
}

impl SearchHandle {
    pub fn new(paths: Paths, directories_only: bool) -> Self {
        Self {
            paths,
            directories_only,
            current: None,
        }
    }

    // sets current to the next file/directory if it exists and doesn't cause errors,
    // otherwise None
    pub fn next(&mut self) {
        self.current = self
            .paths
            .find(|candidate| {
                candidate
                    .as_ref()
                    .is_ok_and(|c| c.is_dir() == self.directories_only)
            })
            .transpose()
            .ok()
            .flatten();
    }
}

impl UserData for SearchHandle {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("NextFile", |_, this, ()| {
            this.next();
            Ok(this.current.is_some())
        });
        methods.add_method_mut("GetFileName", |l, this, ()| {
            Ok(this.current.as_ref().unwrap().file_name().into_lua(l))
        });
        methods.add_method("GetFileSize", |_, this, ()| match &this.current {
            Some(path) => match fs::metadata(path) {
                Ok(metadata) => Ok(metadata.len()),
                Err(_) => Ok(0),
            },
            None => Ok(0),
        });
        methods.add_method("GetFileModifiedTime", |_, this, ()| match &this.current {
            Some(path) => get_time_modified(path).map_or(Ok(0), Ok),
            None => Ok(0),
        });
    }
}

fn get_time_modified<P: AsRef<Path>>(path: P) -> anyhow::Result<u64> {
    let metadata = fs::metadata(path)?;
    let modified_time = metadata.modified()?;
    let duration_since_epoch = modified_time.duration_since(SystemTime::UNIX_EPOCH)?;
    let seconds_since_epoch = duration_since_epoch.as_secs();
    Ok(seconds_since_epoch)
}
