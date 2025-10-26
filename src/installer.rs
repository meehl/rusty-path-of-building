use crate::{
    app::AppState,
    args::Game,
    color::Srgba,
    dpi::{LogicalPoint, LogicalRect},
    fonts::{Alignment, LayoutJob},
    mode::{AppEvent, ModeFrameOutput, ModeTransition},
    renderer::primitives::{ClippedPrimitive, DrawPrimitive, TextPrimitive},
};
use anyhow::bail;
use flate2::read::GzDecoder;
use std::{
    fs::{self},
    io::copy,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

const REPO_NAME: &str = "meehl/rusty-pob-manifest";

enum DownloadProgress {
    Percentage(f32),
    TotalBytes(u64),
}

enum Progress {
    Download(DownloadProgress), // download progress as a percentage (between 0 and 1)
    Complete,
    Error(anyhow::Error),
}

enum CurrentProgress {
    Starting,
    Download(DownloadProgress),
}

/// Execution mode in which PoB's assets are downloaded if they don't exist yet.
///
/// Immediately transitions into PoB mode if assets already exist. Otherwise,
/// it downloads them to the user directory and displays the download progress.
pub struct InstallMode {
    progress_rx: Option<Receiver<Progress>>,
    current_progress: CurrentProgress,
}

impl InstallMode {
    pub fn new() -> Self {
        let script_dir = Game::script_dir();
        let (progress_tx, progress_rx) = mpsc::channel();

        thread::spawn(move || {
            // Launch already exists meaning assets have already been downloaded
            if script_dir.join("Launch.lua").exists() {
                progress_tx.send(Progress::Complete).unwrap();
                return;
            }

            if let Err(err) = download_path_of_building(script_dir.as_path(), &progress_tx) {
                progress_tx.send(Progress::Error(err)).unwrap();
            }

            replace_manifest(script_dir.as_path()).unwrap();
            progress_tx.send(Progress::Complete).unwrap();
        });

        Self {
            progress_rx: Some(progress_rx),
            current_progress: CurrentProgress::Starting,
        }
    }

    pub fn frame(&mut self, app_state: &mut AppState) -> anyhow::Result<ModeFrameOutput> {
        let primitives = self.draw_current_progress(app_state);

        Ok(ModeFrameOutput {
            primitives,
            can_elide: false,
        })
    }

    pub fn update(&mut self, _app_state: &mut AppState) -> anyhow::Result<Option<ModeTransition>> {
        if let Some(progress_rx) = &self.progress_rx {
            loop {
                match progress_rx.try_recv() {
                    Ok(Progress::Download(progress)) => {
                        self.current_progress = CurrentProgress::Download(progress);
                    }
                    Ok(Progress::Complete) => {
                        return Ok(Some(ModeTransition::PoB));
                    }
                    Ok(Progress::Error(err)) => {
                        return Err(anyhow::anyhow!("Download failed: {}", err));
                    }
                    Err(TryRecvError::Disconnected) => {
                        return Err(anyhow::anyhow!("Download thread disconnected!"));
                    }
                    Err(TryRecvError::Empty) => {
                        break;
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn handle_event(
        &mut self,
        _app_state: &mut AppState,
        _event: AppEvent,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn draw_current_progress(
        &self,
        app_state: &mut AppState,
    ) -> Box<dyn Iterator<Item = ClippedPrimitive>> {
        let mut job = LayoutJob::new(
            parley::GenericFamily::SansSerif,
            32.0,
            34.0,
            Some(Alignment::Center),
            Some(700.0),
        );

        let progress_text = match &self.current_progress {
            CurrentProgress::Starting => String::from("Starting download..."),
            CurrentProgress::Download(progress) => match progress {
                DownloadProgress::Percentage(progress) => {
                    let percent = (progress * 100.0) as u32;
                    format!("Downloading assets... ({})", percent)
                }
                DownloadProgress::TotalBytes(total_bytes) => {
                    format!("Downloading assets... ({})", format_bytes(*total_bytes))
                }
            },
        };
        job.append(&progress_text, Srgba::WHITE);

        let layout = app_state.fonts.layout(job, app_state.window.scale_factor);

        // center text vertically and horizontally
        let screen_size = app_state.window.logical_size().cast::<f32>();
        let pos = LogicalPoint::new(screen_size.width / 2.0, screen_size.height / 2.0);

        let primitive = TextPrimitive::new(pos, layout);

        let clipped_primitive = ClippedPrimitive {
            clip_rect: LogicalRect::from_size(app_state.window.logical_size().cast()),
            primitive: DrawPrimitive::Text(primitive),
        };

        let primitives = vec![clipped_primitive];
        Box::new(primitives.into_iter())
    }
}

/// Downloads latest release of Path of Building
fn download_path_of_building<P: AsRef<Path>>(
    target_dir: P,
    progress_tx: &mpsc::Sender<Progress>,
) -> anyhow::Result<()> {
    log::info!("Downloading Path of Building assets...");

    let repo = match Game::current() {
        Game::Poe1 => "PathOfBuildingCommunity/PathOfBuilding",
        Game::Poe2 => "PathOfBuildingCommunity/PathOfBuilding-PoE2",
    };
    let url = format!(
        "https://github.com/{}/archive/refs/heads/master.tar.gz",
        repo
    );

    let mut response = ureq::get(url).call()?;
    let total_size = response
        .headers()
        .get("Content-Length")
        .and_then(|s| s.to_str().ok()?.parse::<u64>().ok());

    let body_reader = response.body_mut().as_reader();
    let mut archive = tar::Archive::new(GzDecoder::new(body_reader));
    let mut downloaded = 0u64;

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

        downloaded += file.size();

        if let Some(total) = total_size {
            let progress = downloaded as f32 / total as f32;
            progress_tx.send(Progress::Download(DownloadProgress::Percentage(progress)))?;
        } else {
            progress_tx.send(Progress::Download(DownloadProgress::TotalBytes(downloaded)))?;
        }
    }

    Ok(())
}

/// Replace manifest and UpdateCheck with rusty-path-of-building's modified versions
/// This is needed to make updating work without writing our own update system.
fn replace_manifest<P: AsRef<Path>>(target_dir: P) -> anyhow::Result<()> {
    log::info!("Downloading modified manifest...");

    let game_path = match Game::current() {
        Game::Poe1 => "poe1",
        Game::Poe2 => "poe2",
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

fn format_bytes(size_in_bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size_in_bytes >= GB {
        format!("{:.2} GB", size_in_bytes as f64 / GB as f64)
    } else if size_in_bytes >= MB {
        format!("{:.2} MB", size_in_bytes as f64 / MB as f64)
    } else if size_in_bytes >= KB {
        format!("{:.2} KB", size_in_bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", size_in_bytes)
    }
}
