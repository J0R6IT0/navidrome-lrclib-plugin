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

#[cfg(test)]
mod tests {
    use super::*;

    fn response(headers: &[(&str, &str)]) -> Response {
        Response {
            status: 429,
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: Vec::new(),
        }
    }

    #[track_caller]
    fn check_retry_after(raw: &str, expected: Option<i64>) {
        assert_eq!(parse_retry_after(raw), expected, "retry-after from {raw:?}");
    }

    #[track_caller]
    fn check_url(request: Http, expected: &str) {
        assert_eq!(request.url().unwrap(), expected);
    }

    #[test]
    fn a_wait_in_seconds_is_understood() {
        check_retry_after("30", Some(30));
        check_retry_after("  30  ", Some(30));
        check_retry_after("0", Some(0));
    }

    #[test]
    fn a_wait_we_cannot_read_is_left_to_the_caller() {
        check_retry_after("", None);
        check_retry_after("Wed, 21 Oct 2015 07:28:00 GMT", None);
        check_retry_after("30s", None);
        check_retry_after("1.5", None);
    }

    #[test]
    fn headers_are_found_however_the_server_cased_them() {
        for name in ["Retry-After", "retry-after", "RETRY-AFTER"] {
            assert_eq!(response(&[(name, "12")]).header("Retry-After"), Some("12"));
        }
    }

    #[test]
    fn a_header_nobody_sent_is_absent() {
        assert_eq!(
            response(&[("Content-Type", "text/html")]).header("Retry-After"),
            None
        );
    }

    #[test]
    fn a_rate_limit_honours_the_advertised_wait() {
        assert_eq!(
            response(&[("retry-after", "45")]).rate_limited(),
            ProviderError::RateLimited {
                retry_after_secs: 45
            }
        );
    }

    #[test]
    fn a_rate_limit_without_a_wait_still_backs_off() {
        assert!(matches!(
            response(&[]).rate_limited(),
            ProviderError::RateLimited { retry_after_secs } if retry_after_secs > 0
        ));
    }

    #[test]
    fn a_request_without_params_keeps_its_url() {
        check_url(
            Http::get("https://lrclib.net/api/search"),
            "https://lrclib.net/api/search",
        );
    }

    #[test]
    fn params_are_appended_in_the_order_they_were_added() {
        check_url(
            Http::get("https://lrclib.net/api/get")
                .param("artist_name", "Queen")
                .param("track_name", "Bohemian Rhapsody"),
            "https://lrclib.net/api/get?artist_name=Queen&track_name=Bohemian+Rhapsody",
        );
    }

    #[test]
    fn a_param_escapes_what_a_url_cannot_carry() {
        check_url(
            Http::get("https://api.example/search")
                .param("q", "AC/DC & friends")
                .param("empty", ""),
            "https://api.example/search?q=AC%2FDC+%26+friends&empty=",
        );
    }
}
