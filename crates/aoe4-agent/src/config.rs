//! YAML configuration. The shipped file is `config.example.yaml`.
//! Unknown keys are rejected so a typo fails at startup instead of being ignored.

use crate::frame::{fraction_in_unit, FractionRect, TownCenter};
use crate::taunt::TauntRule;
use crate::vision::{DetectParams, HsvRange};
use crate::{Error, Result};
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub process: ProcessConfig,
    pub capture: CaptureConfig,
    pub colors: ColorConfig,
    pub detection: DetectionConfig,
    #[serde(default)]
    pub taunts: Vec<TauntConfig>,
    pub input: InputConfig,
    pub coach: CoachConfig,
    #[serde(default)]
    pub log: LogConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessConfig {
    pub name: String,
    #[serde(default = "default_true")]
    pub require_foreground: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureConfig {
    #[serde(default = "default_poll_ms")]
    pub poll_ms: u64,
    pub minimap: FractionRect,
    pub town_center: TownCenter,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorConfig {
    #[serde(default)]
    pub enemies: Vec<NamedRange>,
    #[serde(default)]
    pub own: Option<NamedRange>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedRange {
    pub name: String,
    pub h_min: f32,
    pub h_max: f32,
    pub s_min: f32,
    pub s_max: f32,
    pub v_min: f32,
    pub v_max: f32,
}

impl NamedRange {
    pub fn range(&self) -> HsvRange {
        HsvRange {
            h_min: self.h_min,
            h_max: self.h_max,
            s_min: self.s_min,
            s_max: self.s_max,
            v_min: self.v_min,
            v_max: self.v_max,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionConfig {
    /// 4 or 8.
    #[serde(default = "default_connectivity")]
    pub connectivity: u8,
    #[serde(default = "default_min_cluster")]
    pub min_cluster_pixels: usize,
    #[serde(default = "default_true")]
    pub near_town_center_only: bool,
    #[serde(default = "default_consecutive")]
    pub consecutive_frames: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TauntConfig {
    pub id: String,
    pub command: String,
    #[serde(default)]
    pub label: String,
    pub cooldown_secs: u64,
    #[serde(default = "default_min_clusters")]
    pub min_clusters: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChatTarget {
    #[default]
    All,
    Team,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputConfig {
    /// When true, taunts and coach chat lines are printed and not typed.
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default)]
    pub chat: ChatTarget,
    #[serde(default = "default_key_delay")]
    pub key_delay_ms: u64,
    #[serde(default = "default_open_delay")]
    pub chat_open_delay_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AnnounceChannel {
    #[default]
    Tts,
    Chat,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CoachMode {
    #[default]
    Timer,
    Hotkey,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoachConfig {
    #[serde(default = "default_api_base")]
    pub api_base: String,
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,
    #[serde(default)]
    pub channel: AnnounceChannel,
    #[serde(default)]
    pub mode: CoachMode,
    #[serde(default = "default_lead")]
    pub lead_secs: u64,
    /// Windows virtual key such as `F13`, or `none` to use stdin only.
    #[serde(default = "default_hotkey")]
    pub step_hotkey: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_poll_ms() -> u64 {
    200
}
fn default_connectivity() -> u8 {
    8
}
fn default_min_cluster() -> usize {
    8
}
fn default_consecutive() -> u32 {
    3
}
fn default_min_clusters() -> usize {
    1
}
fn default_key_delay() -> u64 {
    40
}
fn default_open_delay() -> u64 {
    80
}
fn default_api_base() -> String {
    crate::guides::DEFAULT_API_BASE.to_string()
}
fn default_cache_dir() -> String {
    "cache".into()
}
fn default_lead() -> u64 {
    2
}
fn default_hotkey() -> String {
    "F13".into()
}
fn default_log_level() -> String {
    "info".into()
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Err(Error::Config(format!(
                "config file {} does not exist. Copy config.example.yaml to config.yaml.",
                path.display()
            )));
        }
        let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        Self::parse(&text).map_err(|err| match err {
            Error::Config(message) => Error::Config(format!("{}: {message}", path.display())),
            other => other,
        })
    }

    pub fn parse(text: &str) -> Result<Self> {
        let cfg: Config = serde_yaml::from_str(text)
            .map_err(|err| Error::Config(format!("YAML is invalid: {err}")))?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<()> {
        if self.process.name.trim().is_empty() {
            return Err(Error::Config("process.name must not be empty".into()));
        }
        if !(16..=5_000).contains(&self.capture.poll_ms) {
            return Err(Error::Config(
                "capture.poll_ms must be from 16 to 5000".into(),
            ));
        }
        fraction_in_unit("capture.minimap.x", self.capture.minimap.x)?;
        fraction_in_unit("capture.minimap.y", self.capture.minimap.y)?;
        fraction_in_unit("capture.minimap.width", self.capture.minimap.width)?;
        fraction_in_unit("capture.minimap.height", self.capture.minimap.height)?;
        if self.capture.minimap.width <= 0.0 || self.capture.minimap.height <= 0.0 {
            return Err(Error::Config("minimap width and height must be > 0".into()));
        }
        if self.capture.minimap.x + self.capture.minimap.width > 1.01
            || self.capture.minimap.y + self.capture.minimap.height > 1.01
        {
            return Err(Error::Config(
                "minimap rectangle extends past the monitor; keep x+width and y+height within 1"
                    .into(),
            ));
        }
        fraction_in_unit("capture.town_center.cx", self.capture.town_center.cx)?;
        fraction_in_unit("capture.town_center.cy", self.capture.town_center.cy)?;
        if !(0.01..=1.5).contains(&self.capture.town_center.radius) {
            return Err(Error::Config(
                "town_center.radius must be from 0.01 to 1.5 (fraction of the shorter crop side)"
                    .into(),
            ));
        }
        if self.colors.enemies.is_empty() {
            return Err(Error::Config(
                "colors.enemies needs at least one range".into(),
            ));
        }
        for range in &self.colors.enemies {
            range.range().validate(&range.name)?;
        }
        if let Some(own) = &self.colors.own {
            own.range().validate(&own.name)?;
        }
        if self.detection.connectivity != 4 && self.detection.connectivity != 8 {
            return Err(Error::Config(
                "detection.connectivity must be 4 or 8".into(),
            ));
        }
        if self.detection.min_cluster_pixels == 0 {
            return Err(Error::Config(
                "detection.min_cluster_pixels must be >= 1".into(),
            ));
        }
        if !(1..=60).contains(&self.detection.consecutive_frames) {
            return Err(Error::Config(
                "detection.consecutive_frames must be from 1 to 60".into(),
            ));
        }
        if self.taunts.is_empty() {
            return Err(Error::Config(
                "taunts must list at least one command".into(),
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for taunt in &self.taunts {
            if taunt.id.trim().is_empty() || !seen.insert(taunt.id.clone()) {
                return Err(Error::Config(format!(
                    "taunt id '{}' is empty or duplicated",
                    taunt.id
                )));
            }
            if taunt.command.trim().is_empty()
                || taunt.command.chars().any(|c| c.is_control())
                || taunt.command.chars().count() > 80
            {
                return Err(Error::Config(format!(
                    "taunt '{}' command must be 1-80 characters without control codes",
                    taunt.id
                )));
            }
            if !(1..=3_600).contains(&taunt.cooldown_secs) {
                return Err(Error::Config(format!(
                    "taunt '{}' cooldown_secs must be from 1 to 3600",
                    taunt.id
                )));
            }
            if taunt.min_clusters == 0 {
                return Err(Error::Config(format!(
                    "taunt '{}' min_clusters must be >= 1",
                    taunt.id
                )));
            }
        }
        if self.input.key_delay_ms > 500 || self.input.chat_open_delay_ms > 1_000 {
            return Err(Error::Config(
                "key_delay_ms must be <= 500 and chat_open_delay_ms <= 1000".into(),
            ));
        }
        if self.coach.api_base.trim().is_empty() {
            return Err(Error::Config("coach.api_base must not be empty".into()));
        }
        if self.coach.lead_secs > 120 {
            return Err(Error::Config(
                "coach.lead_secs must be from 0 to 120".into(),
            ));
        }
        parse_hotkey(&self.coach.step_hotkey).map(|_| ())?;
        match self.log.level.to_ascii_lowercase().as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => {}
            other => {
                return Err(Error::Config(format!(
                    "log.level '{other}' is not error, warn, info, debug, or trace"
                )))
            }
        }
        Ok(())
    }

    pub fn rules(&self) -> Vec<TauntRule> {
        self.taunts
            .iter()
            .map(|taunt| TauntRule {
                id: taunt.id.clone(),
                command: taunt.command.clone(),
                label: if taunt.label.is_empty() {
                    taunt.command.clone()
                } else {
                    taunt.label.clone()
                },
                cooldown: Duration::from_secs(taunt.cooldown_secs),
                min_clusters: taunt.min_clusters,
            })
            .collect()
    }

    pub fn detector(&self) -> DetectParams {
        DetectParams {
            enemies: self.colors.enemies.iter().map(NamedRange::range).collect(),
            own: self.colors.own.as_ref().map(NamedRange::range),
            eight_connected: self.detection.connectivity == 8,
            min_cluster_pixels: self.detection.min_cluster_pixels,
            town_center: self
                .detection
                .near_town_center_only
                .then_some(self.capture.town_center),
        }
    }

    pub fn key_delay(&self) -> Duration {
        Duration::from_millis(self.input.key_delay_ms)
    }

    pub fn chat_open_delay(&self) -> Duration {
        Duration::from_millis(self.input.chat_open_delay_ms)
    }

    pub fn poll(&self) -> Duration {
        Duration::from_millis(self.capture.poll_ms)
    }
}

/// Virtual-key code for a small set of keys that are safe to poll globally.
/// `none` disables the Windows hotkey (stdin still advances hotkey mode).
pub fn parse_hotkey(name: &str) -> Result<Option<u16>> {
    let name = name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let upper = name.to_ascii_uppercase();
    if let Some(rest) = upper.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u16>() {
            if (1..=24).contains(&n) {
                return Ok(Some(0x70 + n - 1));
            }
        }
    }
    Err(Error::Config(format!(
        "step_hotkey '{name}' is not F1-F24 or none"
    )))
}

pub fn init_logging(level: &str) {
    let mut builder = env_logger::Builder::new();
    if std::env::var_os("RUST_LOG").is_some() {
        builder.parse_default_env();
    } else {
        builder.parse_filters(level);
    }
    let _ = builder.try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../../../config.example.yaml");

    #[test]
    fn example_config_parses_and_matches_the_committed_default() {
        let cfg = Config::parse(EXAMPLE).unwrap();
        assert_eq!(cfg.process.name, "RelicCardinal.exe");
        assert!(cfg.input.dry_run);
        assert_eq!(cfg.taunts[0].command, "/63");
        assert_eq!(cfg.taunts[0].label, "I'm under attack!");
        assert_eq!(cfg.coach.api_base, crate::guides::DEFAULT_API_BASE);
        assert_eq!(cfg.coach.channel, AnnounceChannel::Tts);
        assert!(cfg.detector().town_center.is_some());
        let committed = include_str!("../../../config.yaml");
        assert_eq!(EXAMPLE, committed);
    }

    #[test]
    fn example_red_range_matches_pure_red_and_not_blue() {
        let cfg = Config::parse(EXAMPLE).unwrap();
        let red = cfg.colors.enemies[0].range();
        assert!(red.contains(crate::vision::rgb_to_hsv(255, 0, 0)));
        let own = cfg.colors.own.unwrap().range();
        assert!(own.contains(crate::vision::rgb_to_hsv(0, 0, 255)));
        assert!(!red.contains(crate::vision::rgb_to_hsv(0, 0, 255)));
    }

    #[test]
    fn hotkeys_cover_the_function_row() {
        assert_eq!(parse_hotkey("F13").unwrap(), Some(0x7C));
        assert_eq!(parse_hotkey("f1").unwrap(), Some(0x70));
        assert_eq!(parse_hotkey("none").unwrap(), None);
        assert!(parse_hotkey("Enter").is_err());
    }
}
