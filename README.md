# AoE4-Agent

Local companion for Age of Empires IV. It watches the minimap and can send a taunt when an enemy cluster shows up near your town center, and it reads build orders from [aoe4guides.com](https://aoe4guides.com) on a timer or a hotkey.

The design, including what is deliberately not solved yet, is in [plan.md](plan.md). Version history is in [CHANGELOG.md](CHANGELOG.md).

## What it does

1. **Taunt.** A background loop looks for `RelicCardinal.exe`, grabs the minimap with a screen copy, and masks enemy colors in HSV. Connected components turn that mask into clusters. A cluster that sits inside a configurable disk around the town center, for several frames in a row, sends a taunt such as `/63` ("I'm under attack!") subject to a per-taunt cooldown. A draft of a richer per-taunt file, not loaded yet, is in [docs/TAUNT_SCHEMA.md](docs/TAUNT_SCHEMA.md).
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

## Set up with AoE4 on Steam

Age of Empires IV on Steam is app `1466860`. Steam launches `RelicCardinal.exe` from `steamapps\common\Age of Empires IV\`. Put the agent somewhere else, so a game update does not replace it and the game folder stays untouched.

Create `C:\AoE4Agent` (any folder without `&` or `%` in the path) and copy in:

- `aoe4-agent.exe` (the CI artifact, or `target\release\aoe4-agent.exe`)
- `config.yaml` (start from `config.example.yaml`)
- optionally `scripts\start-aoe4-agent.bat`, if you want the shortcut fallback below

Leave `input.dry_run: true` until a dry-run skirmish looks right. The first launches should only log.

### How Steam runs Launch Options

Steam does not run that text box through `cmd.exe`. After it replaces `%command%` with the real game command (the quoted path to `RelicCardinal.exe` plus Steam's own arguments), it passes the whole string to `CreateProcess`. Characters such as `&` are arguments, not shell syntax, unless the program Steam starts is actually `cmd.exe`.

This does **not** start the game:

```text
"C:\AoE4Agent\aoe4-agent.exe" both & %command%
```

`CreateProcess` starts `aoe4-agent.exe` and hands it `both`, `&`, and the game path as arguments. The game never launches. Steam's "Playing" state lasts only as long as that agent process.

This does start both. `cmd /c` is the process Steam creates, so `cmd` is what interprets `&`:

```text
cmd /c start "AoE4 Agent" /D "C:\AoE4Agent" aoe4-agent.exe --config "C:\AoE4Agent\config.yaml" taunt & %command%
```

Paste that into Steam: Library → Age of Empires IV → Properties → General → Launch Options. Edit the two `C:\AoE4Agent` paths if you used another folder. Any flags you want the game itself to see go after `%command%`, because this line replaces the usual "append these flags to the exe" behavior.

What that line does:

1. `start "AoE4 Agent"` opens the agent in its own console. The first quoted string is the window title (`start` requires that). `/D` sets the working directory, and `--config` is absolute so it does not depend on Steam's current directory (the game folder).
2. `start` returns immediately. The agent is not a child Steam will tear down when the game exits.
3. `%command%` then runs `RelicCardinal.exe` in the same `cmd` process, and `cmd` waits for it. Steam waits on `cmd`, so playtime and the Steam overlay follow the game, not the agent.
4. When you quit the game, that `cmd` window closes. The agent does not. Its loop has no "game exited" stop; it prints `waiting for RelicCardinal.exe` and keeps running until you close the "AoE4 Agent" window or press Ctrl+C there. Close it before the next launch, or a second agent starts beside the first.

A `cmd` window stays up for the whole session, because that is the process Steam is waiting on. Minimize it. Closing it ends that process; Steam then treats the session as finished and can close the game with it. Quit from the game's own menu.

`taunt` is the right command in this line. `both --build-id ...` starts the coach clock when Steam starts the game, which is still the main menu, so the build timer is already late when the match clock hits 0:00. Start `coach run` yourself when the match starts, or use `coach.mode: hotkey`.

The `cmd` window flashes and the agent window is separate. That is the working shape. `start /min` in front of the agent name minimizes the agent window if you want it out of the way after the first few runs:

```text
cmd /c start /min "AoE4 Agent" /D "C:\AoE4Agent" aoe4-agent.exe --config "C:\AoE4Agent\config.yaml" taunt & %command%
```

### Fallback: shortcut or start it yourself

Use this when you do not want a launch option. Clear Launch Options first so Steam does not start a second agent.

`scripts\start-aoe4-agent.bat` assumes the exe and `config.yaml` sit next to the script. Copy all three into `C:\AoE4Agent`, then:

- Double-click the script to open only the agent. Press Play in Steam afterwards.
- Or run `start-aoe4-agent.bat game` (or a desktop shortcut whose target ends in `game`). The script starts the agent, then asks Steam to launch app `1466860` with `steam://rungameid/1466860`.

You can also skip the script: open a terminal in `C:\AoE4Agent`, run `aoe4-agent.exe --config C:\AoE4Agent\config.yaml taunt`, then press Play in Steam with empty Launch Options.

Same lifetime either way. Leaving the match does not close the agent.

### In-game settings the capture assumes

The crop is a fraction of the **primary** monitor, and the minimap is assumed to be the default bottom-left HUD.

- Window mode: **Borderless** (AoE4's usual default), not exclusive fullscreen. Exclusive fullscreen often captures as a black image. Windowed works only if the game fills the monitor the fractions were measured on.
- Leave the game on the primary display. A second monitor is not in the 0.1 crop math.
- Do not move or hide the minimap. Interface / UI scale is not inferred. Pick a scale, then calibrate, and dump again if you change it.
- Unique player colors, and set `colors.own` to your color. The shipped file assumes you are blue. Team colors need `colors.enemies` reduced to the opponent's color.
- Run Steam and the agent as a normal user. If Steam is "Run as administrator", a normal agent cannot type into the game.

### First run, still with dry_run on

From `C:\AoE4Agent`:

```powershell
.\aoe4-agent.exe --config C:\AoE4Agent\config.yaml check
```

That checks the YAML and the guides API. It does not need the game. On Windows it also prints the crop in pixels and whether `RelicCardinal.exe` is running.

Start a skirmish (the minimap is not on the main menu), then:

```powershell
.\aoe4-agent.exe --config C:\AoE4Agent\config.yaml taunt --dump C:\AoE4Agent\crop.ppm
```

Open `crop.ppm`. It should be the minimap and little else. If it is the corner of the desktop or a slice of the HUD, change `capture.minimap` and dump again. `taunt --geometry-width` and `--geometry-height` only print the rectangle; they do not look at the screen.

With an enemy force near your town center:

```powershell
.\aoe4-agent.exe --config C:\AoE4Agent\config.yaml taunt --once
```

`--once` forces a single frame, still obeys `dry_run`, and prints whether it would send `/63`. Nothing is typed while `dry_run` is true.

### Anti-cheat

Launch Options are ordinary Steam configuration. AoE4's own content-editor notes tell you to put flags there. This does not turn anti-cheat off, and the agent does not attach to `RelicCardinal.exe` or read its memory.

With `dry_run: true` the process only reads the screen and writes a log. Setting `dry_run: false` types into the game with `SendInput`. AoE4's anti-cheat has closed the client when it saw known cheat tools running, including in a solo skirmish. A taunt is a smaller action than those tools, and it can still be treated as automation. Ranked play with typing enabled is your decision; the shipped config leaves typing off. Do not run Cheat Engine, WeMod, or similar next to the agent. Details are in [plan.md](plan.md).

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

`platform/windows.rs` is compiled by the mingw target and by the Windows CI job. Those commands do not launch AoE4.

## Testing

Work in this order. Each stage adds one real-world unknown, and the earlier stages do not need a match.

1. **Unit tests.** `cargo test` covers HSV masking, connected components, town-center distance, cooldowns, build-order parsing, and the scheduler. No game, and no Windows, required.
2. **A saved crop.** `taunt --dump crop.ppm` during a skirmish writes a binary P6 PPM of the minimap rectangle. `taunt --image crop.ppm` runs the detector on that file and prints whether it would send `/63`. It does not capture or type, so it also works on Linux. `--image` treats the whole file as the minimap crop, and it accepts P6 PPM only. A PNG screenshot has to be cropped to the minimap and converted first (`magick crop.png crop.ppm`). An empty or wrong crop should print `no taunt`; a crop with an enemy-colored cluster inside the town-center disk should name `/63`.
3. **Dry-run against the AI.** Keep `input.dry_run: true`. Start a custom skirmish, not a ranked match, with `aoe4-agent.exe taunt` already running (or via the Steam launch option). Walk an enemy force up to your town center and confirm the log shows the taunt, and that chat in the game stays empty. Quitting the match does not exit the agent; stop it with Ctrl+C.
4. **Live.** Only after step 3 looks right, set `input.dry_run: false` and repeat the same skirmish. Confirm the game actually receives `/63`, then decide whether that is worth the anti-cheat note above. Put `dry_run` back to true when you are done experimenting.

## License

[GPL-3.0](LICENSE).

Age of Empires IV © Microsoft Corporation. This project is not endorsed by Microsoft or Relic.
