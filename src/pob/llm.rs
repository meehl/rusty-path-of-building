use crate::ai_config::AiConfig;
use reqwest::{blocking::Client, header::CONTENT_TYPE};
use std::{
    collections::HashSet,
    sync::mpsc::{Receiver, Sender, channel},
    thread,
    time::Duration,
};

/// LLM responses can take a while, especially for long answers.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_BODY_BYTES: usize = 256 * 1024;
const MAX_CONCURRENT_REQUESTS: usize = 2;
const MAX_ATTEMPTS: usize = 3;
const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(500);
/// Maximum number of characters of an error response body that is reported back.
const MAX_ERROR_BODY_CHARS: usize = 500;

/// The outcome of a finished chat completion request.
#[derive(Debug)]
pub struct LlmResult {
    pub id: u64,
    /// Raw response body on success, human readable error message on failure.
    pub outcome: Result<String, String>,
}

/// Sends chat completion requests on background threads and collects their results.
///
/// The API key stays on the Rust side: Lua only provides the request body.
pub struct LlmManager {
    config: AiConfig,
    next_id: u64,
    pending: usize,
    tx: Sender<LlmResult>,
    rx: Receiver<LlmResult>,
    cancelled: HashSet<u64>,
}

impl LlmManager {
    pub fn new(config: AiConfig) -> Self {
        let (tx, rx) = channel();
        Self {
            config,
            next_id: 0,
            pending: 0,
            tx,
            rx,
            cancelled: HashSet::new(),
        }
    }

    pub fn config(&self) -> &AiConfig {
        &self.config
    }

    /// Starts a request with the given JSON `body` and returns the id its result will carry.
    pub fn send(&mut self, body: String) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.pending += 1;

        let tx = self.tx.clone();
        if body.len() > MAX_BODY_BYTES {
            let _ = tx.send(LlmResult {
                id,
                outcome: Err("AI request is too large.".to_string()),
            });
            return id;
        }
        if self.pending > MAX_CONCURRENT_REQUESTS {
            let _ = tx.send(LlmResult {
                id,
                outcome: Err("Too many AI requests are in progress.".to_string()),
            });
            return id;
        }
        let Some(api_key) = self.config.api_key().map(ToString::to_string) else {
            let _ = tx.send(LlmResult {
                id,
                outcome: Err(
                    "No API key configured. Set MARITACA_API_KEY or add \"api_key\" to ai.json"
                        .to_string(),
                ),
            });
            return id;
        };

        let url = self.config.chat_completions_url();
        thread::spawn(move || {
            profiling::register_thread!(format!("LLM Request {id} Thread"));
            let outcome = post_chat(&url, &api_key, body);
            let _ = tx.send(LlmResult { id, outcome });
        });

        id
    }

    /// Returns all results that arrived since the last call.
    pub fn poll(&mut self) -> Vec<LlmResult> {
        let results: Vec<LlmResult> = self.rx.try_iter().collect();
        self.pending -= results.len();
        results
            .into_iter()
            .filter(|result| !self.cancelled.remove(&result.id))
            .collect()
    }

    /// Discards a request's eventual result. The blocking HTTP operation cannot be interrupted,
    /// but obsolete answers are never delivered to Lua.
    pub fn cancel(&mut self, id: u64) {
        self.cancelled.insert(id);
    }

    /// Returns `true` while at least one request has not delivered its result yet.
    pub fn has_pending(&self) -> bool {
        self.pending > 0
    }
}

fn post_chat(url: &str, api_key: &str, body: String) -> Result<String, String> {
    let client = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let mut last_error = String::from("Request failed");
    for attempt in 0..MAX_ATTEMPTS {
        let response = client
            .post(url)
            .bearer_auth(api_key)
            .header(CONTENT_TYPE, "application/json")
            .body(body.clone())
            .send();

        match response {
            Ok(response) => {
                let status = response.status();
                let text = response
                    .text()
                    .map_err(|e| format!("Failed to read response: {e}"))?;
                if status.is_success() {
                    return Ok(text);
                }
                let snippet: String = text.chars().take(MAX_ERROR_BODY_CHARS).collect();
                last_error = format!("HTTP {}: {snippet}", status.as_u16());
                if !status.is_server_error() && status.as_u16() != 408 && status.as_u16() != 429 {
                    break;
                }
            }
            Err(error) => {
                last_error = format!("Request failed: {error}");
            }
        }
        if attempt + 1 < MAX_ATTEMPTS {
            thread::sleep(INITIAL_RETRY_DELAY * 2u32.pow(attempt as u32));
        }
    }
    Err(last_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Instant,
    };

    /// Serves a single HTTP request with the given status line and body. Returns the base URL
    /// and a handle yielding the raw request that was received.
    fn serve_once(
        status_line: &'static str,
        body: &'static str,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 1024];
            loop {
                let n = stream.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request);
                if let Some(header_end) = text.find("\r\n\r\n") {
                    let content_length = text[..header_end]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if request.len() >= header_end + 4 + content_length || n == 0 {
                        break;
                    }
                } else if n == 0 {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&request).into_owned()
        });
        (base_url, handle)
    }

    fn wait_for_result(manager: &mut LlmManager) -> LlmResult {
        let start = Instant::now();
        loop {
            if let Some(result) = manager.poll().pop() {
                return result;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "timed out");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn success_returns_body_and_sends_auth_header() {
        let (base_url, request) = serve_once("200 OK", r#"{"ok":true}"#);
        let result = post_chat(
            &format!("{base_url}/chat/completions"),
            "sk-test",
            r#"{"a":1}"#.into(),
        );
        assert_eq!(result.unwrap(), r#"{"ok":true}"#);

        let request = request.join().unwrap();
        assert!(request.starts_with("POST /chat/completions"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer sk-test")
        );
        assert!(request.ends_with(r#"{"a":1}"#));
    }

    #[test]
    fn http_error_is_reported() {
        let (base_url, _request) = serve_once("401 Unauthorized", r#"{"error":"bad key"}"#);
        let err = post_chat(&format!("{base_url}/x"), "k", String::new()).unwrap_err();
        assert!(err.starts_with("HTTP 401"), "{err}");
        assert!(err.contains("bad key"));
    }

    #[test]
    fn connection_error_is_reported() {
        // nothing listens on port 1
        let err = post_chat("http://127.0.0.1:1/x", "k", String::new()).unwrap_err();
        assert!(err.starts_with("Request failed"), "{err}");
    }

    #[test]
    fn manager_without_key_reports_error() {
        let mut manager = LlmManager::new(AiConfig::default());
        let id = manager.send("{}".into());
        assert!(manager.has_pending());
        let result = wait_for_result(&mut manager);
        assert_eq!(result.id, id);
        assert!(result.outcome.unwrap_err().contains("No API key"));
        assert!(!manager.has_pending());
    }

    #[test]
    fn cancelled_result_is_not_delivered() {
        let mut manager = LlmManager::new(AiConfig::default());
        let id = manager.send("{}".into());
        manager.cancel(id);
        thread::sleep(Duration::from_millis(10));
        assert!(manager.poll().is_empty());
        assert!(!manager.has_pending());
    }

    #[test]
    fn oversized_body_is_rejected() {
        let mut manager = LlmManager::new(AiConfig::default());
        let id = manager.send("x".repeat(MAX_BODY_BYTES + 1));
        let result = wait_for_result(&mut manager);
        assert_eq!(result.id, id);
        assert!(result.outcome.unwrap_err().contains("too large"));
    }
}
