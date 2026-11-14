//! Command loops. Detection and scheduling stay in the pure modules;
//! this file only wires them to a [`Platform`] and the console.

use crate::coach::{announcement, clip_chars, sanitize_chat, BuildOrder, Scheduler};
use crate::config::{parse_hotkey, AnnounceChannel, ChatTarget, CoachMode, Config};
use crate::frame::{fraction_to_pixels, Image};
use crate::guides::{self, GuidesClient};
use crate::platform::{HostPlatform, Platform};
use crate::ppm::{read_ppm, write_ppm};
use crate::taunt::TauntBrain;
use crate::vision::detect;
use crate::{Error, Result};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub fn geometry_report(screen_w: u32, screen_h: u32, cfg: &Config) -> String {
    let rect = fraction_to_pixels(screen_w, screen_h, &cfg.capture.minimap);
    let (cx, cy, radius) = cfg
        .capture
        .town_center
        .pixels(rect.width as usize, rect.height as usize);
    format!(
        "monitor: {screen_w}x{screen_h}\n\
         minimap crop: x={} y={} w={} h={}\n\
         town center: center=({cx:.1}, {cy:.1}) radius={radius:.1} px inside the crop",
        rect.x, rect.y, rect.width, rect.height
    )
}

pub fn describe_image(cfg: &Config, image: &Image) -> String {
    let params = cfg.detector();
    let blobs = match detect(image, &params) {
        Ok(blobs) => blobs,
        Err(err) => return format!("detection failed: {err}"),
    };
    let mut lines = vec![format!(
        "frame {}x{}, near clusters: {}",
        image.width,
        image.height,
        blobs.len()
    )];
    for blob in &blobs {
        let (x, y) = blob.centroid();
        lines.push(format!(
            "  area={} centroid=({x:.1},{y:.1}) bbox=({},{})-({},{})",
            blob.area, blob.min_x, blob.min_y, blob.max_x, blob.max_y
        ));
    }
    let rules = cfg.rules();
    let mut brain = TauntBrain::new(1);
    match brain.observe(&rules, blobs.len(), Instant::now()) {
        Some(index) => lines.push(format!(
            "would send {} ({})",
            rules[index].command, rules[index].label
        )),
        None => lines.push("no taunt".into()),
    }
    lines.join("\n")
}

pub fn run_taunt(
    cfg: &Config,
    image: Option<&Path>,
    dump: Option<&Path>,
    once: bool,
    stop: &AtomicBool,
    chat: &Mutex<()>,
) -> Result<()> {
    if let Some(path) = image {
        let frame = read_ppm(path)?;
        println!("{}", describe_image(cfg, &frame));
        return Ok(());
    }
    let platform = HostPlatform;
    platform.prepare();
    if !platform.supports_capture() {
        return Err(Error::Platform(
            "live taunt capture requires Windows. Use `taunt --image crop.ppm` to test detection on this OS.".into(),
        ));
    }
    if let Some(path) = dump {
        let frame = capture_minimap(cfg, &platform)?;
        write_ppm(path, &frame)?;
        println!("wrote minimap crop to {}", path.display());
        println!("{}", describe_image(cfg, &frame));
        return Ok(());
    }
    banner(cfg);
    let (w, h) = platform.monitor_size()?;
    println!("{}", geometry_report(w, h, cfg));
    if cfg.input.dry_run {
        println!("input.dry_run is true: taunts are logged and not typed into the game.");
    }
    let mut brain = TauntBrain::new(if once {
        1
    } else {
        cfg.detection.consecutive_frames
    });
    let rules = cfg.rules();
    let mut next_wait_log = Instant::now();
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        if !platform.process_running(&cfg.process.name)? {
            if Instant::now() >= next_wait_log {
                println!("waiting for {}", cfg.process.name);
                next_wait_log = Instant::now() + Duration::from_secs(5);
            }
            wait(stop, cfg.poll());
            continue;
        }
        if cfg.process.require_foreground && !platform.is_foreground(&cfg.process.name)? {
            wait(stop, cfg.poll());
            continue;
        }
        let frame = capture_minimap(cfg, &platform)?;
        let blobs = detect(&frame, &cfg.detector())?;
        if let Some(index) = brain.observe(&rules, blobs.len(), Instant::now()) {
            let rule = &rules[index];
            if cfg.input.dry_run {
                println!(
                    "dry-run taunt {}: {} ({})",
                    rule.id, rule.command, rule.label
                );
            } else {
                let _guard = chat.lock().unwrap_or_else(|err| err.into_inner());
                platform.send_chat(
                    &rule.command,
                    cfg.input.chat == ChatTarget::Team,
                    cfg.key_delay(),
                    cfg.chat_open_delay(),
                )?;
                println!("sent taunt {}: {} ({})", rule.id, rule.command, rule.label);
            }
        } else {
            log::debug!("near clusters: {}", blobs.len());
        }
        if once {
            println!("{}", describe_image(cfg, &frame));
            break;
        }
        wait(stop, cfg.poll());
    }
    Ok(())
}

pub fn run_coach(
    cfg: &Config,
    build_id: &str,
    channel: Option<AnnounceChannel>,
    start_at: Duration,
    stop: &AtomicBool,
) -> Result<()> {
    let id = guides::normalize_build_id(build_id)?;
    let client = GuidesClient::new(&cfg.coach.api_base);
    let build = guides::fetch_build(&client, Path::new(&cfg.coach.cache_dir), &id)?;
    let channel = channel.unwrap_or(cfg.coach.channel);
    run_coach_with_build(cfg, &build, channel, start_at, stop, &Mutex::new(()))
}

pub fn run_both(
    cfg: &Config,
    build_id: &str,
    start_at: Duration,
    stop: &Arc<AtomicBool>,
) -> Result<()> {
    let platform = HostPlatform;
    platform.prepare();
    if !platform.supports_capture() {
        return Err(Error::Platform(
            "`both` includes the taunt watcher, which needs Windows. Run `coach run` on this OS."
                .into(),
        ));
    }
    let id = guides::normalize_build_id(build_id)?;
    let client = GuidesClient::new(&cfg.coach.api_base);
    let build = guides::fetch_build(&client, Path::new(&cfg.coach.cache_dir), &id)?;
    let chat = Arc::new(Mutex::new(()));
    let coach_cfg = cfg.clone();
    let coach_stop = Arc::clone(stop);
    let coach_chat = Arc::clone(&chat);
    let coach = std::thread::spawn(move || {
        run_coach_with_build(
            &coach_cfg,
            &build,
            coach_cfg.coach.channel,
            start_at,
            &coach_stop,
            &coach_chat,
        )
    });
    let taunt_result = run_taunt(cfg, None, None, false, stop, &chat);
    stop.store(true, Ordering::SeqCst);
    let coach_result = coach
        .join()
        .map_err(|_| Error::Message("coach thread panicked".into()))?;
    taunt_result?;
    coach_result
}

pub fn run_check(cfg: &Config) -> Result<()> {
    println!(
        "config: ok (dry_run={}, process={}, channel={:?}, mode={:?})",
        cfg.input.dry_run, cfg.process.name, cfg.coach.channel, cfg.coach.mode
    );
    let client = GuidesClient::new(&cfg.coach.api_base);
    let status = client.status()?;
    println!("api: {status} ({})", cfg.coach.api_base);
    let platform = HostPlatform;
    platform.prepare();
    if platform.supports_capture() {
        match platform.monitor_size() {
            Ok((w, h)) => println!("{}", geometry_report(w, h, cfg)),
            Err(err) => println!("monitor: {err}"),
        }
        match platform.process_running(&cfg.process.name) {
            Ok(true) => {
                let foreground = platform.is_foreground(&cfg.process.name).unwrap_or(false);
                println!("{}: running (foreground={foreground})", cfg.process.name);
            }
            Ok(false) => println!("{}: not running", cfg.process.name),
            Err(err) => println!("process check: {err}"),
        }
    } else {
        println!("capture: not available on this OS");
        println!("process: not checked");
    }
    Ok(())
}

pub fn install_ctrlc(stop: Arc<AtomicBool>) {
    if let Err(err) = ctrlc::set_handler(move || {
        stop.store(true, Ordering::SeqCst);
    }) {
        log::warn!("ctrl-c handler not installed: {err}");
    }
}

fn run_coach_with_build(
    cfg: &Config,
    build: &BuildOrder,
    channel: AnnounceChannel,
    start_at: Duration,
    stop: &AtomicBool,
    chat: &Mutex<()>,
) -> Result<()> {
    if build.steps.is_empty() {
        return Err(Error::Build(format!("build {} has no steps", build.id)));
    }
    println!(
        "coach: {} [{}] by {} ({} steps, mode={:?}, channel={:?})",
        build.title,
        build.civ,
        build.author,
        build.steps.len(),
        cfg.coach.mode,
        channel
    );
    if !build.source.is_empty() {
        println!("source: {}", build.source);
    }
    let mut sched = Scheduler::new(
        build.steps.clone(),
        Duration::from_secs(cfg.coach.lead_secs),
    );
    let use_lead = cfg.coach.mode == CoachMode::Timer;
    sched.skip_past(start_at, use_lead);
    let skipped = build.steps.len() - sched.remaining();
    if skipped > 0 {
        println!("skipped {skipped} steps already behind {start_at:?}");
    }
    if sched.remaining() == 0 {
        println!("nothing left to announce");
        return Ok(());
    }
    let platform = HostPlatform;
    platform.prepare();
    let hotkey = parse_hotkey(&cfg.coach.step_hotkey)?;
    let mut hotkey_down = false;
    let (stdin_tx, stdin_rx) = std::sync::mpsc::channel::<()>();
    if cfg.coach.mode == CoachMode::Hotkey {
        println!(
            "hotkey mode: press Enter in this console, or {} when it is polled",
            cfg.coach.step_hotkey
        );
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            for _line in stdin.lines() {
                if stdin_tx.send(()).is_err() {
                    break;
                }
            }
        });
    }
    let origin = Instant::now();
    while !stop.load(Ordering::SeqCst) {
        let steps = match cfg.coach.mode {
            CoachMode::Timer => {
                let elapsed = start_at.saturating_add(origin.elapsed());
                sched.poll(elapsed)
            }
            CoachMode::Hotkey => {
                let mut go = stdin_rx.try_recv().is_ok();
                if let Some(vk) = hotkey {
                    let down = platform.hotkey_is_down(vk).unwrap_or(false);
                    if down && !hotkey_down {
                        go = true;
                    }
                    hotkey_down = down;
                }
                if go {
                    sched.advance().into_iter().collect()
                } else {
                    Vec::new()
                }
            }
        };
        for step in steps {
            announce(cfg, &platform, chat, channel, &announcement(&step))?;
        }
        if sched.remaining() == 0 {
            println!("coach finished");
            break;
        }
        wait(stop, Duration::from_millis(100));
    }
    Ok(())
}

fn announce(
    cfg: &Config,
    platform: &HostPlatform,
    chat: &Mutex<()>,
    channel: AnnounceChannel,
    speech: &str,
) -> Result<()> {
    let speech = clip_chars(speech, 400);
    println!("{speech}");
    if matches!(channel, AnnounceChannel::Tts | AnnounceChannel::Both) && platform.supports_tts() {
        if let Err(err) = platform.speak(&speech) {
            log::warn!("tts failed: {err}");
        }
    }
    if matches!(channel, AnnounceChannel::Chat | AnnounceChannel::Both) {
        let line = sanitize_chat(&speech, 120);
        if cfg.input.dry_run {
            println!("dry-run chat: {line}");
        } else if platform.supports_input() {
            let _guard = chat.lock().unwrap_or_else(|err| err.into_inner());
            platform.send_chat(
                &line,
                cfg.input.chat == ChatTarget::Team,
                cfg.key_delay(),
                cfg.chat_open_delay(),
            )?;
        } else {
            return Err(Error::Platform(
                "coach channel includes chat, but typing into the game is Windows-only. Use channel: tts, or set input.dry_run: true.".into(),
            ));
        }
    }
    Ok(())
}

fn capture_minimap(cfg: &Config, platform: &HostPlatform) -> Result<Image> {
    let (w, h) = platform.monitor_size()?;
    let rect = fraction_to_pixels(w, h, &cfg.capture.minimap);
    platform.capture_rect(rect)
}

fn banner(cfg: &Config) {
    println!(
        "aoe4-agent taunt watcher (poll {} ms, {} taunt{}, foreground required: {})",
        cfg.capture.poll_ms,
        cfg.taunts.len(),
        if cfg.taunts.len() == 1 { "" } else { "s" },
        cfg.process.require_foreground
    );
}

fn wait(stop: &AtomicBool, total: Duration) {
    let slice = Duration::from_millis(50);
    let start = Instant::now();
    while start.elapsed() < total && !stop.load(Ordering::SeqCst) {
        let left = total.saturating_sub(start.elapsed());
        std::thread::sleep(slice.min(left));
    }
}

pub fn print_build_list(builds: &[crate::coach::BuildSummary]) {
    if builds.is_empty() {
        println!("no builds returned (the API serves at most 10 per call)");
        return;
    }
    println!(
        "{:<22} {:<6} {:>8} {:>8}  title",
        "id", "civ", "score", "views"
    );
    for build in builds {
        let title = clip_chars(&build.title, 72);
        println!(
            "{:<22} {:<6} {:>8.2} {:>8}  {}",
            build.id, build.civ, build.score, build.views, title
        );
    }
}

pub fn print_build(build: &BuildOrder) {
    println!("{} [{}]", build.title, build.civ);
    if !build.author.is_empty() {
        println!("author: {}", build.author);
    }
    if !build.source.is_empty() {
        println!("source: {}", build.source);
    }
    if !build.description.is_empty() {
        println!("{}", clip_chars(&build.description, 240));
    }
    for (index, step) in build.steps.iter().enumerate() {
        println!("{:2}. {}", index + 1, announcement(step));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    const EXAMPLE: &str = include_str!("../../../config.example.yaml");

    #[test]
    fn example_config_detects_a_red_cluster_near_the_town_center() {
        let cfg = Config::parse(EXAMPLE).unwrap();
        let mut image = Image::solid(100, 100, [20, 20, 20]).unwrap();
        image.fill_rect(25, 75, 6, 6, [255, 0, 0]);
        let report = describe_image(&cfg, &image);
        assert!(report.contains("near clusters: 1"), "{report}");
        assert!(report.contains("would send /63"), "{report}");

        let mut blue = Image::solid(100, 100, [20, 20, 20]).unwrap();
        blue.fill_rect(25, 75, 6, 6, [0, 0, 255]);
        let report = describe_image(&cfg, &blue);
        assert!(report.contains("near clusters: 0"), "{report}");
        assert!(report.contains("no taunt"), "{report}");

        let mut far = Image::solid(100, 100, [20, 20, 20]).unwrap();
        far.fill_rect(90, 2, 6, 6, [255, 0, 0]);
        let report = describe_image(&cfg, &far);
        assert!(report.contains("no taunt"), "{report}");
    }

    #[test]
    fn geometry_report_names_the_crop() {
        let cfg = Config::parse(EXAMPLE).unwrap();
        let report = geometry_report(1920, 1080, &cfg);
        assert!(report.contains("monitor: 1920x1080"), "{report}");
        assert!(report.contains("minimap crop:"), "{report}");
        assert!(report.contains("town center:"), "{report}");
    }
}
