//! HTTP client for clankers plugins.
//!
//! Requires `"net"` permission in `plugin.json`. The host controls which
//! domains are reachable via `allowed_hosts`.

use std::collections::BTreeMap;

use extism_pdk::http as pdk_http;
use extism_pdk::HttpRequest;

/// HTTP response wrapper.
pub struct Response {
    /// HTTP status code.
    pub status: u16,
    body: Vec<u8>,
}

impl Response {
    /// Body as UTF-8 string (lossy).
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Parse body as JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        serde_json::from_slice(&self.body).map_err(|e| format!("JSON parse error: {e}"))
    }

    /// True if status is 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Perform an HTTP GET request.
pub fn get(url: &str) -> Result<Response, String> {
    request("GET", url, &BTreeMap::new(), None)
}

/// Perform an HTTP POST request with headers and body.
pub fn post(
    url: &str,
    headers: &BTreeMap<String, String>,
    body: &str,
) -> Result<Response, String> {
    request("POST", url, headers, Some(body))
}

/// General HTTP request.
pub fn request(
    method: &str,
    url: &str,
    headers: &BTreeMap<String, String>,
    body: Option<&str>,
) -> Result<Response, String> {
    let mut req = HttpRequest::new(url);
    req.method = Some(method.to_string());
    req.headers = headers.clone();

    let resp = pdk_http::request::<String>(&req, body.map(|b| b.to_string()))
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    Ok(Response {
        status: resp.status_code(),
        body: resp.body(),
    })
}
