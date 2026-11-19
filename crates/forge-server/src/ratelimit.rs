//! In-memory token-bucket rate limiting (spec: openapi + roadmap
//! Step 6). Keyed by a hash of the `Authorization` header when
//! present, else the client IP. Single-node only — a multi-node
//! deployment needs a shared store (accepted limitation, recorded in
//! the threat model).

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Bucket parameters: `limit` requests per `window`.
#[derive(Debug, Clone, Copy)]
pub struct RateLimit {
    /// Requests allowed per window.
    pub limit: u32,
    /// Window length.
    pub window: Duration,
}

impl RateLimit {
    /// `n` requests per minute.
    #[must_use]
    pub fn per_minute(n: u32) -> Self {
        Self {
            limit: n,
            window: Duration::from_secs(60),
        }
    }
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    updated: Instant,
}

/// Shared limiter state.
#[derive(Debug)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
    config: RateLimit,
}

impl RateLimiter {
    /// New limiter.
    #[must_use]
    pub fn new(config: RateLimit) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            config,
        }
    }

    /// The configured limit (for headers).
    #[must_use]
    pub fn limit(&self) -> u32 {
        self.config.limit
    }

    /// Try to take one token for `key`. Returns `(allowed, remaining,
    /// seconds_until_a_token)`.
    pub fn check(&self, key: &str) -> (bool, u32, u64) {
        let now = Instant::now();
        let rate = f64::from(self.config.limit) / self.config.window.as_secs_f64();
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Opportunistic cleanup so the map cannot grow forever.
        if buckets.len() > 10_000 {
            buckets.retain(|_, b| now.duration_since(b.updated) < self.config.window * 2);
        }
        let bucket = buckets.entry(key.to_string()).or_insert_with(|| Bucket {
            tokens: f64::from(self.config.limit),
            updated: now,
        });
        let elapsed = now.duration_since(bucket.updated).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * rate).min(f64::from(self.config.limit));
        bucket.updated = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let remaining = bucket.tokens.floor() as u32;
            (true, remaining, 0)
        } else {
            let wait = ((1.0 - bucket.tokens) / rate).ceil();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let wait_secs = wait as u64;
            (false, 0, wait_secs.max(1))
        }
    }
}

/// The rate-limit key for a request: a hash of the Authorization
/// header (never the raw token) or the client IP.
fn key_for(headers: &HeaderMap, addr: Option<SocketAddr>) -> String {
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(value) = auth.to_str() {
            let mut hasher = Sha256::new();
            hasher.update(value.as_bytes());
            return format!("auth:{:x}", hasher.finalize());
        }
    }
    addr.map_or_else(|| "ip:unknown".to_string(), |a| format!("ip:{}", a.ip()))
}

/// Axum middleware enforcing the limiter and adding
/// `RateLimit-*` headers (and `Retry-After` on 429).
pub async fn rate_limit_layer(
    State(limiter): State<Arc<RateLimiter>>,
    req: Request,
    next: Next,
) -> Response {
    // `axum::serve … with_connect_info` puts the peer address in the
    // request extensions.
    let addr = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0);
    let key = key_for(req.headers(), addr);
    let (allowed, remaining, retry_after) = limiter.check(&key);
    let limit = limiter.limit();
    if !allowed {
        let mut resp = crate::error::ApiError::rate_limited().into_response();
        let headers = resp.headers_mut();
        headers.insert("ratelimit-limit", HeaderValue::from(limit));
        headers.insert("ratelimit-remaining", HeaderValue::from(0));
        headers.insert("retry-after", HeaderValue::from(retry_after));
        return resp;
    }
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();
    headers.insert("ratelimit-limit", HeaderValue::from(limit));
    headers.insert("ratelimit-remaining", HeaderValue::from(remaining));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_allows_up_to_the_limit_then_blocks() {
        let limiter = RateLimiter::new(RateLimit::per_minute(3));
        for i in 0..3 {
            let (ok, remaining, _) = limiter.check("k");
            assert!(ok, "request {i} should pass");
            assert_eq!(remaining, 2 - i);
        }
        let (ok, remaining, retry) = limiter.check("k");
        assert!(!ok);
        assert_eq!(remaining, 0);
        assert!(retry >= 1);
        // A different key has its own bucket.
        assert!(limiter.check("other").0);
    }

    #[test]
    fn keys_hash_authorization_and_fall_back_to_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer fgh_secret"),
        );
        let key = key_for(&headers, None);
        assert!(key.starts_with("auth:"), "{key}");
        assert!(!key.contains("secret"), "raw token must never be a key");
        let ip_key = key_for(&HeaderMap::new(), Some("10.0.0.1:1234".parse().unwrap()));
        assert_eq!(ip_key, "ip:10.0.0.1");
    }
}
