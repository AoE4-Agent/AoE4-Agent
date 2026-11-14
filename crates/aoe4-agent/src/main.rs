//! CLI for the AoE4 taunt watcher and build-order coach.
//!
//! `input.dry_run` defaults to true. Typing into the game is a Windows-only
//! path and can conflict with anti-cheat; see plan.md before turning it off.

use aoe4_agent::coach::parse_clock;
use aoe4_agent::config::{init_logging, AnnounceChannel, Config};
use aoe4_agent::engine::{self, install_ctrlc};
use aoe4_agent::guides::{self, GuidesClient, ListQuery};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "aoe4-agent",
    version,
    about = "Taunt watcher and build-order coach for Age of Empires IV",
    after_help = "Typing into the game can violate the game's terms and may be flagged by anti-cheat. \
The shipped config leaves input.dry_run true. See plan.md."
)]
struct Cli {
    /// Path to the YAML config.
    #[arg(long, global = true, default_value = "config.yaml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Watch the minimap and send taunts while RelicCardinal.exe is running.
    Taunt {
        /// Treat this PPM as an already-cropped minimap, print detections, and exit.
        #[arg(long)]
        image: Option<PathBuf>,
        /// Capture one minimap crop, write it as a PPM, and exit.
        #[arg(long)]
        dump: Option<PathBuf>,
        /// Capture or decide once instead of looping. Implies a one-frame gate.
        #[arg(long)]
        once: bool,
        /// With --geometry-height, print the crop for this monitor width and exit.
        #[arg(long)]
        geometry_width: Option<u32>,
        /// With --geometry-width, print the crop for this monitor height and exit.
        #[arg(long)]
        geometry_height: Option<u32>,
    },
    /// Fetch and announce build orders from aoe4guides.
    Coach {
        #[command(subcommand)]
        action: CoachCmd,
    },
    /// Run the taunt watcher and a coach together.
    Both {
        /// Build id from `coach list`, or an aoe4guides.com build URL.
        #[arg(long)]
        build_id: String,
        /// Game-clock offset, so a late start does not re-read the opening.
        #[arg(long, default_value = "0:00")]
        start_at: String,
    },
    /// Parse the config, ping the guides API, and on Windows report the game process.
    Check,
}

#[derive(Subcommand)]
enum CoachCmd {
    /// List up to 10 builds. The API itself caps each response at 10.
    List {
        /// Civilization short code, for example ENG or FRE.
        #[arg(long)]
        civ: Option<String>,
        /// How many rows to print, from 1 to 10.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// score, views, likes, or timeCreated.
        #[arg(long, default_value = "score")]
        order_by: String,
    },
    /// Announce a build on the configured channel.
    Run {
        #[arg(long)]
        build_id: String,
        /// Override coach.channel: tts, chat, or both.
        #[arg(long)]
        channel: Option<ChannelArg>,
        #[arg(long, default_value = "0:00")]
        start_at: String,
    },
    /// Print the normalized steps without speaking them.
    Show {
        #[arg(long)]
        build_id: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ChannelArg {
    Tts,
    Chat,
    Both,
}

impl From<ChannelArg> for AnnounceChannel {
    fn from(value: ChannelArg) -> Self {
        match value {
            ChannelArg::Tts => AnnounceChannel::Tts,
            ChannelArg::Chat => AnnounceChannel::Chat,
            ChannelArg::Both => AnnounceChannel::Both,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let cfg = match Config::load(&cli.config) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(1);
        }
    };
    init_logging(&cfg.log.level);
    if let Err(err) = dispatch(&cli.command, &cfg) {
        eprintln!("error: {err}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn dispatch(command: &Command, cfg: &Config) -> aoe4_agent::Result<()> {
    match command {
        Command::Taunt {
            image,
            dump,
            once,
            geometry_width,
            geometry_height,
        } => {
            match (geometry_width, geometry_height) {
                (Some(w), Some(h)) => {
                    println!("{}", engine::geometry_report(*w, *h, cfg));
                    return Ok(());
                }
                (Some(_), None) | (None, Some(_)) => {
                    return Err(aoe4_agent::Error::Config(
                        "pass both --geometry-width and --geometry-height".into(),
                    ));
                }
                (None, None) => {}
            }
            let stop = Arc::new(AtomicBool::new(false));
            if image.is_none() && dump.is_none() {
                install_ctrlc(Arc::clone(&stop));
            }
            engine::run_taunt(
                cfg,
                image.as_deref(),
                dump.as_deref(),
                *once,
                &stop,
                &std::sync::Mutex::new(()),
            )
        }
        Command::Coach { action } => match action {
            CoachCmd::List {
                civ,
                limit,
                order_by,
            } => {
                if *limit == 0 {
                    return Err(aoe4_agent::Error::Config(
                        "--limit must be at least 1".into(),
                    ));
                }
                if *limit > 10 {
                    println!("the guides API returns at most 10 builds; showing 10");
                }
                let client = GuidesClient::new(&cfg.coach.api_base);
                let builds = client.list_builds(&ListQuery {
                    civ: civ.clone(),
                    order_by: guides::normalize_order_by(order_by)?,
                    limit: *limit,
                })?;
                engine::print_build_list(&builds);
                Ok(())
            }
            CoachCmd::Run {
                build_id,
                channel,
                start_at,
            } => {
                let stop = Arc::new(AtomicBool::new(false));
                install_ctrlc(Arc::clone(&stop));
                engine::run_coach(
                    cfg,
                    build_id,
                    channel.map(AnnounceChannel::from),
                    parse_clock(start_at)?,
                    &stop,
                )
            }
            CoachCmd::Show { build_id } => {
                let id = guides::normalize_build_id(build_id)?;
                let client = GuidesClient::new(&cfg.coach.api_base);
                let build =
                    guides::fetch_build(&client, std::path::Path::new(&cfg.coach.cache_dir), &id)?;
                engine::print_build(&build);
                Ok(())
            }
        },
        Command::Both { build_id, start_at } => {
            let stop = Arc::new(AtomicBool::new(false));
            install_ctrlc(Arc::clone(&stop));
            engine::run_both(cfg, build_id, parse_clock(start_at)?, &stop)
        }
        Command::Check => engine::run_check(cfg),
    }
}
