use ahash::AHasher;
use mlua::{Lua, Result as LuaResult, Table};
use std::{
    env,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

pub fn get_executable_dir() -> anyhow::Result<PathBuf> {
    let exe_path = env::current_exe()?;
    let exe_parent_dir = exe_path.parent().unwrap().canonicalize()?;
    Ok(exe_parent_dir)
}

pub fn change_working_directory<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    env::set_current_dir(path.as_ref()).map_err(|e| {
        anyhow::anyhow!(
            "Failed to change working directory {}: {}",
            path.as_ref().display(),
            e
        )
    })
}

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut state = AHasher::default();
    t.hash(&mut state);
    state.finish()
}

/// Performs replacement only in lines that match a given pattern
pub fn replace_in_matching_lines(
    input: &str,
    match_pattern: &str,
    replace_pattern: &str,
    replacement_text: &str,
) -> String {
    let match_re = regex::Regex::new(match_pattern).expect("Invalid match regex");
    let replace_re = regex::Regex::new(replace_pattern).expect("Invalid replace regex");

    let mut output = String::new();
    for line in input.lines() {
        if match_re.is_match(line) {
            // if line matches the pattern, replace
            let replaced_line = replace_re.replace_all(line, replacement_text);
            output.push_str(&replaced_line);
        } else {
            // otherwise, keep original line
            output.push_str(line);
        }
        output.push('\n');
    }
    output
}

/// Appends a search pattern to Lua's `package.path`.
pub fn append_lua_package_path(lua: &Lua, pattern: &str) -> LuaResult<()> {
    let package: Table = lua.globals().get("package")?;
    let mut package_path: String = package.get("path")?;

    if !package_path.is_empty() {
        package_path.push(';');
    }

    package_path.push_str(pattern);
    package.set("path", package_path)?;

    Ok(())
}

/// Adds a directory to Lua's `package.path`.
///
/// This adds the following patterns:
/// - `<path>/?.lua`
/// - `<path>/?/init.lua`
pub fn append_lua_package_dir(lua: &Lua, path: &Path) -> LuaResult<()> {
    let path = path.to_string_lossy();
    append_lua_package_path(lua, &format!("{path}/?.lua"))?;
    append_lua_package_path(lua, &format!("{path}/?/init.lua"))?;
    Ok(())
}
