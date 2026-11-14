# AoE4-Agent

Local companion for Age of Empires IV. It watches the minimap and can send a taunt when an enemy cluster shows up near your town center, and it reads build orders from [aoe4guides.com](https://aoe4guides.com) on a timer or a hotkey.

The design, including what is deliberately not solved yet, is in [plan.md](plan.md). Version history is in [CHANGELOG.md](CHANGELOG.md).

## What it does

1. **Taunt.** A background loop looks for `RelicCardinal.exe`, grabs the minimap with a screen copy, and masks enemy colors in HSV. Connected components turn that mask into clusters. A cluster that sits inside a configurable disk around the town center, for several frames in a row, sends a taunt such as `/63` ("I'm under attack!") subject to a per-taunt cooldown.
2. **Coach.** Build orders come from the aoe4guides API. Steps are announced on a clock, or one at a time when you press Enter (or F13). Windows can speak them with SAPI. They can also be typed into chat.

`input.dry_run` ships as `true`. Until you set it to `false`, nothing is typed into the game. Typing can conflict with anti-cheat and with the game's terms. Read that section of [plan.md](plan.md) before changing it. Text-to-speech does not inject keys.

This project is not affiliated with Microsoft, Relic, or aoe4guides.

## Requirements

- Rust stable (the CI workflow uses stable; 1.88 or newer is a safe local choice).
- Windows 10 or 11 to capture the screen, type, or speak. Linux and macOS build, test, and run the coach as console text.
- AoE4 in **borderless** fullscreen on the **primary** monitor. Exclusive fullscreen often captures as a black image.
- Run the game and the agent at the same privilege level. An elevated game ignores keystrokes from a normal process.

On Windows, the MSVC toolchain is the one CI uses:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install Rustlang.Rustup
rustup default stable-x86_64-pc-windows-msvc
```

VS Code launch and build profiles live in `.vscode/` (rust-analyzer, CodeLLDB). `Ctrl+Shift+B` is `cargo build --release`.

## Build

From the repo root:

```bash
cargo test
cargo build --release
```

The Windows release binary is `target/release/aoe4-agent.exe`. CI on `windows-latest` builds that file, uploads it as the artifact `aoe4-agent-windows-x86_64`, and attaches it to a GitHub release when a tag named `v*` is pushed. Release binaries are not committed.

A Linux host can type-check the Windows code with the mingw cross target (this needs `gcc-mingw-w64-x86-64`):

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

That produces `target/x86_64-pc-windows-gnu/release/aoe4-agent.exe`. `.cargo/config.toml` points that target at `x86_64-w64-mingw32-gcc`. The artifact CI publishes is the MSVC build, not this one.

## Run

`config.yaml` matches `config.example.yaml`. Edit the copy you actually launch.

```powershell
# Linux or Windows. Confirms the config and the guides API.
aoe4-agent check

# Crop rectangle at 1080p, from the fractions in the config. No capture.
aoe4-agent taunt --geometry-width 1920 --geometry-height 1080

# One minimap crop, written as a PPM, so you can see whether the fractions are right.
aoe4-agent taunt --dump crop.ppm

# Detector only, on a crop you already saved. Works without Windows.
aoe4-agent taunt --image crop.ppm

# Watch until Ctrl+C. With dry_run true this only logs.
aoe4-agent taunt

# Top English builds. The API returns at most 10.
aoe4-agent coach list --civ ENG --limit 5

# Print every step.
aoe4-agent coach show --build-id BUILD_ID

# Speak (and, if configured, type) the steps. Start it as the game clock hits 0:00,
# or pass --start-at 5:00 to skip the opening.
aoe4-agent coach run --build-id BUILD_ID

# Taunt loop and coach together. Windows only.
aoe4-agent both --build-id BUILD_ID
```

`coach run --channel tts` overrides the config for one launch. `RUST_LOG=debug` overrides `log.level`.

A build id is either the bare id or a guides URL:

```text
aoe4-agent coach show --build-id https://aoe4guides.com/builds/BUILD_ID
```

## Config

| Section | What it controls |
| --- | --- |
| `process` | `RelicCardinal.exe`, and whether the game must be the foreground window |
| `capture.minimap` | Crop as fractions of the primary monitor, origin top-left |
| `capture.town_center` | Disk inside that crop. Radius is a fraction of the shorter side |
| `colors` | Enemy HSV ranges, and your own color (excluded before clustering) |
| `detection` | 4- or 8-connectivity, minimum blob size, consecutive frames |
| `taunts` | Command, label, cooldown, cluster count |
| `input` | `dry_run`, all-chat vs team-chat, key delays |
| `coach` | API host, cache directory, `tts` / `chat` / `both`, `timer` / `hotkey` |

`colors.own` defaults to blue. If you are not blue, change it, or your villagers look like an attack and a blue enemy is invisible. Team colors: keep only the opponent's range in `colors.enemies`.

The fractions track resolution (1080p and 4K use the same file). They do not track the in-game UI scale, a moved HUD, or which side of the map you spawned on. Dump a crop, nudge the numbers, dump again.

`coach.mode: hotkey` waits for Enter in the agent console, or for `step_hotkey` (default F13), instead of trusting a wall clock. The timer drifts if you pause or start late. `--start-at 5:00` skips steps that are already behind.

`coach.api_base` points at the Cloud Run host that actually serves the list API. `aoe4guides.com` only proxies single-build reads and the docs. If that host moves, `GET https://aoe4guides.com/api/builds` returns a `message` naming the new one. Responses are cached under `cache/` (gitignored). Please don't scrape the catalog; the API's fair-use note asks callers to cache and to credit [AoE4 Guides](https://aoe4guides.com).

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

The tests cover HSV, connected components, town-center distance, cooldowns, build-order parsing, and the scheduler. They do not launch AoE4. `platform/windows.rs` is compiled by the mingw target and by the Windows CI job.

## License

[GPL-3.0](LICENSE).

Age of Empires IV © Microsoft Corporation. This project is not endorsed by Microsoft or Relic.
