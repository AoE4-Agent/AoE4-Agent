//! Client for the aoe4guides build-order API.
//!
//! Verified on 2026-11-14: `https://aoe4guides.com/api/builds` (the list)
//! is not proxied. The site tells clients to call the Cloud Run host
//! directly and to cache responses. Single-build GETs are also served
//! there, and `?overlay=true` returns the flat shape used by overlay tools.
//! The public API allows at most 10 builds per list call.

use crate::coach::{parse_build_document, parse_build_list, BuildOrder, BuildSummary};
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_API_BASE: &str = "https://aoe4-guides-api-7h2vti5ckq-ey.a.run.app";
pub const USER_AGENT: &str = "aoe4-agent/0.1.0 (+https://github.com/AoE4-Agent/AoE4-Agent)";

#[derive(Debug, Clone)]
pub struct ListQuery {
    pub civ: Option<String>,
    pub order_by: String,
    pub limit: usize,
}

pub struct GuidesClient {
    base: String,
    agent: ureq::Agent,
}

impl GuidesClient {
    pub fn new(base: impl Into<String>) -> Self {
        let base = base.into().trim_end_matches('/').to_string();
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(25))
            .user_agent(USER_AGENT)
            .build();
        Self { base, agent }
    }

    pub fn status(&self) -> Result<String> {
        let url = format!("{}/status", self.base);
        let body = self.get(&url)?;
        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|err| Error::Api(format!("status was not JSON: {err}")))?;
        Ok(value
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string())
    }

    pub fn list_builds(&self, query: &ListQuery) -> Result<Vec<BuildSummary>> {
        let mut url = format!(
            "{}/builds?orderBy={}",
            self.base,
            query_escape(&query.order_by)
        );
        if let Some(civ) = &query.civ {
            url.push_str("&civ=");
            url.push_str(&query_escape(&normalize_civ(civ)));
        }
        let body = self.get(&url)?;
        let mut builds = parse_build_list(&body)?;
        let limit = query.limit.clamp(1, 10);
        if builds.len() > limit {
            builds.truncate(limit);
        }
        Ok(builds)
    }

    /// Fetch the overlay form and fall back to the native document.
    pub fn get_build(&self, id: &str) -> Result<(String, BuildOrder)> {
        validate_build_id(id)?;
        let overlay_url = format!("{}/builds/{}?overlay=true", self.base, query_escape(id));
        match self.get(&overlay_url) {
            Ok(body) => match parse_build_document(&body, id) {
                Ok(build) => return Ok((body, build)),
                Err(overlay_err) => {
                    log::warn!("overlay parse failed ({overlay_err}); trying the native document");
                }
            },
            Err(err) => {
                log::warn!("overlay request failed ({err}); trying the native document");
            }
        }
        let native_url = format!("{}/builds/{}", self.base, query_escape(id));
        let body = self.get(&native_url)?;
        let build = parse_build_document(&body, id)?;
        Ok((body, build))
    }

    fn get(&self, url: &str) -> Result<String> {
        match self.agent.get(url).call() {
            Ok(response) => response
                .into_string()
                .map_err(|err| Error::Api(format!("reading {url}: {err}"))),
            Err(ureq::Error::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                if code == 404 {
                    return Err(Error::Api(format!(
                        "HTTP 404 from {url}: build was not found"
                    )));
                }
                let detail = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("message")
                            .and_then(|m| m.as_str())
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| clip(&body, 240));
                Err(Error::Api(format!("HTTP {code} from {url}: {detail}")))
            }
            Err(err) => Err(Error::Api(format!("request {url} failed: {err}"))),
        }
    }
}

pub fn fetch_build(client: &GuidesClient, cache_dir: &Path, id: &str) -> Result<BuildOrder> {
    validate_build_id(id)?;
    match client.get_build(id) {
        Ok((raw, build)) => {
            if let Err(err) = store_cache(cache_dir, id, &raw) {
                log::warn!("could not cache build {id}: {err}");
            }
            Ok(build)
        }
        Err(err) => match load_cache(cache_dir, id) {
            Ok(Some(build)) => {
                log::warn!("guides API failed ({err}); using the cached copy of {id}");
                Ok(build)
            }
            Ok(None) => Err(err),
            Err(cache_err) => Err(Error::Api(format!("{err} (cache unreadable: {cache_err})"))),
        },
    }
}

pub fn load_cache(dir: &Path, id: &str) -> Result<Option<BuildOrder>> {
    let path = cache_file(dir, id)?;
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|source| Error::io(&path, source))?;
    parse_build_document(&raw, id).map(Some)
}

pub fn store_cache(dir: &Path, id: &str, raw: &str) -> Result<()> {
    let path = cache_file(dir, id)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
    }
    std::fs::write(&path, raw).map_err(|source| Error::io(&path, source))?;
    Ok(())
}

fn cache_file(dir: &Path, id: &str) -> Result<PathBuf> {
    validate_build_id(id)?;
    Ok(dir.join(format!("{id}.json")))
}

pub fn validate_build_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(Error::Build(format!(
            "build id '{id}' must be 1-128 characters of letters, digits, '-' or '_'"
        )));
    }
    Ok(())
}

/// Accept a bare id or an aoe4guides URL such as
/// `https://aoe4guides.com/builds/abc123?overlay=true`.
pub fn normalize_build_id(raw: &str) -> Result<String> {
    let raw = raw.trim();
    let without_query = raw.split(['?', '#']).next().unwrap_or(raw);
    let id = if without_query.contains("://") {
        without_query
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(without_query)
    } else if without_query.contains(['/', '\\']) {
        return Err(Error::Build(format!(
            "build id '{raw}' must be a bare id or an http(s) URL, not a path"
        )));
    } else {
        without_query
    };
    validate_build_id(id)?;
    Ok(id.to_string())
}

pub fn normalize_civ(civ: &str) -> String {
    let trimmed = civ.trim();
    if trimmed.len() <= 4 && trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
        trimmed.to_ascii_uppercase()
    } else {
        trimmed.to_string()
    }
}

pub fn normalize_order_by(value: &str) -> Result<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "score" => Ok("score".into()),
        "views" => Ok("views".into()),
        "likes" => Ok("likes".into()),
        "timecreated" | "time_created" | "created" | "new" => Ok("timeCreated".into()),
        other => Err(Error::Config(format!(
            "order '{other}' is not one of score, views, likes, timeCreated"
        ))),
    }
}

fn query_escape(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn clip(text: &str, max: usize) -> String {
    let mut end = text.len().min(max);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_from_urls_and_rejects_paths() {
        assert_eq!(normalize_build_id("abc123").unwrap(), "abc123");
        assert_eq!(
            normalize_build_id("https://aoe4guides.com/builds/abc123?overlay=true").unwrap(),
            "abc123"
        );
        assert!(normalize_build_id("../secret").is_err());
        assert!(normalize_build_id("a/b").is_err());
        assert!(validate_build_id("..").is_err());
    }

    #[test]
    fn civ_and_sort_normalization() {
        assert_eq!(normalize_civ("eng"), "ENG");
        assert_eq!(normalize_civ("English"), "English");
        assert_eq!(normalize_order_by("time_created").unwrap(), "timeCreated");
        assert!(normalize_order_by("random").is_err());
    }

    #[test]
    fn cache_roundtrip_uses_the_id_as_a_filename() {
        let dir = std::env::temp_dir().join(format!(
            "aoe4-agent-cache-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let raw = r#"{"name":"Cached","civilization":"English","author":"T","source":"https://aoe4guides.com/builds/abc123","build_order":[{"age":1,"time":"0:10","villager_count":6,"resources":{"food":6,"wood":0,"gold":0,"stone":0,"builder":-1},"notes":["sheep"]}]}"#;
        store_cache(&dir, "abc123", raw).unwrap();
        let loaded = load_cache(&dir, "abc123").unwrap().unwrap();
        assert_eq!(loaded.title, "Cached");
        assert_eq!(loaded.steps[0].notes, "sheep");
        assert!(load_cache(&dir, "missing1").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
