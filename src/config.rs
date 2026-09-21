//! Runtime configuration loaded from environment variables.

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use url::Url;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:3000";
const DEFAULT_MEMPOOL_URL: &str =
    "https://mempool.space/api/v1/lightning/nodes/rankings/connectivity";
const DEFAULT_IMPORT_INTERVAL_SECS: u64 = 60;
const DEFAULT_HTTP_CLIENT_TIMEOUT_SECS: u64 = 10;

/// Validated runtime configuration.
#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub mempool_url: Url,
    pub import_interval: Duration,
    pub http_client_timeout: Duration,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("missing required environment variable {0}")]
    Missing(&'static str),
    #[error("invalid value {value:?} for {key}: {reason}")]
    Invalid {
        key: &'static str,
        value: String,
        reason: String,
    },
}

impl Config {
    /// Loads the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Loads the configuration using `lookup` to resolve each variable.
    ///
    /// Taking a closure instead of reading the environment directly lets tests
    /// supply values without mutating global process state. Empty values are
    /// treated as unset, so `FOO=` in a compose file falls back to the default.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let get = |key: &str| lookup(key).filter(|value| !value.is_empty());

        let database_url = get("DATABASE_URL").ok_or(ConfigError::Missing("DATABASE_URL"))?;

        let bind_addr =
            parse_or_default("BIND_ADDR", get("BIND_ADDR"), DEFAULT_BIND_ADDR, |raw| {
                raw.parse::<SocketAddr>().map_err(|err| err.to_string())
            })?;

        let mempool_url = parse_or_default(
            "MEMPOOL_URL",
            get("MEMPOOL_URL"),
            DEFAULT_MEMPOOL_URL,
            parse_http_url,
        )?;

        let import_interval = parse_positive_secs(
            "IMPORT_INTERVAL_SECS",
            get("IMPORT_INTERVAL_SECS"),
            DEFAULT_IMPORT_INTERVAL_SECS,
        )?;

        let http_client_timeout = parse_positive_secs(
            "HTTP_CLIENT_TIMEOUT_SECS",
            get("HTTP_CLIENT_TIMEOUT_SECS"),
            DEFAULT_HTTP_CLIENT_TIMEOUT_SECS,
        )?;

        Ok(Self {
            database_url,
            bind_addr,
            mempool_url,
            import_interval,
            http_client_timeout,
        })
    }
}

// Manual impl so the database password never ends up in logs.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("database_url", &redact_database_url(&self.database_url))
            .field("bind_addr", &self.bind_addr)
            .field("mempool_url", &self.mempool_url.as_str())
            .field("import_interval", &self.import_interval)
            .field("http_client_timeout", &self.http_client_timeout)
            .finish()
    }
}

/// Replaces the password in a database URL with `***`.
///
/// An unparsable URL is fully hidden: it might still contain a secret.
pub fn redact_database_url(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(mut url) => {
            if url.password().is_some() && url.set_password(Some("***")).is_err() {
                return "<redacted>".to_owned();
            }
            url.to_string()
        }
        Err(_) => "<unparsable>".to_owned(),
    }
}

fn parse_or_default<T>(
    key: &'static str,
    value: Option<String>,
    default: &str,
    parse: impl Fn(&str) -> Result<T, String>,
) -> Result<T, ConfigError> {
    let raw = value.unwrap_or_else(|| default.to_owned());
    parse(&raw).map_err(|reason| ConfigError::Invalid {
        key,
        value: raw,
        reason,
    })
}

fn parse_http_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|err| err.to_string())?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        other => Err(format!(
            "unsupported scheme {other:?}, expected http or https"
        )),
    }
}

fn parse_positive_secs(
    key: &'static str,
    value: Option<String>,
    default: u64,
) -> Result<Duration, ConfigError> {
    let Some(raw) = value else {
        return Ok(Duration::from_secs(default));
    };
    match raw.parse::<u64>() {
        Ok(0) => Err(ConfigError::Invalid {
            key,
            value: raw,
            reason: "must be greater than zero".to_owned(),
        }),
        Ok(secs) => Ok(Duration::from_secs(secs)),
        Err(err) => Err(ConfigError::Invalid {
            key,
            value: raw,
            reason: err.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const DB_URL: &str = "postgres://bipa:secret@localhost:5432/bipa";

    fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Config::from_lookup(|key| map.get(key).cloned())
    }

    fn invalid_key(result: Result<Config, ConfigError>) -> &'static str {
        match result {
            Err(ConfigError::Invalid { key, .. }) => key,
            other => panic!("expected ConfigError::Invalid, got {other:?}"),
        }
    }

    #[test]
    fn all_variables_provided() {
        let config = load(&[
            ("DATABASE_URL", DB_URL),
            ("BIND_ADDR", "127.0.0.1:8080"),
            ("MEMPOOL_URL", "http://localhost:9999/nodes"),
            ("IMPORT_INTERVAL_SECS", "5"),
            ("HTTP_CLIENT_TIMEOUT_SECS", "2"),
        ])
        .unwrap();

        assert_eq!(config.database_url, DB_URL);
        assert_eq!(config.bind_addr, "127.0.0.1:8080".parse().unwrap());
        assert_eq!(config.mempool_url.as_str(), "http://localhost:9999/nodes");
        assert_eq!(config.import_interval, Duration::from_secs(5));
        assert_eq!(config.http_client_timeout, Duration::from_secs(2));
    }

    #[test]
    fn defaults_apply_when_only_database_url_is_set() {
        let config = load(&[("DATABASE_URL", DB_URL)]).unwrap();

        assert_eq!(config.bind_addr, "0.0.0.0:3000".parse().unwrap());
        assert_eq!(
            config.mempool_url.as_str(),
            "https://mempool.space/api/v1/lightning/nodes/rankings/connectivity"
        );
        assert_eq!(config.import_interval, Duration::from_secs(60));
        assert_eq!(config.http_client_timeout, Duration::from_secs(10));
    }

    #[test]
    fn empty_optional_values_fall_back_to_defaults() {
        let config = load(&[("DATABASE_URL", DB_URL), ("IMPORT_INTERVAL_SECS", "")]).unwrap();
        assert_eq!(config.import_interval, Duration::from_secs(60));
    }

    #[test]
    fn missing_database_url() {
        assert_eq!(load(&[]), Err(ConfigError::Missing("DATABASE_URL")));
    }

    #[test]
    fn empty_database_url_is_treated_as_missing() {
        assert_eq!(
            load(&[("DATABASE_URL", "")]),
            Err(ConfigError::Missing("DATABASE_URL"))
        );
    }

    #[test]
    fn non_numeric_interval() {
        let result = load(&[("DATABASE_URL", DB_URL), ("IMPORT_INTERVAL_SECS", "abc")]);
        assert_eq!(invalid_key(result), "IMPORT_INTERVAL_SECS");
    }

    #[test]
    fn negative_interval() {
        let result = load(&[("DATABASE_URL", DB_URL), ("IMPORT_INTERVAL_SECS", "-1")]);
        assert_eq!(invalid_key(result), "IMPORT_INTERVAL_SECS");
    }

    #[test]
    fn zero_interval() {
        let result = load(&[("DATABASE_URL", DB_URL), ("IMPORT_INTERVAL_SECS", "0")]);
        assert_eq!(invalid_key(result), "IMPORT_INTERVAL_SECS");
    }

    #[test]
    fn zero_timeout() {
        let result = load(&[("DATABASE_URL", DB_URL), ("HTTP_CLIENT_TIMEOUT_SECS", "0")]);
        assert_eq!(invalid_key(result), "HTTP_CLIENT_TIMEOUT_SECS");
    }

    #[test]
    fn invalid_bind_address() {
        let result = load(&[("DATABASE_URL", DB_URL), ("BIND_ADDR", "not-an-address")]);
        assert_eq!(invalid_key(result), "BIND_ADDR");
    }

    #[test]
    fn invalid_mempool_url() {
        let result = load(&[("DATABASE_URL", DB_URL), ("MEMPOOL_URL", "not a url")]);
        assert_eq!(invalid_key(result), "MEMPOOL_URL");
    }

    #[test]
    fn non_http_mempool_url() {
        let result = load(&[
            ("DATABASE_URL", DB_URL),
            ("MEMPOOL_URL", "ftp://example.com"),
        ]);
        assert_eq!(invalid_key(result), "MEMPOOL_URL");
    }

    #[test]
    fn error_message_names_the_variable() {
        let err = load(&[("DATABASE_URL", DB_URL), ("IMPORT_INTERVAL_SECS", "0")]).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("IMPORT_INTERVAL_SECS"), "{message}");
        assert!(message.contains("greater than zero"), "{message}");
    }

    #[test]
    fn redacts_password() {
        assert_eq!(
            redact_database_url(DB_URL),
            "postgres://bipa:***@localhost:5432/bipa"
        );
    }

    #[test]
    fn keeps_url_without_password() {
        assert_eq!(
            redact_database_url("postgres://bipa@localhost/bipa"),
            "postgres://bipa@localhost/bipa"
        );
    }

    #[test]
    fn hides_unparsable_url() {
        assert_eq!(redact_database_url("::not a url secret::"), "<unparsable>");
    }

    #[test]
    fn debug_output_does_not_leak_password() {
        let config = load(&[("DATABASE_URL", DB_URL)]).unwrap();
        let debug = format!("{config:?}");
        assert!(!debug.contains("secret"), "{debug}");
        assert!(debug.contains("***"), "{debug}");
    }
}
