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
    #[serde(default)]
    pub process: ProcessConfig,
    pub capture: CaptureConfig,
    pub colors: ColorConfig,
    #[serde(default)]
    pub detection: DetectionConfig,
    #[serde(default)]
    pub taunts: Vec<TauntConfig>,
    #[serde(default)]
    pub input: InputConfig,
    #[serde(default)]
    pub coach: CoachConfig,
    #[serde(default)]
    pub log: LogConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessConfig {
    /// Game executable, matched case-insensitively.
    #[serde(default = "default_process_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub require_foreground: bool,
    /// Repeat the "waiting for the game" line at this interval.
    #[serde(default = "default_status_log_secs")]
    pub status_log_secs: u64,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            name: default_process_name(),
            require_foreground: default_true(),
            status_log_secs: default_status_log_secs(),
        }
    }
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

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            connectivity: default_connectivity(),
            min_cluster_pixels: default_min_cluster(),
            near_town_center_only: default_true(),
            consecutive_frames: default_consecutive(),
        }
    }
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
    /// Key that opens chat and sends the line. `enter` is the AoE4 default.
    #[serde(default = "default_chat_key")]
    pub chat_key: String,
    /// Held while tapping `chat_key` when `chat` is `team`.
    #[serde(default = "default_team_modifier")]
    pub team_modifier: String,
    #[serde(default = "default_key_delay")]
    pub key_delay_ms: u64,
    #[serde(default = "default_open_delay")]
    pub chat_open_delay_ms: u64,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            dry_run: default_true(),
            chat: ChatTarget::All,
            chat_key: default_chat_key(),
            team_modifier: default_team_modifier(),
            key_delay_ms: default_key_delay(),
            chat_open_delay_ms: default_open_delay(),
        }
    }
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
    /// Relative to the executable's directory unless this is a rooted path.
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
    #[serde(default = "default_read_timeout")]
    pub read_timeout_secs: u64,
    /// Client-side cap on `coach list`. The guides API itself returns at most 10.
    #[serde(default = "default_list_cap")]
    pub list_cap: usize,
    #[serde(default)]
    pub channel: AnnounceChannel,
    #[serde(default)]
    pub mode: CoachMode,
    #[serde(default = "default_lead")]
    pub lead_secs: u64,
    /// Steps already this far behind `--start-at` are skipped.
    #[serde(default = "default_skip_grace")]
    pub skip_grace_secs: u64,
    /// How often the coach checks the clock or the hotkey.
    #[serde(default = "default_tick_ms")]
    pub tick_ms: u64,
    #[serde(default = "default_speech_chars")]
    pub speech_chars: usize,
    #[serde(default = "default_chat_chars")]
    pub chat_chars: usize,
    /// Windows virtual key such as `F13`, or `none` to use stdin only.
    #[serde(default = "default_hotkey")]
    pub step_hotkey: String,
}

impl Default for CoachConfig {
    fn default() -> Self {
        Self {
            api_base: default_api_base(),
            cache_dir: default_cache_dir(),
            connect_timeout_secs: default_connect_timeout(),
            read_timeout_secs: default_read_timeout(),
            list_cap: default_list_cap(),
            channel: AnnounceChannel::Tts,
            mode: CoachMode::Timer,
            lead_secs: default_lead(),
            skip_grace_secs: default_skip_grace(),
            tick_ms: default_tick_ms(),
            speech_chars: default_speech_chars(),
            chat_chars: default_chat_chars(),
            step_hotkey: default_hotkey(),
        }
    }
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

fn default_process_name() -> String {
    "RelicCardinal.exe".into()
}
fn default_status_log_secs() -> u64 {
    5
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
fn default_chat_key() -> String {
    "enter".into()
}
fn default_team_modifier() -> String {
    "shift".into()
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
fn default_connect_timeout() -> u64 {
    10
}
fn default_read_timeout() -> u64 {
    25
}
fn default_list_cap() -> usize {
    10
}
fn default_lead() -> u64 {
    2
}
fn default_skip_grace() -> u64 {
    1
}
fn default_tick_ms() -> u64 {
    100
}
fn default_speech_chars() -> usize {
    400
}
fn default_chat_chars() -> usize {
    120
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
        if !(1..=3_600).contains(&self.process.status_log_secs) {
            return Err(Error::Config(
                "process.status_log_secs must be from 1 to 3600".into(),
            ));
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
        named_key(&self.input.chat_key)?;
        named_key(&self.input.team_modifier)?;
        if self.coach.api_base.trim().is_empty() {
            return Err(Error::Config("coach.api_base must not be empty".into()));
        }
        if self.coach.connect_timeout_secs == 0
            || self.coach.connect_timeout_secs > 120
            || self.coach.read_timeout_secs == 0
            || self.coach.read_timeout_secs > 180
        {
            return Err(Error::Config(
                "coach connect_timeout_secs must be 1..=120 and read_timeout_secs 1..=180".into(),
            ));
        }
        if !(1..=50).contains(&self.coach.list_cap) {
            return Err(Error::Config("coach.list_cap must be from 1 to 50".into()));
        }
        if self.coach.lead_secs > 120 || self.coach.skip_grace_secs > 120 {
            return Err(Error::Config(
                "coach.lead_secs and skip_grace_secs must be from 0 to 120".into(),
            ));
        }
        if !(20..=5_000).contains(&self.coach.tick_ms) {
            return Err(Error::Config(
                "coach.tick_ms must be from 20 to 5000".into(),
            ));
        }
        if !(20..=2_000).contains(&self.coach.speech_chars)
            || !(8..=240).contains(&self.coach.chat_chars)
        {
            return Err(Error::Config(
                "coach.speech_chars must be 20..=2000 and chat_chars 8..=240".into(),
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

    /// `(chat key, optional modifier)`. The modifier is set only for team chat.
    pub fn chat_chord(&self) -> Result<(u16, Option<u16>)> {
        let key = named_key(&self.input.chat_key)?;
        let modifier = if self.input.chat == ChatTarget::Team {
            Some(named_key(&self.input.team_modifier)?)
        } else {
            None
        };
        Ok((key, modifier))
    }

    pub fn guides_client(&self) -> crate::guides::GuidesClient {
        crate::guides::GuidesClient::with_timeouts(
            &self.coach.api_base,
            Duration::from_secs(self.coach.connect_timeout_secs),
            Duration::from_secs(self.coach.read_timeout_secs),
        )
    }
}

/// Virtual-key code for a chat key or modifier. `none` is not allowed.
pub fn named_key(name: &str) -> Result<u16> {
    match name.trim().to_ascii_lowercase().as_str() {
        "enter" | "return" => Ok(0x0D),
        "shift" => Ok(0x10),
        "ctrl" | "control" => Ok(0x11),
        "alt" => Ok(0x12),
        "space" => Ok(0x20),
        "tab" => Ok(0x09),
        _ => parse_hotkey(name)?.ok_or_else(|| {
            Error::Config(format!(
                "key '{name}' is not enter, shift, ctrl, alt, space, tab, or F1-F24"
            ))
        }),
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
        assert_eq!(cfg.input.chat_key, "enter");
        assert_eq!(cfg.input.team_modifier, "shift");
        assert_eq!(cfg.chat_chord().unwrap(), (0x0D, None));
        assert_eq!(cfg.coach.list_cap, 10);
        assert_eq!(cfg.coach.speech_chars, 400);
        assert_eq!(cfg.coach.chat_chars, 120);
        assert_eq!(cfg.coach.skip_grace_secs, 1);
        assert_eq!(cfg.process.status_log_secs, 5);
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
    fn omitted_tunables_use_the_compiled_defaults() {
        let cfg = Config::parse(
            r#"
capture:
  minimap: {x: 0.0, y: 0.0, width: 1.0, height: 1.0}
  town_center: {cx: 0.5, cy: 0.5, radius: 0.5}
colors:
  enemies:
    - {name: red, h_min: 345, h_max: 15, s_min: 0.4, s_max: 1, v_min: 0.3, v_max: 1}
taunts:
  - {id: under_attack, command: "/63", cooldown_secs: 45}
"#,
        )
        .unwrap();
        assert_eq!(cfg.process.name, "RelicCardinal.exe");
        assert!(cfg.input.dry_run);
        assert_eq!(cfg.input.chat_key, "enter");
        assert_eq!(cfg.input.team_modifier, "shift");
        assert_eq!(cfg.coach.api_base, crate::guides::DEFAULT_API_BASE);
        assert_eq!(cfg.coach.cache_dir, "cache");
        assert_eq!(cfg.coach.connect_timeout_secs, 10);
        assert_eq!(cfg.coach.read_timeout_secs, 25);
        assert_eq!(cfg.coach.list_cap, 10);
        assert_eq!(cfg.coach.skip_grace_secs, 1);
        assert_eq!(cfg.coach.tick_ms, 100);
        assert_eq!(cfg.coach.speech_chars, 400);
        assert_eq!(cfg.coach.chat_chars, 120);
        assert_eq!(cfg.coach.step_hotkey, "F13");
        assert_eq!(cfg.detection.consecutive_frames, 3);
    }

    #[test]
    fn hotkeys_cover_the_function_row() {
        assert_eq!(parse_hotkey("F13").unwrap(), Some(0x7C));
        assert_eq!(parse_hotkey("f1").unwrap(), Some(0x70));
        assert_eq!(parse_hotkey("none").unwrap(), None);
        assert!(parse_hotkey("Enter").is_err());
        assert_eq!(named_key("Enter").unwrap(), 0x0D);
        assert_eq!(named_key("shift").unwrap(), 0x10);
        assert!(named_key("none").is_err());
    }
}
