use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use glob::Paths;
use mlua::{IntoLua, UserData};

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
