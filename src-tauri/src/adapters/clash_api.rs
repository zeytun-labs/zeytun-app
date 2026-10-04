use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct ClashApiClient {
    controller: String,
    secret: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClashProxyInfo {
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub name: String,
    pub now: Option<String>,
    pub all: Option<Vec<String>>,
    pub history: Vec<ClashProxyHistory>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClashProxyHistory {
    pub time: String,
    pub delay: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ClashDelayResult {
    pub delay: Option<u64>,
}

impl<'de> serde::Deserialize<'de> for ClashDelayResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum WireDelay {
            Number(u64),
            Object { delay: Option<u64> },
        }

        match WireDelay::deserialize(deserializer)? {
            WireDelay::Number(delay) => Ok(Self { delay: Some(delay) }),
            WireDelay::Object { delay } => Ok(Self { delay }),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClashProxiesResponse {
    pub proxies: HashMap<String, ClashProxyInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClashDnsQueryResponse {
    #[serde(rename = "Status")]
    pub status: u16,
    #[serde(default, rename = "Answer")]
    pub answer: Vec<ClashDnsAnswer>,
    #[serde(rename = "Server")]
    pub server: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClashDnsAnswer {
    #[serde(rename = "Type", alias = "type")]
    pub record_type: u16,
    #[serde(rename = "TTL", alias = "ttl")]
    pub ttl: u32,
    #[serde(rename = "Data", alias = "data")]
    pub data: String,
}

#[derive(Debug, Clone)]
pub enum ClashApiError {
    RequestFailed(String),
    NotExpected(String),
    NotRunning,
}

impl std::fmt::Display for ClashApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Plain text, no variant name: this string is what the user reads in a
        // toast, and `{e:?}` renders as `RequestFailed("…")` with nested quotes.
        match self {
            Self::RequestFailed(detail) | Self::NotExpected(detail) => write!(f, "{detail}"),
            Self::NotRunning => write!(f, "the core is not running"),
        }
    }
}

impl std::error::Error for ClashApiError {}

impl ClashApiClient {
    pub fn new(controller: String, secret: Option<String>) -> Self {
        Self { controller, secret }
    }

    pub fn controller(&self) -> &str {
        &self.controller
    }

    pub fn secret(&self) -> Option<&str> {
        self.secret.as_deref()
    }

    pub fn is_available(&self) -> Result<bool, ClashApiError> {
        let response = self.get("/proxies")?;
        Ok(response.status() == 200)
    }

    pub fn get_proxies(&self) -> Result<ClashProxiesResponse, ClashApiError> {
        let response = self.get("/proxies")?;
        let body = response
            .into_string()
            .map_err(|e| ClashApiError::RequestFailed(e.to_string()))?;
        serde_json::from_str(&body)
            .map_err(|e| ClashApiError::NotExpected(format!("invalid proxies response: {e}")))
    }

    pub fn select_proxy(&self, group_name: &str, proxy_name: &str) -> Result<(), ClashApiError> {
        let url = format!("/proxies/{}", urlencode_path(group_name));
        let body = serde_json::json!({ "name": proxy_name }).to_string();
        self.put(&url, &body)?;
        Ok(())
    }

    pub fn close_all_connections(&self) -> Result<(), ClashApiError> {
        let _ = self.delete("/connections")?;
        Ok(())
    }

    pub fn decide_connection_ask(
        &self,
        id: &str,
        outbound: &str,
        action: &str,
        reject: bool,
    ) -> Result<(), ClashApiError> {
        let body = serde_json::json!({
            "id": id,
            "outbound": outbound,
            "action": action,
            "reject": reject,
        })
        .to_string();
        self.post("/connection-ask/decide", &body)?;
        Ok(())
    }

    /// Forget cached ask decisions for the given group keys so the next
    /// connection from those processes triggers a fresh ask dialog.
    pub fn forget_ask_session_keys(&self, keys: &[String]) -> Result<(), ClashApiError> {
        let body = serde_json::json!({ "keys": keys }).to_string();
        self.post("/connection-ask/forget", &body)?;
        Ok(())
    }

    /// Live temp-rule overlay (no SIGHUP). Body: JSON array of {id, expires_at, rule}.
    pub fn put_temp_rules(&self, body: &str) -> Result<(), ClashApiError> {
        self.put("/temp-rules/", body)?;
        Ok(())
    }

    /// Live permanent user rules (no SIGHUP).
    pub fn put_permanent_rules(&self, body: &str) -> Result<(), ClashApiError> {
        self.put("/permanent-rules/", body)?;
        Ok(())
    }

    /// Full live overlay {temp, permanent}.
    pub fn put_live_rules(&self, body: &str) -> Result<(), ClashApiError> {
        self.put("/live-rules/", body)?;
        Ok(())
    }

    /// Latency for one proxy, as measured by the core itself.
    ///
    /// A failed measurement is **not** an API error. The core answers 503 when
    /// the probe errored or came back 0 ms, and 504 when it ran out of time
    /// (`experimental/clashapi/proxies.go:218-228`), so both mean "this proxy
    /// did not answer" — reported as `delay: None`, the same shape the UI
    /// already renders as `timeout`. Only a genuinely broken API call is an
    /// error here; otherwise one dead proxy in a group surfaces as an app
    /// failure and hides the results of every proxy that did answer.
    pub fn proxy_delay_test(
        &self,
        proxy_name: &str,
        url: &str,
        timeout_ms: u64,
    ) -> Result<ClashDelayResult, ClashApiError> {
        let encoded = urlencode_path(proxy_name);
        let path = format!(
            "/proxies/{encoded}/delay?url={}&timeout={}",
            urlencode_query(url),
            timeout_ms
        );
        let response = self.get_allow_status(&path)?;
        let status = response.status();
        if status == 503 || status == 504 {
            // Not an error: the probe failed or ran out of time. Keep the core's
            // reason on the app console — it is the only place the real cause
            // (DNS failure, dead outbound) is visible — and report no delay.
            let detail = response.into_string().unwrap_or_default();
            eprintln!("[latency] {proxy_name}: no answer (status {status}) {detail}");
            return Ok(ClashDelayResult { delay: None });
        }
        let response = self.expect_success(&path, response)?;
        let body = response
            .into_string()
            .map_err(|e| ClashApiError::RequestFailed(e.to_string()))?;
        serde_json::from_str(&body)
            .map_err(|e| ClashApiError::NotExpected(format!("invalid delay response: {e}")))
    }

    pub fn get_rules(&self) -> Result<serde_json::Value, ClashApiError> {
        let response = self.get("/rules")?;
        let body = response
            .into_string()
            .map_err(|e| ClashApiError::RequestFailed(e.to_string()))?;
        serde_json::from_str(&body)
            .map_err(|e| ClashApiError::NotExpected(format!("invalid rules response: {e}")))
    }

    pub fn get_connections(&self) -> Result<serde_json::Value, ClashApiError> {
        let response = self.get("/connections")?;
        let body = response
            .into_string()
            .map_err(|e| ClashApiError::RequestFailed(e.to_string()))?;
        serde_json::from_str(&body)
            .map_err(|e| ClashApiError::NotExpected(format!("invalid connections response: {e}")))
    }

    pub fn dns_query(&self, name: &str) -> Result<ClashDnsQueryResponse, ClashApiError> {
        let path = format!("/dns/query?name={}&type=A", urlencode_query(name));
        let response = self.get(&path)?;
        let response = self.expect_success(&path, response)?;
        let body = response
            .into_string()
            .map_err(|e| ClashApiError::RequestFailed(e.to_string()))?;
        serde_json::from_str(&body)
            .map_err(|e| ClashApiError::NotExpected(format!("invalid DNS query response: {e}")))
    }

    pub fn flush_dns_cache(&self) -> Result<(), ClashApiError> {
        let dns = self
            .post("/cache/dns/flush", "")
            .and_then(|response| self.expect_success("/cache/dns/flush", response));
        let fakeip = self
            .post("/cache/fakeip/flush", "")
            .and_then(|response| self.expect_success("/cache/fakeip/flush", response));

        match (dns, fakeip) {
            (Ok(_), Ok(_)) => Ok(()),
            (Err(dns_error), Ok(_)) => Err(dns_error),
            (Ok(_), Err(fakeip_error)) => Err(fakeip_error),
            (Err(dns_error), Err(fakeip_error)) => Err(ClashApiError::RequestFailed(format!(
                "DNS cache flush failed: {dns_error:?}; FakeIP cache flush failed: {fakeip_error:?}"
            ))),
        }
    }

    fn expect_success(
        &self,
        path: &str,
        response: ureq::Response,
    ) -> Result<ureq::Response, ClashApiError> {
        let status = response.status();
        if (200..300).contains(&status) {
            return Ok(response);
        }

        let detail = response.into_string().unwrap_or_default();
        Err(ClashApiError::RequestFailed(format!(
            "http://{}{}: status code {status}: {detail}",
            self.controller, path
        )))
    }

    fn get(&self, path: &str) -> Result<ureq::Response, ClashApiError> {
        let url = format!("http://{}{}", self.controller, path);
        let mut req = ureq::get(&url).set("User-Agent", "Zeytun/0.1");
        if let Some(secret) = &self.secret {
            req = req.set("Authorization", &format!("Bearer {secret}"));
        }
        req.call()
            .map_err(|e| ClashApiError::RequestFailed(e.to_string()))
    }

    /// `get`, but a >= 400 status comes back as a `Response` instead of an error.
    ///
    /// ureq treats any >= 400 status as `Err(Error::Status)`, which is wrong for
    /// endpoints where a specific failing status carries meaning the caller must
    /// inspect. Transport failures are still errors.
    fn get_allow_status(&self, path: &str) -> Result<ureq::Response, ClashApiError> {
        let url = format!("http://{}{}", self.controller, path);
        let mut req = ureq::get(&url).set("User-Agent", "Zeytun/0.1");
        if let Some(secret) = &self.secret {
            req = req.set("Authorization", &format!("Bearer {secret}"));
        }
        match req.call() {
            Ok(response) => Ok(response),
            Err(ureq::Error::Status(_, response)) => Ok(response),
            Err(e) => Err(ClashApiError::RequestFailed(e.to_string())),
        }
    }

    fn put(&self, path: &str, body: &str) -> Result<ureq::Response, ClashApiError> {
        let url = format!("http://{}{}", self.controller, path);
        let mut req = ureq::put(&url)
            .set("Content-Type", "application/json")
            .set("User-Agent", "Zeytun/0.1");
        if let Some(secret) = &self.secret {
            req = req.set("Authorization", &format!("Bearer {secret}"));
        }
        match req.send_string(body) {
            Ok(resp) => {
                let status = resp.status();
                if (200..300).contains(&status) {
                    Ok(resp)
                } else {
                    let detail = resp.into_string().unwrap_or_default();
                    Err(ClashApiError::RequestFailed(format!(
                        "{url}: status code {status}: {detail}"
                    )))
                }
            }
            Err(ureq::Error::Status(code, resp)) => {
                let detail = resp.into_string().unwrap_or_default();
                Err(ClashApiError::RequestFailed(format!(
                    "{url}: status code {code}: {detail}"
                )))
            }
            Err(e) => Err(ClashApiError::RequestFailed(e.to_string())),
        }
    }

    fn post(&self, path: &str, body: &str) -> Result<ureq::Response, ClashApiError> {
        let url = format!("http://{}{}", self.controller, path);
        let mut req = ureq::post(&url)
            .set("Content-Type", "application/json")
            .set("User-Agent", "Zeytun/0.1");
        if let Some(secret) = &self.secret {
            req = req.set("Authorization", &format!("Bearer {secret}"));
        }
        req.send_string(body)
            .map_err(|e| ClashApiError::RequestFailed(e.to_string()))
    }

    fn delete(&self, path: &str) -> Result<ureq::Response, ClashApiError> {
        let url = format!("http://{}{}", self.controller, path);
        let mut req = ureq::delete(&url).set("User-Agent", "Zeytun/0.1");
        if let Some(secret) = &self.secret {
            req = req.set("Authorization", &format!("Bearer {secret}"));
        }
        req.call()
            .map_err(|e| ClashApiError::RequestFailed(e.to_string()))
    }

    // TODO: RENAME IT
    pub fn patch_config(&self, mode: &str) -> Result<(), ClashApiError> {
        let url = format!("http://{}/configs", self.controller);
        let mut req = ureq::request("PATCH", &url)
            .set("Content-Type", "application/json")
            .set("User-Agent", "Zeytun/0.1");
        if let Some(secret) = &self.secret {
            req = req.set("Authorization", &format!("Bearer {secret}"));
        }
        let body = serde_json::json!({ "mode": mode }).to_string();
        req.send_string(&body)
            .map_err(|e| ClashApiError::RequestFailed(e.to_string()))?;
        Ok(())
    }
}

fn urlencode_path(input: &str) -> String {
    input
        .split('/')
        .map(urlencoding)
        .collect::<Vec<_>>()
        .join("/")
}

fn urlencode_query(input: &str) -> String {
    urlencoding(input)
}

fn urlencoding(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push_str("%20"),
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uppercase_dns_query_response_fields() {
        let response: ClashDnsQueryResponse = serde_json::from_value(serde_json::json!({
            "Status": 0,
            "Answer": [
                { "Type": 1, "TTL": 120, "Data": "203.0.113.10" },
                { "type": 28, "ttl": 60, "data": "2001:db8::1" }
            ],
            "Server": "primary"
        }))
        .expect("DNS query response");

        assert_eq!(response.status, 0);
        assert_eq!(response.server, "primary");
        assert_eq!(response.answer[0].record_type, 1);
        assert_eq!(response.answer[0].ttl, 120);
        assert_eq!(response.answer[0].data, "203.0.113.10");
    }

    /// A failed measurement and a successful one must both parse, and neither is
    /// an error. The wire shape is a bare number on some builds, an object on
    /// others — `ClashDelayResult` accepts both.
    #[test]
    fn delay_result_accepts_both_wire_shapes() {
        let object: ClashDelayResult = serde_json::from_str(r#"{"delay":142}"#).unwrap();
        assert_eq!(object.delay, Some(142));

        let bare: ClashDelayResult = serde_json::from_str("142").unwrap();
        assert_eq!(bare.delay, Some(142));

        let missing: ClashDelayResult = serde_json::from_str(r#"{"delay":null}"#).unwrap();
        assert_eq!(missing.delay, None);
    }

    /// The user-facing text must not leak Rust's Debug formatting: `{e:?}` on the
    /// enum produced `RequestFailed("…")` in a toast.
    #[test]
    fn display_is_plain_text_not_debug() {
        let err = ClashApiError::RequestFailed("status code 503".into());
        assert_eq!(err.to_string(), "status code 503");
        assert!(!err.to_string().contains("RequestFailed"));
        assert_eq!(
            ClashApiError::NotRunning.to_string(),
            "the core is not running"
        );
    }
}
