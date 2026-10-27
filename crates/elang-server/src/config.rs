//! Configuration parser for the `elang-server` binary.
//!
//! Extracted out of `main.rs` so each branch of the env-var parsing is
//! unit-testable without spinning up a real tokio runtime or touching the
//! process's environment (which `cargo test` runs in parallel and would
//! race).
//!
//! The parser is deliberately tolerant: any unparseable value falls back
//! to the corresponding default. The Hub treats unparseable or missing
//! configuration as a server-side config error, never as an SDK failure.

use std::collections::HashMap;

use crate::{
    state::AppState, DEFAULT_MAX_BODY_BYTES, DEFAULT_MAX_CONCURRENT_REQUESTS, DEFAULT_TIMEOUT_MS,
    MAX_BODY_BYTES_CEILING, MAX_CONCURRENT_REQUESTS_CEILING,
};

/// Bind address default if `ELANG_SERVER_PORT` is unset or invalid.
pub const DEFAULT_PORT: u16 = 8080;

/// Resolved server configuration, derived from the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// TCP listen port.
    pub port: u16,
    /// Request body cap; `413` above it.
    pub max_body_bytes: usize,
    /// Per-request handler budget; `504` above it.
    pub timeout_ms: u64,
    /// Max concurrent in-flight SDK requests.
    pub max_concurrent: usize,
}

impl Config {
    /// Defaults applied when every env var is unset.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            port: DEFAULT_PORT,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_concurrent: DEFAULT_MAX_CONCURRENT_REQUESTS,
        }
    }

    /// Pull a single env-var override. Missing or unparseable values fall
    /// back to `default`.
    fn parse_or<T: std::str::FromStr>(env: &HashMap<String, String>, key: &str, default: T) -> T {
        env.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }
}

/// Build a [`Config`] from an arbitrary `(key, value)` source — usually
/// `std::env::vars()` for `main`, but anything iterable works for tests.
#[must_use]
pub fn parse_config<I, K, V>(env: I) -> Config
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let map: HashMap<String, String> = env
        .into_iter()
        .map(|(k, v)| (k.as_ref().to_owned(), v.as_ref().to_owned()))
        .collect();
    let mut cfg = Config::defaults();

    cfg.port = Config::parse_or(&map, "ELANG_SERVER_PORT", DEFAULT_PORT);
    cfg.timeout_ms = Config::parse_or(&map, "ELANG_SERVER_TIMEOUT_MS", DEFAULT_TIMEOUT_MS).max(1);
    cfg.max_concurrent = Config::parse_or(
        &map,
        "ELANG_SERVER_MAX_CONCURRENT",
        DEFAULT_MAX_CONCURRENT_REQUESTS,
    )
    .clamp(1, MAX_CONCURRENT_REQUESTS_CEILING);

    // Body size: clamp into [1, MAX_BODY_BYTES_CEILING] even if the
    // env-var parse succeeded. This catches both `0` (which would
    // deadlock the `RequestBodyLimitLayer` on every request) and
    // absurdly large values that exceed VM memory.
    let raw_body = map
        .get("ELANG_SERVER_MAX_BODY_BYTES")
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_BODY_BYTES);
    cfg.max_body_bytes = raw_body.clamp(1, MAX_BODY_BYTES_CEILING);

    cfg
}

/// Build the [`AppState`] for a [`Config`].
#[must_use]
pub fn build_state(cfg: &Config) -> AppState {
    AppState::with_concurrency(cfg.timeout_ms, cfg.max_concurrent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply_when_no_env_vars_set() {
        let cfg = parse_config(std::iter::empty::<(&str, &str)>());
        assert_eq!(cfg, Config::defaults());
    }

    #[test]
    fn all_overrides_applied() {
        let cfg = parse_config([
            ("ELANG_SERVER_PORT", "9999"),
            ("ELANG_SERVER_MAX_BODY_BYTES", "5242880"),
            ("ELANG_SERVER_TIMEOUT_MS", "60000"),
            ("ELANG_SERVER_MAX_CONCURRENT", "8"),
        ]);
        assert_eq!(cfg.port, 9999);
        assert_eq!(cfg.max_body_bytes, 5_242_880);
        assert_eq!(cfg.timeout_ms, 60_000);
        assert_eq!(cfg.max_concurrent, 8);
    }

    #[test]
    fn unparseable_values_fall_back_to_defaults() {
        let cfg = parse_config([
            ("ELANG_SERVER_PORT", "not-a-number"),
            ("ELANG_SERVER_MAX_BODY_BYTES", "still-not-a-number"),
            ("ELANG_SERVER_TIMEOUT_MS", "garbage"),
            ("ELANG_SERVER_MAX_CONCURRENT", "nope"),
        ]);
        assert_eq!(cfg.port, DEFAULT_PORT);
        assert_eq!(cfg.max_body_bytes, DEFAULT_MAX_BODY_BYTES);
        assert_eq!(cfg.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert_eq!(cfg.max_concurrent, DEFAULT_MAX_CONCURRENT_REQUESTS);
    }

    #[test]
    fn max_body_bytes_clamped_to_floor_one() {
        let cfg = parse_config([("ELANG_SERVER_MAX_BODY_BYTES", "0")]);
        assert_eq!(cfg.max_body_bytes, 1);
    }

    #[test]
    fn max_body_bytes_clamped_to_ceiling() {
        // Use a value bigger than `MAX_BODY_BYTES_CEILING` but still
        // below `usize::MAX` so the string parse succeeds.
        let cfg = parse_config([("ELANG_SERVER_MAX_BODY_BYTES", "999999999999")]);
        assert_eq!(cfg.max_body_bytes, MAX_BODY_BYTES_CEILING);
    }

    #[test]
    fn timeout_and_concurrency_are_clamped_to_supported_bounds() {
        let cfg = parse_config([
            ("ELANG_SERVER_TIMEOUT_MS", "0"),
            ("ELANG_SERVER_MAX_CONCURRENT", "0"),
        ]);
        assert_eq!(cfg.timeout_ms, 1);
        assert_eq!(cfg.max_concurrent, 1);

        for limit in [MAX_CONCURRENT_REQUESTS_CEILING, usize::MAX] {
            let cfg = parse_config([("ELANG_SERVER_MAX_CONCURRENT", limit.to_string())]);
            assert_eq!(cfg.max_concurrent, MAX_CONCURRENT_REQUESTS_CEILING);
            assert_eq!(
                build_state(&cfg).permits().available_permits(),
                MAX_CONCURRENT_REQUESTS_CEILING
            );
        }
    }

    #[test]
    fn build_state_uses_timeout_and_concurrency_from_config() {
        let cfg = Config {
            port: DEFAULT_PORT,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            timeout_ms: 12_345,
            max_concurrent: 7,
        };
        let state = build_state(&cfg);
        assert_eq!(state.timeout_ms(), 12_345);
        assert_eq!(state.permits().available_permits(), 7);
    }
}
