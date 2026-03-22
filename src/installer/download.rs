use std::{
    fmt,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use flate2::read::GzDecoder;
use reqwest::blocking::{Client, Response};
use tar::Archive;

#[derive(Debug)]
pub enum DownloadEvent {
    Progress { downloaded: u64, total: Option<u64> },
    Retrying { attempt: u32 },
}

/// Build a reqwest blocking client
pub fn build_client() -> Result<Client, reqwest::Error> {
    Client::builder().timeout(Duration::from_secs(60)).build()
}

// Rate-limit-aware GET.
//
// `raw.githubusercontent.com` doesn't seem to use `x-ratelimit-reset` and
// `x-ratelimit-remaining` headers like the API so only the status code is checked.
fn get_checked(client: &Client, url: &str) -> Result<Response, GithubError> {
    let response = client.get(url).send()?;
    let status = response.status();

    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after_secs = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60);
        return Err(GithubError::RateLimited {
            retry_after_s: retry_after_secs,
        });
    }

    if !status.is_success() {
        return Err(GithubError::Http {
            status: status.as_u16(),
            url: url.to_string(),
        });
    }

    Ok(response)
}

/// Wrapper to retry `f` up to `max_retries` times
pub fn with_retry<T, F>(max_retries: u32, base_delay: Duration, mut f: F) -> Result<T, GithubError>
where
    F: FnMut(u32) -> Result<T, GithubError>,
{
    let mut attempt = 0;
    loop {
        match f(attempt) {
            Err(GithubError::RateLimited {
                retry_after_s: retry_after_secs,
            }) if attempt < max_retries => {
                log::warn!("Rate limited – sleeping {retry_after_secs}s (attempt {attempt})");
                thread::sleep(Duration::from_secs(retry_after_secs));
                attempt += 1;
            }
            Err(GithubError::Network(e)) if attempt < max_retries => {
                let delay = base_delay * (attempt + 1);
                log::warn!("Network error on attempt {attempt}: {e}. Retrying in {delay:?}");
                thread::sleep(delay);
                attempt += 1;
            }
            other => return other,
        }
    }
}

/// Download file contents from raw.githubusercontent.com into a `String`.
pub fn fetch_file_contents(
    client: &Client,
    repo: &str,
    file_name: &str,
) -> Result<String, GithubError> {
    let url = format!("https://raw.githubusercontent.com/{repo}/main/{file_name}");
    let response = get_checked(client, &url)?;
    Ok(response.text()?)
}

/// Download a file from raw.githubusercontent.com and write it to `dest_path`.
pub fn download_file_to_disk<P: AsRef<Path>>(
    client: &Client,
    repo: &str,
    file_name: &str,
    dest_path: P,
) -> Result<(), GithubError> {
    let dest_path = dest_path.as_ref();

    let url = format!("https://raw.githubusercontent.com/{repo}/main/{file_name}");
    let mut response = get_checked(client, &url)?;

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(dest_path)?;
    io::copy(&mut response, &mut file)?;
    Ok(())
}

/// A rule describing how to map tarball entries to destination paths.
///
/// The root prefix is automatically striped and doesn't need to be specified.
pub enum ExtractionRule {
    /// Map a single file to an exact destination path.
    File {
        tarball_path: String,
        dest_path: PathBuf,
    },
    /// Strip prefix from matching entries and place the remainder under `dest_dir`.
    RewritePrefix { prefix: String, dest_dir: PathBuf },
}

/// Download a GitHub release tarball and extract entries to specified destinations.
pub fn download_and_extract_tarball(
    client: &Client,
    repo_name: &str,
    tag: &str,
    rules: &[ExtractionRule],
    max_retries: u32,
    on_progress: &mut impl FnMut(DownloadEvent),
) -> Result<(), GithubError> {
    let url = format!("https://github.com/{repo_name}/archive/refs/tags/{tag}.tar.gz");

    let tmp_path =
        std::env::temp_dir().join(format!("gh-{}-{}.tar.gz", repo_name.replace('/', "-"), tag));

    // Download to temp file
    with_retry(max_retries, Duration::from_secs(5), |attempt| {
        if attempt > 0 {
            on_progress(DownloadEvent::Retrying { attempt });
        }

        let response = get_checked(client, &url)?;
        let content_length = response.content_length();

        let mut tmp_file = File::create(&tmp_path)?;
        const CHUNK_SIZE: usize = 512 * 1024;
        let mut chunk = vec![0u8; CHUNK_SIZE];
        let mut downloaded: u64 = 0;
        let mut body = response;

        let mut last_progress = Instant::now();
        const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

        loop {
            let n = body.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            tmp_file.write_all(&chunk[..n])?;
            downloaded += n as u64;

            if last_progress.elapsed() >= PROGRESS_INTERVAL {
                on_progress(DownloadEvent::Progress {
                    downloaded,
                    total: content_length,
                });
                last_progress = Instant::now();
            }
        }
        on_progress(DownloadEvent::Progress {
            downloaded,
            total: content_length,
        });

        Ok(())
    })?;

    // Extract tarball to destination based on extract rules
    let tmp_file = File::open(&tmp_path)?;
    let gz = GzDecoder::new(io::BufReader::new(tmp_file));
    let mut archive = Archive::new(gz);

    let mut entries = archive.entries()?;

    let root_prefix = {
        let first = entries
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "archive is empty"))??;
        first.path()?.to_string_lossy().into_owned()
    };

    for entry_result in entries {
        let mut entry = entry_result?;
        let entry_path = entry.path()?.to_path_buf();
        let entry_str = entry_path.to_string_lossy();

        // Skip directory entries
        if entry_str.ends_with('/') {
            continue;
        }

        // Strip root prefix before matching against rules
        let relative = match entry_str.strip_prefix(root_prefix.as_str()) {
            Some(r) => r,
            None => continue,
        };

        let dest: Option<PathBuf> = rules.iter().find_map(|rule| match rule {
            ExtractionRule::File {
                tarball_path,
                dest_path,
            } => (tarball_path == relative).then(|| dest_path.clone()),
            ExtractionRule::RewritePrefix { prefix, dest_dir } => relative
                .strip_prefix(prefix)
                .map(|tail| dest_dir.join(tail)),
        });

        if let Some(dest_path) = dest {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut dest_file = File::create(&dest_path)?;
            io::copy(&mut entry, &mut dest_file)?;
        }
    }

    fs::remove_file(&tmp_path)?;
    Ok(())
}

#[derive(Debug)]
pub enum GithubError {
    Http { status: u16, url: String },
    RateLimited { retry_after_s: u64 },
    Network(reqwest::Error),
    Io(io::Error),
}

impl fmt::Display for GithubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http { status, url } => write!(f, "HTTP error {status}: {url}"),
            Self::RateLimited { retry_after_s } => {
                write!(f, "Rate limited – retry after {retry_after_s}s")
            }
            Self::Network(e) => write!(f, "Network error: {e}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for GithubError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Network(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for GithubError {
    fn from(e: reqwest::Error) -> Self {
        Self::Network(e)
    }
}

impl From<io::Error> for GithubError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
