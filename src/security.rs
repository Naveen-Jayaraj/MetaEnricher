use crate::AppState;
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use base64::prelude::*;
use moka::future::Cache;
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct RateLimiter {
    cache: Cache<String, Arc<AtomicU32>>,
    limit_per_minute: u32,
}

impl RateLimiter {
    pub fn new(limit_per_minute: u32) -> Self {
        let cache = Cache::builder()
            .time_to_live(Duration::from_secs(60))
            .max_capacity(10_000)
            .build();

        Self {
            cache,
            limit_per_minute,
        }
    }

    pub async fn check(&self, ip: &str) -> Result<u32, u32> {
        let counter = self
            .cache
            .get_with(ip.to_string(), async {
                Arc::new(AtomicU32::new(0))
            })
            .await;

        let count = counter.fetch_add(1, Ordering::Relaxed) + 1;
        if count > self.limit_per_minute {
            Err(count)
        } else {
            Ok(count)
        }
    }
}

#[derive(Clone)]
pub struct SecurityConfig {
    pub api_key: String,
    pub basic_auth_user: String,
    pub basic_auth_pass: String,
    pub rate_limiter: RateLimiter,
}

impl SecurityConfig {
    pub fn from_env() -> Self {
        let api_key = std::env::var("API_KEY")
            .or_else(|_| std::env::var("AUTH_TOKEN"))
            .unwrap_or_else(|_| "metaenricher_secret_key".to_string());

        let basic_auth_user = std::env::var("AUTH_USERNAME")
            .unwrap_or_else(|_| "admin".to_string());

        let basic_auth_pass = std::env::var("AUTH_PASSWORD")
            .unwrap_or_else(|_| api_key.clone());

        let rate_limit_per_minute: u32 = std::env::var("RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        tracing::info!(
            "Security Config initialized: Rate Limit = {} req/min, Auth = Enabled",
            rate_limit_per_minute
        );

        Self {
            api_key,
            basic_auth_user,
            basic_auth_pass,
            rate_limiter: RateLimiter::new(rate_limit_per_minute),
        }
    }

    pub fn validate_credentials(&self, header_val: &str) -> bool {
        let header_val = header_val.trim();

        // 1. Direct match with API key
        if header_val == self.api_key {
            return true;
        }

        // 2. Bearer Token
        if let Some(token) = header_val.strip_prefix("Bearer ") {
            if token.trim() == self.api_key {
                return true;
            }
        }

        // 3. Basic Auth: Basic <base64(user:pass)>
        if let Some(basic_b64) = header_val.strip_prefix("Basic ") {
            if let Ok(decoded) = BASE64_STANDARD.decode(basic_b64.trim()) {
                if let Ok(credentials) = String::from_utf8(decoded) {
                    if let Some((user, pass)) = credentials.split_once(':') {
                        if user == self.basic_auth_user && pass == self.basic_auth_pass {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }
}

/// Helper to extract best-effort client IP from headers (Cloudflare, Render, proxies)
fn extract_client_ip(req: &Request) -> String {
    let headers = req.headers();

    // 1. Cloudflare
    if let Some(cf_ip) = headers.get("cf-connecting-ip").and_then(|v| v.to_str().ok()) {
        return cf_ip.trim().to_string();
    }

    // 2. X-Forwarded-For (First IP in list)
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next() {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    // 3. X-Real-IP
    if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        return real_ip.trim().to_string();
    }

    "127.0.0.1".to_string()
}

/// Unified Security Middleware: Rate Limiting + Authentication
pub async fn security_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();

    // 1. Unrestricted Endpoints (Render Health Checks & Visual Dashboard Shell)
    if path == "/health" || path == "/api/health" || path == "/" {
        return next.run(req).await;
    }

    let client_ip = extract_client_ip(&req);

    // 2. Rate Limiting Check
    if let Err(count) = state.security.rate_limiter.check(&client_ip).await {
        tracing::warn!(
            "Rate limit exceeded for IP {}: {} requests in 60s",
            client_ip,
            count
        );
        let mut response = (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "success": false,
                "error": "Rate limit exceeded. Maximum requests per minute allowed.",
                "retryAfterSeconds": 60
            })),
        )
            .into_response();

        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
        return response;
    }

    // 3. Authentication Check
    let mut authenticated = false;

    // Check Header: X-API-Key
    if let Some(api_key_hdr) = req.headers().get("x-api-key").and_then(|v| v.to_str().ok()) {
        if api_key_hdr.trim() == state.security.api_key {
            authenticated = true;
        }
    }

    // Check Header: Authorization (Bearer or Basic)
    if !authenticated {
        if let Some(auth_hdr) = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
            if state.security.validate_credentials(auth_hdr) {
                authenticated = true;
            }
        }
    }

    // Check Query Parameter: ?key=... or ?api_key=... (Useful for direct .lrc downloads)
    if !authenticated {
        if let Some(query) = req.uri().query() {
            for pair in query.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    if (k == "key" || k == "api_key") && v == state.security.api_key {
                        authenticated = true;
                        break;
                    }
                }
            }
        }
    }

    if !authenticated {
        tracing::warn!(
            "Unauthorized access attempt to {} from IP {}",
            path,
            client_ip
        );
        let mut response = (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "success": false,
                "error": "Unauthorized. Please provide a valid 'X-API-Key' or 'Authorization' header."
            })),
        )
            .into_response();

        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"MetaEnricher\""),
        );
        return response;
    }

    // Authorized & Within Rate Limits -> Proceed
    next.run(req).await
}
