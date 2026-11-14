//! Build-order timing, text cleanup, and the announcement schedule.
//!
//! aoe4guides documents come in two shapes (native groups, or the flat
//! overlay used by stream tools). Both normalize to [`BuildStep`].

use crate::{Error, Result};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStep {
    pub age: u8,
    pub at: Duration,
    /// Gameplan or "Aging up", set on the first step of a native group.
    pub intro: Option<String>,
    pub villagers: Option<u32>,
    pub food: Option<u32>,
    pub wood: Option<u32>,
    pub gold: Option<u32>,
    pub stone: Option<u32>,
    pub builders: Option<u32>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOrder {
    pub id: String,
    pub title: String,
    pub civ: String,
    pub author: String,
    pub source: String,
    pub description: String,
    pub steps: Vec<BuildStep>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildSummary {
    pub id: String,
    pub title: String,
    pub civ: String,
    pub author: String,
    pub score: f64,
    pub views: i64,
}

/// `m:ss` or `h:mm:ss`. Seconds (and minutes, when hours are present) must be `< 60`.
pub fn parse_clock(text: &str) -> Result<Duration> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::Build("empty clock time".into()));
    }
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 || parts.iter().any(|part| part.is_empty()) {
        return Err(Error::Build(format!(
            "time '{text}' is not m:ss or h:mm:ss"
        )));
    }
    let numbers: Result<Vec<u64>> = parts
        .iter()
        .map(|part| {
            if !part.chars().all(|c| c.is_ascii_digit()) {
                return Err(Error::Build(format!(
                    "time '{text}' has a non-numeric field"
                )));
            }
            part.parse::<u64>()
                .map_err(|_| Error::Build(format!("time '{text}' is out of range")))
        })
        .collect();
    let numbers = numbers?;
    let (hours, minutes, seconds) = match numbers.as_slice() {
        [m, s] => (0, *m, *s),
        [h, m, s] => (*h, *m, *s),
        _ => unreachable!(),
    };
    if seconds >= 60 || (numbers.len() == 3 && minutes >= 60) {
        return Err(Error::Build(format!(
            "time '{text}' has minutes or seconds at or above 60"
        )));
    }
    Ok(Duration::from_secs(hours * 3600 + minutes * 60 + seconds))
}

pub fn format_clock(duration: Duration) -> String {
    let secs = duration.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn announcement(step: &BuildStep) -> String {
    let mut out = format_clock(step.at);
    if step.age > 0 {
        out.push_str(&format!(", age {}", step.age));
    }
    if let Some(villagers) = step.villagers {
        let noun = if villagers == 1 {
            "villager"
        } else {
            "villagers"
        };
        out.push_str(&format!(", {villagers} {noun}"));
    }
    let resources = resource_phrase(step);
    if !resources.is_empty() {
        out.push_str(". ");
        out.push_str(&resources);
    }
    if let Some(intro) = &step.intro {
        if !intro.is_empty() {
            out.push_str(". ");
            out.push_str(intro);
        }
    }
    if !step.notes.is_empty() {
        out.push_str(". ");
        out.push_str(&step.notes);
    }
    out
}

fn resource_phrase(step: &BuildStep) -> String {
    let mut parts = Vec::new();
    push_count(&mut parts, step.food, "food", "food");
    push_count(&mut parts, step.wood, "wood", "wood");
    push_count(&mut parts, step.gold, "gold", "gold");
    push_count(&mut parts, step.stone, "stone", "stone");
    push_count(&mut parts, step.builders, "builder", "builders");
    parts.join(", ")
}

fn push_count(parts: &mut Vec<String>, count: Option<u32>, singular: &str, plural: &str) {
    match count {
        Some(1) => parts.push(format!("1 {singular}")),
        Some(count) if count > 1 => parts.push(format!("{count} {plural}")),
        _ => {}
    }
}

/// Strip guide markup down to something that can be spoken or typed.
///
/// Handles the two dialects the API actually returns: native HTML
/// (`<img title="House">`, `<br>`) and overlay tokens
/// (`@resource/sheep.webp@`).
pub fn clean_note(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' {
            if let Some(rel) = chars[i..].iter().position(|&c| c == '>') {
                let end = i + rel;
                let tag: String = chars[i + 1..end].iter().collect();
                let tag_l = tag.to_ascii_lowercase();
                if tag_l.starts_with("br") || tag_l.starts_with("/br") {
                    out.push_str(". ");
                } else if tag_l.trim_start_matches('/').starts_with("img") {
                    if let Some(title) = html_attr(&tag, "title") {
                        push_word(&mut out, &title);
                    }
                }
                i = end + 1;
                continue;
            }
        }
        if chars[i] == '@' {
            if let Some(rel) = chars[i + 1..].iter().position(|&c| c == '@') {
                let token: String = chars[i + 1..i + 1 + rel].iter().collect();
                push_word(&mut out, &token_word(&token));
                i = i + 1 + rel + 1;
                continue;
            }
        }
        if chars[i] == '&' {
            if let Some(rel) = chars[i + 1..].iter().position(|&c| c == ';') {
                let ent: String = chars[i + 1..i + 1 + rel].iter().collect();
                let repl = match ent.as_str() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "apos" | "#39" => "'",
                    "nbsp" => " ",
                    _ => "",
                };
                out.push_str(repl);
                i = i + 1 + rel + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    tidy(&out)
}

fn push_word(out: &mut String, word: &str) {
    if word.is_empty() {
        return;
    }
    if !out.is_empty() && !out.ends_with(' ') {
        out.push(' ');
    }
    out.push_str(word);
}

fn token_word(token: &str) -> String {
    let file = token.rsplit(['/', '\\']).next().unwrap_or(token);
    let stem = file.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file);
    let stem = stem.strip_prefix("resource_").unwrap_or(stem);
    stem.replace(['-', '_'], " ")
}

fn html_attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let key = format!("{name}=");
    let idx = lower.find(&key)?;
    let rest = tag.get(idx + key.len()..)?;
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = rest.get(quote.len_utf8()..)?;
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn tidy(input: &str) -> String {
    let mut collapsed = String::new();
    let mut prev_space = false;
    for c in input.chars() {
        if c.is_control() {
            continue;
        }
        if c.is_whitespace() {
            if prev_space || collapsed.is_empty() {
                continue;
            }
            prev_space = true;
            collapsed.push(' ');
            continue;
        }
        prev_space = false;
        collapsed.push(c);
    }
    let collapsed = collapsed.trim().replace(" .", ".");
    collapsed
        .trim()
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .trim()
        .to_string()
}

pub fn clip_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// One line, no control characters, safe to type into chat.
pub fn sanitize_chat(text: &str, max_chars: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    clip_chars(&tidy(&flat), max_chars)
}

#[derive(Debug, Clone)]
pub struct Scheduler {
    steps: Vec<BuildStep>,
    announced: Vec<bool>,
    lead: Duration,
}

impl Scheduler {
    pub fn new(mut steps: Vec<BuildStep>, lead: Duration) -> Self {
        steps.sort_by_key(|step| step.at);
        let announced = vec![false; steps.len()];
        Self {
            steps,
            announced,
            lead,
        }
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn remaining(&self) -> usize {
        self.announced.iter().filter(|done| !**done).count()
    }

    /// Mark steps whose speak-time is already more than `grace` behind
    /// `elapsed`, so joining a game late does not dump the whole build.
    pub fn skip_past(&mut self, elapsed: Duration, use_lead: bool, grace: Duration) {
        for (i, step) in self.steps.iter().enumerate() {
            let speak_at = if use_lead {
                step.at.saturating_sub(self.lead)
            } else {
                step.at
            };
            if speak_at.saturating_add(grace) < elapsed {
                self.announced[i] = true;
            }
        }
    }

    pub fn poll(&mut self, elapsed: Duration) -> Vec<BuildStep> {
        let mut due = Vec::new();
        for (i, step) in self.steps.iter().enumerate() {
            if !self.announced[i] && elapsed.saturating_add(self.lead) >= step.at {
                self.announced[i] = true;
                due.push(step.clone());
            }
        }
        due
    }

    pub fn advance(&mut self) -> Option<BuildStep> {
        let index = self.announced.iter().position(|done| !done)?;
        self.announced[index] = true;
        Some(self.steps[index].clone())
    }
}

pub fn parse_build_document(body: &str, fallback_id: &str) -> Result<BuildOrder> {
    let value: Value = serde_json::from_str(body)
        .map_err(|err| Error::Build(format!("build JSON is not valid: {err}")))?;
    if !value.is_object() {
        return Err(Error::Build("build JSON is not an object".into()));
    }
    if value.get("build_order").is_some() {
        return parse_overlay(&value, fallback_id);
    }
    if value.get("steps").is_some() {
        return parse_native(&value, fallback_id);
    }
    Err(document_error(&value, fallback_id))
}

pub fn parse_build_list(body: &str) -> Result<Vec<BuildSummary>> {
    let value: Value = serde_json::from_str(body)
        .map_err(|err| Error::Api(format!("build list is not valid JSON: {err}")))?;
    let Some(items) = value.as_array() else {
        return Err(Error::Api(api_message(&value)));
    };
    let mut out = Vec::new();
    for item in items {
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            continue;
        };
        if crate::guides::validate_build_id(id).is_err() {
            continue;
        }
        out.push(BuildSummary {
            id: id.to_string(),
            title: string_field(item, "title").unwrap_or_else(|| "(untitled)".into()),
            civ: string_field(item, "civ").unwrap_or_default(),
            author: string_field(item, "author").unwrap_or_default(),
            score: item.get("score").and_then(Value::as_f64).unwrap_or(0.0),
            views: item.get("views").and_then(Value::as_i64).unwrap_or(0),
        });
    }
    Ok(out)
}

fn parse_overlay(value: &Value, fallback_id: &str) -> Result<BuildOrder> {
    let id = string_field(value, "id")
        .filter(|id| crate::guides::validate_build_id(id).is_ok())
        .or_else(|| string_field(value, "source").and_then(|source| id_from_source(&source)))
        .unwrap_or_else(|| fallback_id.to_string());
    crate::guides::validate_build_id(&id)?;
    let steps_value = value
        .get("build_order")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Build("overlay build has no build_order array".into()))?;
    let mut steps = Vec::new();
    for (index, step) in steps_value.iter().enumerate() {
        let Some(time) = step.get("time").and_then(Value::as_str) else {
            continue;
        };
        if time.trim().is_empty() {
            continue;
        }
        let at = parse_clock(time)
            .map_err(|err| Error::Build(format!("overlay step {}: {err}", index + 1)))?;
        let notes = step
            .get("notes")
            .and_then(Value::as_array)
            .map(|notes| {
                notes
                    .iter()
                    .filter_map(Value::as_str)
                    .map(clean_note)
                    .filter(|note| !note.is_empty())
                    .collect::<Vec<_>>()
                    .join(". ")
            })
            .unwrap_or_default();
        let resources = step.get("resources");
        steps.push(BuildStep {
            age: step.get("age").and_then(Value::as_u64).unwrap_or(0) as u8,
            at,
            intro: None,
            villagers: positive_u32(loose_i64(step.get("villager_count"))),
            food: positive_u32(loose_i64(resources.and_then(|r| r.get("food")))),
            wood: positive_u32(loose_i64(resources.and_then(|r| r.get("wood")))),
            gold: positive_u32(loose_i64(resources.and_then(|r| r.get("gold")))),
            stone: positive_u32(loose_i64(resources.and_then(|r| r.get("stone")))),
            builders: positive_u32(loose_i64(resources.and_then(|r| r.get("builder")))),
            notes,
        });
    }
    if steps.is_empty() {
        return Err(Error::Build(format!("build {id} has no timed steps")));
    }
    Ok(BuildOrder {
        id,
        title: string_field(value, "name").unwrap_or_else(|| "(untitled)".into()),
        civ: string_field(value, "civilization").unwrap_or_default(),
        author: string_field(value, "author").unwrap_or_default(),
        source: string_field(value, "source").unwrap_or_default(),
        description: string_field(value, "description").unwrap_or_default(),
        steps,
    })
}

fn parse_native(value: &Value, fallback_id: &str) -> Result<BuildOrder> {
    let id = string_field(value, "id").unwrap_or_else(|| fallback_id.to_string());
    crate::guides::validate_build_id(&id)?;
    let groups = value
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Build("native build has no steps array".into()))?;
    let mut steps = Vec::new();
    for group in groups {
        let kind = group.get("type").and_then(Value::as_str).unwrap_or("");
        let age = group.get("age").and_then(Value::as_u64).unwrap_or(0) as u8;
        let gameplan = group
            .get("gameplan")
            .and_then(Value::as_str)
            .map(clean_note)
            .filter(|text| !text.is_empty());
        let inner = group.get("steps").and_then(Value::as_array);
        let Some(inner) = inner else {
            continue;
        };
        let mut first = true;
        for (index, step) in inner.iter().enumerate() {
            let Some(time) = step.get("time").and_then(Value::as_str) else {
                continue;
            };
            if time.trim().is_empty() {
                continue;
            }
            let at =
                parse_clock(time).map_err(|err| Error::Build(format!("native step: {err}")))?;
            let intro = if first {
                first = false;
                if let Some(plan) = gameplan.clone() {
                    Some(plan)
                } else if kind.eq_ignore_ascii_case("ageUp") {
                    Some("Aging up".into())
                } else {
                    None
                }
            } else {
                None
            };
            let _ = index;
            steps.push(BuildStep {
                age,
                at,
                intro,
                villagers: positive_u32(loose_i64(step.get("villagers"))),
                food: positive_u32(loose_i64(step.get("food"))),
                wood: positive_u32(loose_i64(step.get("wood"))),
                gold: positive_u32(loose_i64(step.get("gold"))),
                stone: positive_u32(loose_i64(step.get("stone"))),
                builders: positive_u32(loose_i64(
                    step.get("builders").or_else(|| step.get("builder")),
                )),
                notes: step
                    .get("description")
                    .and_then(Value::as_str)
                    .map(clean_note)
                    .unwrap_or_default(),
            });
        }
    }
    if steps.is_empty() {
        return Err(Error::Build(format!("build {id} has no timed steps")));
    }
    Ok(BuildOrder {
        id,
        title: string_field(value, "title").unwrap_or_else(|| "(untitled)".into()),
        civ: string_field(value, "civ").unwrap_or_default(),
        author: string_field(value, "author").unwrap_or_default(),
        source: string_field(value, "source").unwrap_or_default(),
        description: string_field(value, "description").unwrap_or_default(),
        steps,
    })
}

fn id_from_source(source: &str) -> Option<String> {
    crate::guides::normalize_build_id(source).ok()
}

fn document_error(value: &Value, fallback_id: &str) -> Error {
    if let Some(message) = value.get("message").and_then(Value::as_str) {
        if !message.is_empty() {
            return Error::Api(message.to_string());
        }
    }
    if value.get("reason").is_some() || value.get("error").is_some() {
        return Error::Build(format!("build {fallback_id} was not found"));
    }
    Error::Build(format!(
        "build {fallback_id} is neither an overlay nor a native build order"
    ))
}

fn api_message(value: &Value) -> String {
    value
        .get("message")
        .and_then(Value::as_str)
        .filter(|message| !message.is_empty())
        .unwrap_or("build list was not a JSON array")
        .to_string()
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn loose_i64(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|n| n as i64)),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<i64>().ok()
            }
        }
        _ => None,
    }
}

fn positive_u32(value: Option<i64>) -> Option<u32> {
    match value {
        Some(n) if n >= 0 => u32::try_from(n).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_parse_and_reject() {
        assert_eq!(parse_clock("0:00").unwrap(), Duration::ZERO);
        assert_eq!(parse_clock("0:35").unwrap(), Duration::from_secs(35));
        assert_eq!(parse_clock("2:10").unwrap(), Duration::from_secs(130));
        assert_eq!(
            parse_clock(" 12:05 ").unwrap(),
            Duration::from_secs(12 * 60 + 5)
        );
        assert_eq!(parse_clock("1:02:03").unwrap(), Duration::from_secs(3723));
        assert_eq!(format_clock(Duration::from_secs(3723)), "1:02:03");
        assert_eq!(format_clock(Duration::from_secs(35)), "0:35");
        assert!(parse_clock("2:60").is_err());
        assert!(parse_clock("1:60:00").is_err());
        assert!(parse_clock("2").is_err());
        assert!(parse_clock("").is_err());
        assert!(parse_clock("2:10.5").is_err());
    }

    #[test]
    fn markup_becomes_speech() {
        let html = "Build a <img src=\"/house.webp\" title=\"House\" /> then <br> gold &amp; wood";
        assert_eq!(clean_note(html), "Build a House then. gold & wood");
        let tokens = "@resource/rally.webp@ 6 on @resource/sheep.webp@ and @building_economy/mining-camp.webp@";
        assert_eq!(clean_note(tokens), "rally 6 on sheep and mining camp");
        assert_eq!(clean_note("@resource/resource_food.webp@"), "food");
    }

    #[test]
    fn overlay_and_native_documents_normalize() {
        let overlay = r#"{
            "name": "Test BO",
            "civilization": "English",
            "author": "Tester",
            "source": "https://aoe4guides.com/builds/abc123",
            "build_order": [
                {
                    "age": 1,
                    "time": "0:00",
                    "villager_count": 6,
                    "resources": {"food": 6, "wood": 0, "gold": 0, "stone": 0, "builder": -1},
                    "notes": ["@resource/rally.webp@ 6 on @resource/sheep.webp@"]
                },
                {
                    "age": 2,
                    "time": "4:45",
                    "villager_count": 19,
                    "resources": {"food": 8, "wood": 11, "gold": 0, "stone": 0, "builder": 0},
                    "notes": ["White Tower"]
                }
            ]
        }"#;
        let build = parse_build_document(overlay, "fallback").unwrap();
        assert_eq!(build.id, "abc123");
        assert_eq!(build.steps.len(), 2);
        assert_eq!(build.steps[0].at, Duration::from_secs(0));
        assert_eq!(build.steps[0].food, Some(6));
        assert_eq!(build.steps[0].builders, None);
        assert!(build.steps[0].notes.contains("sheep"));
        assert_eq!(build.steps[1].at, Duration::from_secs(285));
        let spoken = announcement(&build.steps[1]);
        assert!(spoken.contains("4:45"), "{spoken}");
        assert!(spoken.contains("19 villagers"), "{spoken}");
        assert!(spoken.contains("11 wood"), "{spoken}");
        assert!(!spoken.contains("0 gold"), "{spoken}");

        let native = r#"{
            "id": "abc123",
            "title": "Test BO",
            "civ": "ENG",
            "author": "Tester",
            "steps": [
                {
                    "type": "age",
                    "age": 1,
                    "gameplan": "Open",
                    "steps": [{
                        "time": "0:35",
                        "food": "5",
                        "wood": "",
                        "gold": "1",
                        "stone": "",
                        "builders": "1",
                        "description": "Build a <img title=\"House\" /> then <br> gold"
                    }]
                },
                {"type": "ageUp", "age": 1, "gameplan": "", "steps": []}
            ]
        }"#;
        let native = parse_build_document(native, "fallback").unwrap();
        assert_eq!(native.steps.len(), 1);
        assert_eq!(native.steps[0].at, Duration::from_secs(35));
        assert_eq!(native.steps[0].intro.as_deref(), Some("Open"));
        assert_eq!(native.steps[0].gold, Some(1));
        assert_eq!(native.steps[0].builders, Some(1));
        assert!(native.steps[0].notes.contains("House"));
        let spoken = announcement(&native.steps[0]);
        assert!(spoken.starts_with("0:35"), "{spoken}");
        assert!(spoken.contains("Open"), "{spoken}");
        assert!(spoken.contains("1 builder"), "{spoken}");
        assert!(!spoken.contains("1 builders"), "{spoken}");
    }

    #[test]
    fn scheduler_lead_skip_and_hotkey_advance() {
        let steps = vec![step_at(0), step_at(35), step_at(130)];
        let mut sched = Scheduler::new(steps, Duration::from_secs(2));
        assert!(
            sched.poll(Duration::from_secs(0)).len() == 1,
            "0:00 is due immediately with lead"
        );
        assert!(sched.poll(Duration::from_secs(32)).is_empty());
        let due = sched.poll(Duration::from_secs(33));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].at, Duration::from_secs(35));

        let mut late = Scheduler::new(
            vec![step_at(0), step_at(35), step_at(130)],
            Duration::from_secs(2),
        );
        late.skip_past(Duration::from_secs(40), true, Duration::from_secs(1));
        assert_eq!(late.remaining(), 1);
        let rest = late.poll(Duration::from_secs(40));
        assert!(
            rest.is_empty(),
            "2:10 speaks at 2:08, which is not yet due at 0:40"
        );
        assert_eq!(late.poll(Duration::from_secs(128)).len(), 1);

        let mut hotkey = Scheduler::new(vec![step_at(0), step_at(35)], Duration::from_secs(2));
        hotkey.skip_past(Duration::from_secs(10), false, Duration::from_secs(1));
        let next = hotkey.advance().unwrap();
        assert_eq!(next.at, Duration::from_secs(35));
        assert!(hotkey.advance().is_none());
    }

    #[test]
    fn list_and_error_documents() {
        let body = r#"[{"id":"abc123","title":"A","civ":"ENG","author":"T","score":1.5,"views":9,"steps":[]}]"#;
        let list = parse_build_list(body).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "abc123");
        assert_eq!(list[0].views, 9);
        let err =
            parse_build_list(r#"{"error":"not_proxied","message":"use cloud run"}"#).unwrap_err();
        assert!(err.to_string().contains("cloud run"), "{err}");
        let missing = parse_build_document(r#"{"reason":""}"#, "missing1").unwrap_err();
        assert!(missing.to_string().contains("not found"), "{missing}");
    }

    fn step_at(secs: u64) -> BuildStep {
        BuildStep {
            age: 1,
            at: Duration::from_secs(secs),
            intro: None,
            villagers: None,
            food: None,
            wood: None,
            gold: None,
            stone: None,
            builders: None,
            notes: String::new(),
        }
    }
}
