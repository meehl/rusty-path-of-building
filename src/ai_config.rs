//! Configuration for the AI assistant.
//!
//! Values are read from `ai.json` in the script directory and can be overridden with environment
//! variables:
//!
//! - `MARITACA_API_KEY`
//! - `MARITACA_BASE_URL`
//!
//! For private builds, an API key can also be baked into the binary by setting
//! `RPOB_EMBEDDED_API_KEY` while compiling. It is only used when no key is configured otherwise.
//!
//! Example `ai.json`:
//! ```json
//! { "api_key": "...", "base_url": "https://chat.maritaca.ai/api", "models": ["sabia-4", "sabiazinho-4"] }
//! ```
use reqwest::Url;
use std::{fmt, fs, path::Path};

const CONFIG_FILE_NAME: &str = "ai.json";
const DEFAULT_BASE_URL: &str = "https://chat.maritaca.ai/api";
const DEFAULT_MODELS: [&str; 2] = ["sabia-4", "sabiazinho-4"];
const MAX_MODELS: usize = 8;
const MAX_MODEL_NAME_CHARS: usize = 100;
/// API key baked into the binary at compile time (see the module documentation).
const EMBEDDED_API_KEY: Option<&str> = option_env!("RPOB_EMBEDDED_API_KEY");

#[derive(Clone)]
pub struct AiConfig {
    api_key: Option<String>,
    base_url: String,
    models: Vec<String>,
}

// The api key must never end up in logs.
impl fmt::Debug for AiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiConfig")
            .field("has_api_key", &self.api_key.is_some())
            .field("base_url", &self.base_url)
            .field("models", &self.models)
            .finish()
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: DEFAULT_BASE_URL.to_string(),
            models: DEFAULT_MODELS.iter().map(ToString::to_string).collect(),
        }
    }
}

impl AiConfig {
    /// Loads the config from `<dir>/ai.json` and the process environment.
    pub fn load(dir: &Path) -> Self {
        let file_contents = fs::read_to_string(dir.join(CONFIG_FILE_NAME)).ok();
        Self::from_sources(
            file_contents.as_deref(),
            |name| std::env::var(name).ok(),
            EMBEDDED_API_KEY,
        )
    }

    /// Builds a config from the (optional) contents of `ai.json`, an environment lookup and an
    /// optional embedded key. Environment variables take precedence over the file, which takes
    /// precedence over the embedded key.
    fn from_sources(
        file_contents: Option<&str>,
        env: impl Fn(&str) -> Option<String>,
        embedded_api_key: Option<&str>,
    ) -> Self {
        let mut config = Self {
            api_key: embedded_api_key
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .map(ToString::to_string),
            ..Self::default()
        };

        if let Some(contents) = file_contents {
            match serde_json::from_str::<serde_json::Value>(contents) {
                Ok(json) => config.apply_json(&json),
                Err(e) => log::warn!("Ignoring invalid {CONFIG_FILE_NAME}: {e}"),
            }
        }

        if let Some(key) = env("MARITACA_API_KEY").filter(|s| !s.trim().is_empty()) {
            config.api_key = Some(key.trim().to_string());
        }
        if let Some(url) = env("MARITACA_BASE_URL").filter(|s| !s.trim().is_empty()) {
            config.base_url = validated_base_url(&url, &config.base_url);
        }

        config
    }

    fn apply_json(&mut self, json: &serde_json::Value) {
        if let Some(key) = json["api_key"].as_str().filter(|s| !s.trim().is_empty()) {
            self.api_key = Some(key.trim().to_string());
        }
        if let Some(url) = json["base_url"].as_str().filter(|s| !s.trim().is_empty()) {
            self.base_url = validated_base_url(url, &self.base_url);
        }
        if let Some(models) = json["models"].as_array() {
            let models: Vec<String> = models
                .iter()
                .filter_map(|m| m.as_str())
                .map(str::trim)
                .filter(|m| !m.is_empty() && m.chars().count() <= MAX_MODEL_NAME_CHARS)
                .map(ToString::to_string)
                .take(MAX_MODELS)
                .collect();
            if !models.is_empty() {
                self.models = models;
            }
        }
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    pub fn models(&self) -> &[String] {
        &self.models
    }

    /// Full URL of the chat completions endpoint.
    pub fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

fn normalize_base_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn validated_base_url(url: &str, fallback: &str) -> String {
    let normalized = normalize_base_url(url);
    let Ok(parsed) = Url::parse(&normalized) else {
        log::warn!("Ignoring invalid AI base URL");
        return fallback.to_string();
    };
    let local = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    let secure = parsed.scheme() == "https";
    if parsed.host_str().is_none()
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.scheme(), "http" | "https")
        || (!secure && !local)
    {
        log::warn!("Ignoring insecure AI base URL; use HTTPS for remote endpoints");
        return fallback.to_string();
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn defaults_without_sources() {
        let config = AiConfig::from_sources(None, no_env, None);
        assert_eq!(config.api_key(), None);
        assert_eq!(
            config.chat_completions_url(),
            "https://chat.maritaca.ai/api/chat/completions"
        );
        assert_eq!(config.models(), ["sabia-4", "sabiazinho-4"]);
    }

    #[test]
    fn reads_file() {
        let file =
            r#"{ "api_key": " abc ", "base_url": "http://localhost:1/v1/", "models": ["m1"] }"#;
        let config = AiConfig::from_sources(Some(file), no_env, None);
        assert_eq!(config.api_key(), Some("abc"));
        assert_eq!(
            config.chat_completions_url(),
            "http://localhost:1/v1/chat/completions"
        );
        assert_eq!(config.models(), ["m1"]);
    }

    #[test]
    fn env_overrides_file() {
        let file = r#"{ "api_key": "from-file", "base_url": "http://file" }"#;
        let env = |name: &str| match name {
            "MARITACA_API_KEY" => Some("from-env".to_string()),
            "MARITACA_BASE_URL" => Some("https://env".to_string()),
            _ => None,
        };
        let config = AiConfig::from_sources(Some(file), env, None);
        assert_eq!(config.api_key(), Some("from-env"));
        assert_eq!(
            config.chat_completions_url(),
            "https://env/chat/completions"
        );
    }

    #[test]
    fn invalid_file_falls_back_to_defaults() {
        let config = AiConfig::from_sources(Some("not json"), no_env, None);
        assert_eq!(config.api_key(), None);
        assert_eq!(config.models().len(), 2);
    }

    #[test]
    fn blank_values_are_ignored() {
        let file = r#"{ "api_key": "  ", "models": [] }"#;
        let env = |_: &str| Some("   ".to_string());
        let config = AiConfig::from_sources(Some(file), env, None);
        assert_eq!(config.api_key(), None);
        assert_eq!(config.models().len(), 2);
    }

    #[test]
    fn embedded_key_is_the_last_resort() {
        let config = AiConfig::from_sources(None, no_env, Some(" embedded "));
        assert_eq!(config.api_key(), Some("embedded"));

        let file = r#"{ "api_key": "from-file" }"#;
        let config = AiConfig::from_sources(Some(file), no_env, Some("embedded"));
        assert_eq!(config.api_key(), Some("from-file"));

        let env = |name: &str| (name == "MARITACA_API_KEY").then(|| "from-env".to_string());
        let config = AiConfig::from_sources(Some(file), env, Some("embedded"));
        assert_eq!(config.api_key(), Some("from-env"));

        let config = AiConfig::from_sources(None, no_env, Some("  "));
        assert_eq!(config.api_key(), None);
    }

    #[test]
    fn debug_does_not_leak_key() {
        let config = AiConfig::from_sources(Some(r#"{ "api_key": "secret" }"#), no_env, None);
        assert!(!format!("{config:?}").contains("secret"));
    }

    #[test]
    fn rejects_remote_http_and_invalid_urls() {
        let file = r#"{ "base_url": "http://example.com/api" }"#;
        let config = AiConfig::from_sources(Some(file), no_env, None);
        assert_eq!(config.base_url(), DEFAULT_BASE_URL);

        let file = r#"{ "base_url": "not a url" }"#;
        let config = AiConfig::from_sources(Some(file), no_env, None);
        assert_eq!(config.base_url(), DEFAULT_BASE_URL);
    }

    #[test]
    fn accepts_local_http() {
        let file = r#"{ "base_url": "http://localhost:8080/v1/" }"#;
        let config = AiConfig::from_sources(Some(file), no_env, None);
        assert_eq!(config.base_url(), "http://localhost:8080/v1");
    }

    #[test]
    fn rejects_credentials_and_query_from_base_url() {
        for base_url in [
            "https://user:pass@example.com/api",
            "https://example.com/api?token=secret",
        ] {
            let file = format!(r#"{{ "base_url": "{base_url}" }}"#);
            let config = AiConfig::from_sources(Some(&file), no_env, None);
            assert_eq!(config.base_url(), DEFAULT_BASE_URL);
        }
    }
}
