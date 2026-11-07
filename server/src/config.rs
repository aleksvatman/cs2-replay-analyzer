//! Configuration read from environment variables (see `server-doc.md`, section 12).

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8080";
const DEFAULT_MAX_UPLOAD_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, PartialEq)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub data_dir: PathBuf,
    /// `None` is allowed only when the server listens on a loopback address.
    pub api_token: Option<String>,
    pub max_upload_bytes: u64,
    pub max_parallel_parses: usize,
    pub max_queue: usize,
    pub max_matches: usize,
    /// 0 keeps matches forever.
    pub retention_days: u32,
    pub keep_dem: bool,
}

// The token must never reach the logs, so Debug is written by hand.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("bind_addr", &self.bind_addr)
            .field("data_dir", &self.data_dir)
            .field("api_token", &self.api_token.as_ref().map(|_| "<set>"))
            .field("max_upload_bytes", &self.max_upload_bytes)
            .field("max_parallel_parses", &self.max_parallel_parses)
            .field("max_queue", &self.max_queue)
            .field("max_matches", &self.max_matches)
            .field("retention_days", &self.retention_days)
            .field("keep_dem", &self.keep_dem)
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    Invalid {
        var: &'static str,
        value: String,
        reason: &'static str,
    },
    /// A public address without a token would expose the server to everyone.
    TokenRequired(SocketAddr),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Invalid { var, value, reason } => {
                write!(f, "{var}={value:?}: {reason}")
            }
            ConfigError::TokenRequired(addr) => write!(
                f,
                "API_TOKEN is required when BIND_ADDR is not a loopback address (got {addr})"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    pub fn from_env() -> Result<Config, ConfigError> {
        Config::from_lookup(|name| std::env::var(name).ok())
    }

    /// Builds the config from any variable source (tests pass a closure).
    pub fn from_lookup<F>(get: F) -> Result<Config, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let bind_addr: SocketAddr = parse(
            &get,
            "BIND_ADDR",
            DEFAULT_BIND_ADDR.parse().unwrap(),
            "not a host:port address",
        )?;
        let data_dir = get("DATA_DIR")
            .filter(|v| !v.trim().is_empty())
            .map_or_else(|| PathBuf::from("./data"), PathBuf::from);
        let api_token = get("API_TOKEN").filter(|v| !v.is_empty());
        let max_upload_bytes = parse(
            &get,
            "MAX_UPLOAD_BYTES",
            DEFAULT_MAX_UPLOAD_BYTES,
            "not a number",
        )?;
        let max_parallel_parses: usize = parse(&get, "MAX_PARALLEL_PARSES", 1, "not a number")?;
        let max_queue = parse(&get, "MAX_QUEUE", 8, "not a number")?;
        let max_matches = parse(&get, "MAX_MATCHES", 200, "not a number")?;
        let retention_days = parse(&get, "RETENTION_DAYS", 0, "not a number")?;
        let keep_dem = parse_bool(&get, "KEEP_DEM", false)?;

        if max_upload_bytes == 0 {
            return Err(invalid(
                "MAX_UPLOAD_BYTES",
                max_upload_bytes,
                "must be at least 1",
            ));
        }
        if max_parallel_parses == 0 {
            return Err(invalid(
                "MAX_PARALLEL_PARSES",
                max_parallel_parses,
                "must be at least 1",
            ));
        }
        if api_token.is_none() && !bind_addr.ip().is_loopback() {
            return Err(ConfigError::TokenRequired(bind_addr));
        }

        Ok(Config {
            bind_addr,
            data_dir,
            api_token,
            max_upload_bytes,
            max_parallel_parses,
            max_queue,
            max_matches,
            retention_days,
            keep_dem,
        })
    }
}

fn invalid(var: &'static str, value: impl ToString, reason: &'static str) -> ConfigError {
    ConfigError::Invalid {
        var,
        value: value.to_string(),
        reason,
    }
}

fn parse<T, F>(
    get: &F,
    var: &'static str,
    default: T,
    reason: &'static str,
) -> Result<T, ConfigError>
where
    T: FromStr,
    F: Fn(&str) -> Option<String>,
{
    match get(var).filter(|v| !v.trim().is_empty()) {
        None => Ok(default),
        Some(raw) => raw.trim().parse().map_err(|_| invalid(var, raw, reason)),
    }
}

fn parse_bool<F>(get: &F, var: &'static str, default: bool) -> Result<bool, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    match get(var).filter(|v| !v.trim().is_empty()) {
        None => Ok(default),
        Some(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(invalid(var, raw, "expected true, false, 1 or 0")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_lookup(|name| map.get(name).cloned())
    }

    #[test]
    fn defaults_are_safe() {
        let c = load(&[]).unwrap();
        assert_eq!(c.bind_addr, "127.0.0.1:8080".parse().unwrap());
        assert_eq!(c.data_dir, PathBuf::from("./data"));
        assert_eq!(c.api_token, None);
        assert_eq!(c.max_upload_bytes, 1024 * 1024 * 1024);
        assert_eq!(c.max_parallel_parses, 1);
        assert_eq!(c.max_queue, 8);
        assert_eq!(c.max_matches, 200);
        assert_eq!(c.retention_days, 0);
        assert!(!c.keep_dem);
    }

    #[test]
    fn reads_every_variable() {
        let c = load(&[
            ("BIND_ADDR", "0.0.0.0:9000"),
            ("DATA_DIR", "/var/lib/cs2"),
            ("API_TOKEN", "secret"),
            ("MAX_UPLOAD_BYTES", "500"),
            ("MAX_PARALLEL_PARSES", "2"),
            ("MAX_QUEUE", "3"),
            ("MAX_MATCHES", "10"),
            ("RETENTION_DAYS", "30"),
            ("KEEP_DEM", "TRUE"),
        ])
        .unwrap();
        assert_eq!(c.bind_addr, "0.0.0.0:9000".parse().unwrap());
        assert_eq!(c.data_dir, PathBuf::from("/var/lib/cs2"));
        assert_eq!(c.api_token.as_deref(), Some("secret"));
        assert_eq!(c.max_upload_bytes, 500);
        assert_eq!(c.max_parallel_parses, 2);
        assert_eq!(c.max_queue, 3);
        assert_eq!(c.max_matches, 10);
        assert_eq!(c.retention_days, 30);
        assert!(c.keep_dem);
    }

    #[test]
    fn public_address_needs_a_token() {
        let err = load(&[("BIND_ADDR", "0.0.0.0:8080")]).unwrap_err();
        assert_eq!(
            err,
            ConfigError::TokenRequired("0.0.0.0:8080".parse().unwrap())
        );
        // an empty token counts as missing
        assert!(load(&[("BIND_ADDR", "0.0.0.0:8080"), ("API_TOKEN", "")]).is_err());
        // loopback (v4 and v6) is fine without one
        assert!(load(&[("BIND_ADDR", "127.0.0.1:1")]).is_ok());
        assert!(load(&[("BIND_ADDR", "[::1]:1")]).is_ok());
    }

    #[test]
    fn rejects_bad_values() {
        for (var, value) in [
            ("BIND_ADDR", "nonsense"),
            ("MAX_UPLOAD_BYTES", "abc"),
            ("MAX_UPLOAD_BYTES", "0"),
            ("MAX_PARALLEL_PARSES", "0"),
            ("MAX_QUEUE", "-1"),
            ("KEEP_DEM", "maybe"),
        ] {
            let err = load(&[(var, value)]).unwrap_err();
            assert!(
                matches!(err, ConfigError::Invalid { var: v, .. } if v == var),
                "{var}={value}"
            );
        }
    }

    #[test]
    fn debug_hides_the_token() {
        let c = load(&[("API_TOKEN", "super-secret")]).unwrap();
        let text = format!("{c:?}");
        assert!(!text.contains("super-secret"));
        assert!(text.contains("<set>"));
    }
}
