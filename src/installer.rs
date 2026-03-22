use crate::{
    app::AppState,
    args::Game,
    color::Srgba,
    dpi::{LogicalPoint, LogicalRect},
    fonts::{Alignment, FontStyle, LayoutJob},
    installer::download::{
        DownloadEvent, ExtractionRule, build_client, download_and_extract_tarball,
        download_file_to_disk, fetch_file_contents,
    },
    mode::{AppEvent, ModeFrameOutput, ModeTransition},
    renderer::primitives::{ClippedPrimitive, DrawPrimitive, TextPrimitive},
    util::replace_in_matching_lines,
};
use parley::{FontFamily, GenericFamily};
use regex::Regex;
use std::{
    fs,
    path::Path,
    sync::{
        LazyLock,
        mpsc::{self, Receiver, TryRecvError},
    },
    thread,
};

mod download;

const COMPAT_REPO: &str = "meehl/rusty-pob-manifest";

static VERSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)\.(\d+)\.(\d+)$").unwrap());

static MANIFEST_VERSION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<Version").unwrap());

enum Progress {
    Status(String),
    Complete,
    Error(anyhow::Error),
}

/// Execution mode in which initial installation of PoB is performed.
///
/// Immediately transitions into PoB mode if already installed.
pub struct InstallMode {
    progress_rx: Option<Receiver<Progress>>,
    status: String,
}

impl InstallMode {
    pub fn new(game: Game) -> Self {
        let script_dir = game.script_dir();
        let (progress_tx, progress_rx) = mpsc::channel();

        thread::spawn(move || {
            if let Err(err) = install(script_dir.as_path(), game, &progress_tx) {
                let _ = progress_tx.send(Progress::Error(err));
                return;
            }
            let _ = progress_tx.send(Progress::Complete);
        });

        Self {
            progress_rx: Some(progress_rx),
            status: String::from("Starting installation..."),
        }
    }

    pub fn frame(&mut self, app_state: &mut AppState) -> anyhow::Result<ModeFrameOutput> {
        let primitives = self.draw_current_progress(app_state);

        Ok(ModeFrameOutput {
            primitives,
            can_elide: false,
            should_continue: true,
        })
    }

    pub fn update(&mut self, _app_state: &mut AppState) -> anyhow::Result<Option<ModeTransition>> {
        if let Some(progress_rx) = &self.progress_rx {
            loop {
                match progress_rx.try_recv() {
                    Ok(Progress::Status(msg)) => self.status = msg,
                    Ok(Progress::Complete) => return Ok(Some(ModeTransition::PoB)),
                    Ok(Progress::Error(err)) => {
                        return Err(anyhow::anyhow!("Installation failed: {err}"));
                    }
                    Err(TryRecvError::Disconnected) => {
                        return Err(anyhow::anyhow!("Install thread disconnected unexpectedly"));
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
            FontFamily::Generic(GenericFamily::SansSerif),
            32.0,
            34.0,
            Some(Alignment::Center),
            Some(700.0),
            FontStyle::Normal,
        );

        job.append(&self.status, Srgba::WHITE);

        let layout = app_state.fonts.layout(job, app_state.window.scale_factor());

        let screen_size = app_state.window.logical_size().cast::<f32>();
        let pos = LogicalPoint::new(screen_size.width / 2.0, screen_size.height / 2.0);

        let primitive = TextPrimitive::new(pos, layout);

        let clipped_primitive = ClippedPrimitive {
            clip_rect: LogicalRect::from_size(app_state.window.logical_size().cast()),
            primitive: DrawPrimitive::Text(primitive),
        };

        Box::new(vec![clipped_primitive].into_iter())
    }
}

// Helper function for writing status to both the message channel and the log
fn report(tx: &mpsc::Sender<Progress>, msg: impl Into<String>) {
    let msg = msg.into();
    log::info!("{msg}");
    let _ = tx.send(Progress::Status(msg));
}

/// Performs full installation of PoB assets into `target_dir`.
///
/// Skips installation if `rpob.version` already exists.
///
/// Steps:
/// 1. Fetch the compatibility table to determine which PoB version is
///    supported by the current Rusty PoB version.
/// 2. Download and extract the PoB release tarball into `target_dir`.
/// 3. Replace `UpdateCheck.lua` with a patched version and update its
///    checksum in `manifest.xml`. This is needed to support PoB native updater.
/// 4. Set the branch and platform fields in `manifest.xml`.
/// 5. Write `rpob.version` to mark the installation as complete.
fn install<P: AsRef<Path>>(
    target_dir: P,
    game: Game,
    progress_tx: &mpsc::Sender<Progress>,
) -> anyhow::Result<()> {
    let target_dir = target_dir.as_ref();
    let client = build_client()?;

    let version_file_path = target_dir.join("rpob.version");
    let current_version = env!("CARGO_PKG_VERSION");
    if version_file_path.exists() {
        let old_version = fs::read_to_string(&version_file_path)?;
        if old_version != current_version {
            log::info!("Version changed: {old_version} -> {current_version}");
            fs::write(&version_file_path, current_version).unwrap();
        }
        return Ok(());
    }

    report(progress_tx, "Fetching compatibility info...");
    let compat_info = fetch_compatibility_info(&client, game)?;
    let pob_version = highest_supported_pob_version(&compat_info, current_version)
        .ok_or_else(|| anyhow::anyhow!("Unable to determine supported PoB version"))?;
    log::info!("Using PoB version: {pob_version}");

    report(progress_tx, "Downloading assets...");
    download_pob(&client, target_dir, game, pob_version, progress_tx)?;

    report(progress_tx, "Finalizing installation...");
    replace_updatecheck(&client, target_dir)?;
    set_branch_and_platform(target_dir)?;

    fs::write(&version_file_path, current_version)?;
    log::info!("Installation complete.");

    Ok(())
}

#[derive(Debug)]
struct VersionReq {
    pob_ver: String,
    min_rpob_ver: String,
}

/// Fetches compatibility info
fn fetch_compatibility_info(
    client: &reqwest::blocking::Client,
    game: Game,
) -> anyhow::Result<Vec<VersionReq>> {
    let file_name = match game {
        Game::Poe1 => "Compatibility_pob1.lua",
        Game::Poe2 => "Compatibility_pob2.lua",
    };

    let compatibility_info_file = fetch_file_contents(client, COMPAT_REPO, file_name)?;

    // Load and evaluate compatibility file contents as Lua code
    let lua = mlua::Lua::new();
    let compatibility_table = lua.load(compatibility_info_file).eval::<mlua::Table>()?;

    // Compatibility table maps PoB version to minimum required Rusty PoB version.
    // Beta versions have a non-semver suffix and are filtered out by the regex.
    Ok(compatibility_table
        .pairs::<String, String>()
        .filter_map(|p| p.ok())
        .filter(|(pob_version, _)| VERSION_RE.is_match(pob_version))
        .map(|(pob_version, min_req_rpob_ver)| VersionReq {
            pob_ver: pob_version,
            min_rpob_ver: min_req_rpob_ver,
        })
        .collect())
}

/// Determines the highest PoB version supported by the given Rusty PoB version.
fn highest_supported_pob_version<'a>(
    compatibility_info: &'a [VersionReq],
    rpob_version: &str,
) -> Option<&'a str> {
    let mut highest: Option<&str> = None;
    for VersionReq {
        pob_ver,
        min_rpob_ver,
    } in compatibility_info
    {
        if is_higher_version(min_rpob_ver, rpob_version).unwrap_or(false) {
            match highest {
                Some(h) if !is_higher_version(h, pob_ver).unwrap_or(false) => {}
                _ => highest = Some(pob_ver.as_str()),
            }
        }
    }
    highest
}

fn download_pob<P: AsRef<Path>>(
    client: &reqwest::blocking::Client,
    target_dir: P,
    game: Game,
    pob_version: &str,
    progress_tx: &mpsc::Sender<Progress>,
) -> anyhow::Result<()> {
    let target_dir = target_dir.as_ref();

    let pob_repo = match game {
        Game::Poe1 => "PathOfBuildingCommunity/PathOfBuilding",
        Game::Poe2 => "PathOfBuildingCommunity/PathOfBuilding-PoE2",
    };

    let rules = vec![
        ExtractionRule::File {
            tarball_path: "manifest.xml".into(),
            dest_path: target_dir.join("manifest.xml"),
        },
        ExtractionRule::File {
            tarball_path: "help.txt".into(),
            dest_path: target_dir.join("help.txt"),
        },
        ExtractionRule::File {
            tarball_path: "changelog.txt".into(),
            dest_path: target_dir.join("changelog.txt"),
        },
        ExtractionRule::File {
            tarball_path: "LICENSE.md".into(),
            dest_path: target_dir.join("LICENSE.md"),
        },
        ExtractionRule::RewritePrefix {
            prefix: "src/".into(),
            dest_dir: target_dir.to_path_buf(),
        },
        ExtractionRule::RewritePrefix {
            prefix: "runtime/lua/".into(),
            dest_dir: target_dir.join("lua"),
        },
    ];

    download_and_extract_tarball(
        &client,
        pob_repo,
        &format!("v{pob_version}"),
        &rules,
        5,
        &mut |event| {
            let msg = match event {
                DownloadEvent::Progress {
                    downloaded,
                    total: Some(total),
                } => {
                    let pct = (downloaded as f32 / total as f32 * 100.0) as u32;
                    format!("Downloading assets... ({pct}%)")
                }
                DownloadEvent::Progress {
                    downloaded,
                    total: None,
                } => {
                    format!("Downloading assets... ({})", format_bytes(downloaded))
                }
                DownloadEvent::Retrying { attempt } => {
                    format!("Retrying... (Attempt {})", attempt)
                }
            };
            let _ = progress_tx.send(Progress::Status(msg));
        },
    )?;
    Ok(())
}

/// Replaces UpdateCheck.lua with rusty-path-of-building's modified version and
/// updates its checksum in manifest.xml.
fn replace_updatecheck<P: AsRef<Path>>(
    client: &reqwest::blocking::Client,
    target_dir: P,
) -> anyhow::Result<()> {
    let target_dir = target_dir.as_ref();
    let file_name = "UpdateCheck.lua";

    download_file_to_disk(client, COMPAT_REPO, file_name, target_dir.join(file_name))?;

    // .sha1 file format: "<checksum>  UpdateCheck.lua" - we only need the checksum.
    let sha1_contents = fetch_file_contents(client, COMPAT_REPO, "UpdateCheck.lua.sha1")?;
    let new_checksum = sha1_contents
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invalid checksum file format"))?;

    let manifest_path = target_dir.join("manifest.xml");
    let manifest = fs::read_to_string(&manifest_path)?;
    let new_manifest = replace_in_matching_lines(
        &manifest,
        r#"name="UpdateCheck.lua""#,
        r#"sha1="([0-9A-Za-z]+)""#,
        &format!(r#"sha1="{new_checksum}""#),
    );
    fs::write(&manifest_path, &new_manifest)?;

    Ok(())
}

/// Sets branch and platform in manifest.xml
fn set_branch_and_platform<P: AsRef<Path>>(target_dir: P) -> anyhow::Result<()> {
    let path = target_dir.as_ref().join("manifest.xml");
    let manifest = fs::read_to_string(&path)?;

    #[cfg(target_os = "windows")]
    let platform = "win32";
    #[cfg(not(target_os = "windows"))]
    let platform = std::env::consts::OS;

    let replacement = format!(r#"<Version branch="master" platform="{platform}""#);
    let new_manifest = MANIFEST_VERSION_RE.replace(&manifest, replacement);
    fs::write(&path, new_manifest.as_ref())?;

    Ok(())
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
        format!("{size_in_bytes} bytes")
    }
}

/// Returns true if `v2` is greater than or equal to `v1`.
fn is_higher_version(v1: &str, v2: &str) -> anyhow::Result<bool> {
    let parse_version = |v: &str| -> anyhow::Result<(u32, u32, u32)> {
        let caps = VERSION_RE
            .captures(v)
            .ok_or_else(|| anyhow::anyhow!("Invalid semver format: {}", v))?;

        let major = caps[1].parse::<u32>().unwrap();
        let minor = caps[2].parse::<u32>().unwrap();
        let patch = caps[3].parse::<u32>().unwrap();

        Ok((major, minor, patch))
    };

    let (major1, minor1, patch1) = parse_version(v1)?;
    let (major2, minor2, patch2) = parse_version(v2)?;

    Ok(major2 > major1
        || (major2 == major1 && minor2 > minor1)
        || (major2 == major1 && minor2 == minor1 && patch2 >= patch1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_major_version() {
        assert!(is_higher_version("1.0.0", "2.0.0").unwrap());
        assert!(!is_higher_version("2.0.0", "1.0.0").unwrap());
    }

    #[test]
    fn test_minor_version() {
        assert!(is_higher_version("1.5.0", "1.6.0").unwrap());
        assert!(!is_higher_version("1.10.0", "1.9.0").unwrap());
    }

    #[test]
    fn test_patch_version() {
        assert!(is_higher_version("1.0.3", "1.0.4").unwrap());
        assert!(!is_higher_version("1.0.10", "1.0.9").unwrap());
    }

    #[test]
    fn test_same_version() {
        assert!(is_higher_version("1.5.3", "1.5.3").unwrap());
    }

    #[test]
    fn test_invalid_semver_format() {
        assert!(is_higher_version("1.0", "2.0.0").is_err());
        assert!(is_higher_version("1.0.0", "2.0").is_err());
        assert!(is_higher_version("invalid", "2.0.0").is_err());
        assert!(is_higher_version("1.0.0", "a.bb.ccc").is_err());
    }

    #[test]
    fn test_highest_supported_pob_ver() {
        let compat_info = vec![
            VersionReq {
                pob_ver: "2.56.0".into(),
                min_rpob_ver: "0.1.0".into(),
            },
            // compat info might not be sorted by pob_version
            VersionReq {
                pob_ver: "2.58.0".into(),
                min_rpob_ver: "0.2.6".into(),
            },
            VersionReq {
                pob_ver: "2.57.0".into(),
                min_rpob_ver: "0.2.6".into(),
            },
            VersionReq {
                pob_ver: "2.58.1".into(),
                min_rpob_ver: "0.2.8".into(),
            },
            VersionReq {
                pob_ver: "2.59.0".into(),
                min_rpob_ver: "0.2.9".into(),
            },
            VersionReq {
                pob_ver: "2.59.1".into(),
                min_rpob_ver: "0.2.10".into(),
            },
            VersionReq {
                pob_ver: "2.59.2".into(),
                min_rpob_ver: "0.2.10".into(),
            },
        ];
        assert_eq!(highest_supported_pob_version(&compat_info, "0.0.2"), None);
        assert_eq!(highest_supported_pob_version(&compat_info, "a.b.c"), None);
        assert_eq!(
            highest_supported_pob_version(&compat_info, "0.1.0"),
            Some("2.56.0")
        );
        assert_eq!(
            highest_supported_pob_version(&compat_info, "0.2.0"),
            Some("2.56.0")
        );
        assert_eq!(
            highest_supported_pob_version(&compat_info, "0.2.6"),
            Some("2.58.0")
        );
        assert_eq!(
            highest_supported_pob_version(&compat_info, "0.2.7"),
            Some("2.58.0")
        );
        assert_eq!(
            highest_supported_pob_version(&compat_info, "0.2.8"),
            Some("2.58.1")
        );
        assert_eq!(
            highest_supported_pob_version(&compat_info, "0.2.9"),
            Some("2.59.0")
        );
        assert_eq!(
            highest_supported_pob_version(&compat_info, "0.2.10"),
            Some("2.59.2")
        );
    }
}
