use ahash::AHasher;
use mlua::{Lua, Result as LuaResult, Table, chunk::Chunk};
use std::{
    env, fs,
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

/// Rewrites PoB's `#@ <label>` script header into a Lua comment.
///
/// Some Path of Building scripts start with a line such as `#@ <optional_text>`.  Recent LuaJIT
/// versions interpret a leading `#` as a line number directive and reject this header so we
/// replace them with a comment.
///
/// commit that introduced changes to the lexer to enable parsing of line number directives:
/// <https://github.com/LuaJIT/LuaJIT/commit/c41d727dbffa5ec4f346dd9be0f4c64740f124aa>
pub fn fix_script_header(source: &mut [u8]) {
    // skip utf8-bom
    let start = if source.starts_with(b"\xEF\xBB\xBF") {
        3
    } else {
        0
    };
    if source[start..].starts_with(b"#@") {
        source[start..start + 2].copy_from_slice(b"--");
    }
}

/// Loads a Lua script from `path` and applies `fix_script_header`.
///
/// The chunk name is set to `@<path>` to match `Lua::load`'s behavior for paths, so error messages
/// keep referring to the file.
pub fn load_lua_file<'lua>(lua: &'lua Lua, path: &Path) -> LuaResult<Chunk<'lua>> {
    let mut source = fs::read(path)?;
    fix_script_header(&mut source);
    Ok(lua.load(source).set_name(format!("@{}", path.display())))
}
