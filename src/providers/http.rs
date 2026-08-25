use crate::providers::{BROWSER_USER_AGENT, ProviderResult, USER_AGENT, error::ProviderError};
use nd_pdk::host::http::{self, HTTPRequest};
use serde::de::DeserializeOwned;
use std::{borrow::Cow, collections::HashMap};

const TIMEOUT_MS: i32 = 15_000;

pub struct Http {
    method: &'static str,
    base_url: String,
    params: Vec<(String, String)>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Http {
    pub fn get(url: impl Into<String>) -> Self {
        Self::new("GET", url)
    }

    pub fn post(url: impl Into<String>) -> Self {
        Self::new("POST", url)
    }

    fn new(method: &'static str, url: impl Into<String>) -> Self {
        Self {
            method,
            base_url: url.into(),
            params: Vec::new(),
            headers: HashMap::from([("User-Agent".to_string(), USER_AGENT.to_string())]),
            body: Vec::new(),
        }
    }

    pub fn param(mut self, name: &str, value: impl Into<String>) -> Self {
        self.params.push((name.to_string(), value.into()));
        self
    }

    pub fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.insert(name.to_string(), value.into());
        self
    }

    pub fn headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers.extend(headers);
        self
    }

    /// Sites that gate on the plugin's own user agent are served as a browser.
    pub fn browser(self) -> Self {
        self.header("User-Agent", BROWSER_USER_AGENT)
    }

    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    pub fn url(&self) -> ProviderResult<String> {
        if self.params.is_empty() {
            return Ok(self.base_url.clone());
        }

        let query = serde_urlencoded::to_string(&self.params)
            .map_err(|e| ProviderError::other(format!("failed to encode the query string: {e}")))?;

        Ok(format!("{}?{query}", self.base_url))
    }

    pub fn send(self) -> ProviderResult<Response> {
        let response = http::send(HTTPRequest {
            method: self.method.to_string(),
            url: self.url()?,
            headers: self.headers,
            no_follow_redirects: false,
            body: self.body,
            timeout_ms: TIMEOUT_MS,
        })
        .map_err(|e| ProviderError::other(format!("HTTP request failed: {e}")))?
        .ok_or_else(|| ProviderError::other("received an empty HTTP response"))?;

        Ok(Response {
            status: response.status_code,
            headers: response.headers,
            body: response.body,
        })
    }
}

pub struct Response {
    pub status: i32,
    headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.body)
    }

    pub fn json<T: DeserializeOwned>(&self, what: &str) -> ProviderResult<T> {
        serde_json::from_slice(&self.body)
            .map_err(|e| ProviderError::other(format!("failed to parse the {what} response: {e}")))
    }

    pub fn unexpected_status(&self, what: &str) -> ProviderError {
        ProviderError::other(format!("{what} returned status {}", self.status))
    }

    pub fn rate_limited(&self) -> ProviderError {
        ProviderError::rate_limited(self.header("Retry-After").and_then(parse_retry_after))
    }
}

// TODO: support also the date format
fn parse_retry_after(value: &str) -> Option<i64> {
    value.trim().parse().ok()
}
