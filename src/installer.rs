use crate::args::Game;
use anyhow::bail;
use flate2::read::GzDecoder;
use std::{
    fs::{self},
    io::copy,
    path::{Path, PathBuf},
};

const REPO_NAME: &str = "meehl/RustyPathOfBuilding";

pub fn run_installer() -> anyhow::Result<()> {
    let script_dir = Game::script_dir();

    // run installer iff Launch.lua doesn't exist, meaning this is likely the first run
    if script_dir.join("Launch.lua").exists() {
        return Ok(());
    }

    download_path_of_building(script_dir.as_path())?;
    replace_manifest(script_dir.as_path())?;

    Ok(())
}

/// Download latest release of Path of Building
fn download_path_of_building<P: AsRef<Path>>(target_dir: P) -> anyhow::Result<()> {
    println!("Downloading latest release of Path of Building...");

    let repo = match Game::current() {
        Game::Poe1 => "PathOfBuildingCommunity/PathOfBuilding",
        Game::Poe2 => "PathOfBuildingCommunity/PathOfBuilding-PoE2",
    };
    let url = format!(
        "https://github.com/{}/archive/refs/heads/master.tar.gz",
        repo
    );

    let mut response = ureq::get(url).call()?;
    let body_reader = response.body_mut().as_reader();

    let mut archive = tar::Archive::new(GzDecoder::new(body_reader));

    for file in archive.entries()? {
        let mut file = file?;
        let file_path = file.path()?;
        let components: Vec<_> = file_path.components().collect();

        let target_path = match components.len() {
            0..=1 => None,
            // put these into target_dir/
            2 => {
                let filename = components[1].as_os_str();
                if filename == "manifest.xml"
                    || filename == "help.txt"
                    || filename == "changelog.txt"
                    || filename == "LICENSE.md"
                {
                    Some(target_dir.as_ref().join(filename))
                } else {
                    None
                }
            }
            // put lua runtime files into target_dir/lua/
            3.. => {
                if components[1].as_os_str() == "src"
                    || (components[1].as_os_str() == "runtime"
                        && components[2].as_os_str() == "lua")
                {
                    Some(
                        target_dir
                            .as_ref()
                            .join(components[2..].iter().collect::<PathBuf>()),
                    )
                } else {
                    None
                }
            }
        };

        // create needed directories and extract
        if let Some(target_path) = target_path {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            file.unpack(&target_path)?;
        }
    }

    Ok(())
}

/// Replace manifest and UpdateCheck with RustyPathOfBuilding's modified versions
/// This is needed to make updating work without writing our own update system.
fn replace_manifest<P: AsRef<Path>>(target_dir: P) -> anyhow::Result<()> {
    log::info!("Replacing manifest...");

    let game_path = match Game::current() {
        Game::Poe1 => "lua/poe1",
        Game::Poe2 => "lua/poe2",
    };

    // Download on overwrite files
    download_file(
        &format!(
            "https://raw.githubusercontent.com/{REPO_NAME}/master/{}/{}",
            game_path, "manifest.xml"
        ),
        target_dir.as_ref().join("manifest.xml"),
    )?;

    download_file(
        &format!(
            "https://raw.githubusercontent.com/{REPO_NAME}/master/{}/{}",
            game_path, "UpdateCheck.lua"
        ),
        target_dir.as_ref().join("UpdateCheck.lua"),
    )?;

    Ok(())
}

fn download_file<P: AsRef<Path>>(url: &str, file_path: P) -> anyhow::Result<()> {
    let mut response = ureq::get(url).call()?;

    if response.status().is_success() {
        let body = response.body_mut();
        let mut file = fs::File::create(file_path)?;
        copy(&mut body.as_reader(), &mut file)?;
        Ok(())
    } else {
        bail!("Unable to download: {}", url);
    }
}
