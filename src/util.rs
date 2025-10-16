use std::{
    env,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use ahash::AHasher;

pub fn get_executable_dir() -> anyhow::Result<PathBuf> {
    let exe_path = env::current_exe()?;
    let exe_parent_dir = exe_path.parent().unwrap().canonicalize()?;
    Ok(exe_parent_dir)
}

pub fn change_working_directory<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    env::set_current_dir(path.as_ref()).map_err(|e| {
        anyhow::anyhow!(
            "Failed to change working directory {:?}: {}",
            path.as_ref(),
            e
        )
    })
}

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut state = AHasher::default();
    t.hash(&mut state);
    state.finish()
}
