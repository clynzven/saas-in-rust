//! Defence in depth: cross-site request checks, security headers and rate limiting.

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderValue, Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;
use crate::error::AppError;

/// Refuse state-changing requests that come from another site (CSRF).
/// Our cookie is already SameSite=Lax; checking Origin as well means we don't
/// depend on every browser getting that right.
pub async fn same_origin_only(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return next.run(req).await;
    }
    let headers = req.headers();
    let origin = headers
        .get(header::ORIGIN)
        .or_else(|| headers.get(header::REFERER))
        .and_then(|v| v.to_str().ok());
    let ours = &state.config.base_url;
    match origin {
        Some(o) if o == ours || o.starts_with(&format!("{ours}/")) => next.run(req).await,
        _ => {
            tracing::warn!(
                ?origin,
                path = loggable_path(req.uri().path()),
                "blocked cross-origin request"
            );
            AppError::Forbidden.into_response()
        }
    }
}

/// The path to write in logs. Cancel links carry a secret, so it's left out.
pub fn loggable_path(path: &str) -> &str {
    if path.starts_with("/cancel/") {
        "/cancel/[secret]"
    } else {
        path
    }
}

/// Headers that tell browsers to be strict with our pages.
pub async fn headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    let csp = "default-src 'self'; img-src 'self' data:; frame-ancestors 'none'; form-action 'self'; base-uri 'none'";
    for (name, value) in [
        (header::CONTENT_SECURITY_POLICY, csp),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::X_FRAME_OPTIONS, "DENY"),
        (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
    ] {
        h.entry(name).or_insert(HeaderValue::from_static(value));
    }
    res
}

/// The visitor's IP address, for rate limits. Behind a proxy (Fly.io, Caddy,
/// nginx) every connection comes from the proxy, so set TRUSTED_IP_HEADER to the
/// header the proxy fills in, like Fly-Client-IP. Without a proxy, leave it
/// unset: anyone can send that header themselves.
pub struct ClientIp(pub String);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Infallible> {
        let from_proxy = state
            .config
            .trusted_ip_header
            .as_deref()
            .and_then(|name| parts.headers.get(name))
            .and_then(|v| v.to_str().ok())
            // X-Forwarded-For grows to the right: the last entry is the one our proxy added.
            .and_then(|v| v.rsplit(',').next())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip().to_string());
        Ok(ClientIp(
            from_proxy.or(peer).unwrap_or_else(|| "unknown".into()),
        ))
    }
}

/// Keys longer than this are cut, so no one can make an entry huge.
const MAX_KEY_LEN: usize = 300;

/// A fixed-window counter: at most `max` attempts per key per `window`.
/// It lives in memory, which is right for one server. With several
/// instances, keep the counters in Postgres or Redis instead.
pub struct RateLimiter {
    max: u32,
    window: Duration,
    max_keys: usize,
    hits: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new(max: u32, window: Duration) -> Self {
        Self {
            max,
            window,
            max_keys: 10_000,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// How many keys to remember at most (a few megabytes at 10,000).
    pub fn with_max_keys(mut self, max_keys: usize) -> Self {
        self.max_keys = max_keys;
        self
    }

    /// Record an attempt. Returns false when the key is over its limit.
    pub fn allow(&self, key: &str) -> bool {
        let key: String = key.chars().take(MAX_KEY_LEN).collect();
        let now = Instant::now();
        let mut hits = self.hits.lock().expect("rate limiter lock");
        if hits.len() >= self.max_keys {
            hits.retain(|_, (start, _)| now.duration_since(*start) < self.window);
            // Still full: a flood of new keys. Refuse new ones instead of growing.
            if hits.len() >= self.max_keys && !hits.contains_key(&key) {
                return false;
            }
        }
        let entry = hits.entry(key).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= self.max
    }

    pub fn reset(&self, key: &str) {
        let key: String = key.chars().take(MAX_KEY_LEN).collect();
        self.hits.lock().expect("rate limiter lock").remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_per_key_and_resets() {
        let rl = RateLimiter::new(3, Duration::from_secs(60));
        assert!(rl.allow("a") && rl.allow("a") && rl.allow("a"));
        assert!(!rl.allow("a"), "fourth attempt is blocked");
        assert!(rl.allow("b"), "other keys are unaffected");
        rl.reset("a");
        assert!(rl.allow("a"));
    }

    #[test]
    fn window_expires() {
        let rl = RateLimiter::new(1, Duration::from_millis(20));
        assert!(rl.allow("a"));
        assert!(!rl.allow("a"));
        std::thread::sleep(Duration::from_millis(25));
        assert!(rl.allow("a"));
    }

    #[test]
    fn secrets_stay_out_of_logs() {
        assert_eq!(loggable_path("/cancel/abc123"), "/cancel/[secret]");
        assert_eq!(loggable_path("/b/kape-barbers"), "/b/kape-barbers");
    }

    #[test]
    fn memory_stays_bounded() {
        let rl = RateLimiter::new(5, Duration::from_secs(60)).with_max_keys(2);
        assert!(rl.allow("a") && rl.allow("b"));
        assert!(
            !rl.allow("c"),
            "full: new keys are refused while the old ones are fresh"
        );
        assert!(rl.allow("a"), "known keys still count normally");
        let long = "x".repeat(100_000);
        let rl = RateLimiter::new(1, Duration::from_secs(60));
        assert!(rl.allow(&long));
        assert!(!rl.allow(&long), "a long key is cut, not stored whole");
        assert_eq!(
            rl.hits.lock().unwrap().keys().next().unwrap().len(),
            MAX_KEY_LEN
        );
    }
}
