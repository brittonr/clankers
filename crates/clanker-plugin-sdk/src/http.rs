//! HTTP client for clankers plugins.
//!
//! Requires `"net"` permission in `plugin.json`. The host controls which
//! domains are reachable via `allowed_hosts`.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

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
