# AoE4 Agent — implementation plan

Age of Empires IV companion with two jobs: notice an enemy cluster on the minimap near the town center and send a taunt, and read a build order from [aoe4guides.com](https://aoe4guides.com) out loud or into chat. Version 0.1.0 is the first cut of that design. It is a local process the player starts. It does not attach to the game, read game memory, or ship a network server.

## Goals for 0.1.0

- One Rust workspace, one binary: `aoe4-agent`.
- `taunt`, `coach`, and `both` subcommands, plus `check`.
- A YAML config for the minimap crop, enemy colors, taunt commands, cooldowns, and coach timing.
- Detection written as pure functions (HSV mask, connected components, cooldown, build-order timing) with unit tests that run on Linux.
- Windows capture, process watch, key input, and speech behind `cfg(windows)`, so the same crate cross-compiles with mingw and builds with MSVC.
- A GitHub Actions workflow that builds the release `.exe` on `windows-latest` and, on a `v*` tag, attaches it to a release. Binaries are not committed.

## Non-goals for 0.1.0

- Reading the game's memory, packets, or replay files.
- A drawn overlay, DXGI desktop duplication, or automatic minimap finding.
- Knowing whether a match is actually in progress beyond "the process exists and is in the foreground".
- Syncing the coach to the on-screen game clock.
- Linux capture or input. Linux runs the coach as console text and the detector on a PPM file.

## Architecture

```text
config.yaml
    │
    ▼
aoe4-agent  ── taunt ──► process watch ──► GDI crop of the minimap
                │                              │
                │                              ▼
                │                         HSV mask + components
                │                              │
                │                              ▼
                │                     streak + cooldown gate
                │                              │
                │                              ▼
                │                     Enter, "/63", Enter     (or dry-run log)
                │
                └── coach ──► aoe4guides API (cached) ──► timer or hotkey
                                    │
                                    ▼
                              println + SAPI and/or chat
```

`both` runs those two loops together. They share a mutex around `SendInput` so a taunt and a coach line cannot type over each other.

The workspace is one package, `crates/aoe4-agent`, with a library and a binary of the same name. Splitting vision and the coach into more crates would not change the link, and a single package keeps `cargo test` and the VS Code launch profiles (`--package=aoe4-agent --bin=aoe4-agent`) pointed at one place.

| Module | Responsibility | Runs on Linux tests |
| --- | --- | --- |
| `frame` | RGB image, fractional rectangles, town-center disk | yes |
| `vision` | HSV, mask, union-find components, "near the town center" | yes |
| `taunt` | Per-taunt cooldown and the consecutive-frame streak | yes |
| `coach` | Clock parser, markup cleanup, native and overlay JSON, scheduler | yes |
| `guides` | HTTP client, id checks, on-disk cache | parser and cache tests; live call is manual |
| `config` | YAML schema and validation | yes, against `config.example.yaml` |
| `platform` | Process, BitBlt, SendInput, SAPI, or a stub | stub compiles everywhere; Win32 compiles for `cfg(windows)` |
| `engine` | The loops that glue the pieces to the console | geometry and a synthetic minimap |

## Taunt pipeline

1. Resolve `capture.minimap` (fractions of the primary monitor) to a pixel rectangle. `fraction_to_pixels` clamps the rectangle so rounding cannot walk off the monitor. The same fractions are checked against 720p, 1080p, 1440p, ultrawide, and 4K in tests.
2. On Windows, mark the process per-monitor DPI aware, then `BitBlt` only that rectangle and read it back as top-down RGB.
3. Convert each pixel to HSV (hue in degrees, saturation and value in 0..=1). A range with `h_min > h_max` wraps, which is how red is expressed (345 through 15). Pixels that also match `colors.own` are dropped first, so the player's own villagers do not count as an attack.
4. Label the mask with 8-connected components (4-connected is a config switch). Drop blobs smaller than `min_cluster_pixels`. Optionally keep only blobs whose centroid lies inside the town-center disk. The radius is a fraction of the crop's shorter side, so it scales with the crop.
5. `TauntBrain` increments a streak while any rule's `min_clusters` is met, and clears it on a quiet frame. After `consecutive_frames` hits, the first rule that is off cooldown fires. Firing consumes that rule's cooldown and clears the streak. A later rule can fire on a following frame if the first is cooling down and the cluster count still qualifies.
6. The default command is `/63`, which AoE4 turns into "I'm under attack!" ([Age of Empires support](https://support.ageofempires.com/hc/en-us/articles/10091280315924-Taunts-Cheats) requires the slash; community taunt sheets list 63 as that line). The command is config, not a constant in the detector.

`taunt --image crop.ppm` runs steps 3–5 on a file and prints the decision. It never types. `taunt --dump crop.ppm` captures one crop on Windows and writes a PPM. `taunt --once` is a single live frame with the streak gate forced to 1, for checking a real minimap without waiting.

## How a taunt gets into game chat

This was an open question. 0.1.0 answers it with synthetic keyboard input, and documents where that answer is still a guess.

AoE4's taunt system is chat text. Support's rule is: open chat, type a slash and the taunt number, send. The agent does that with `SendInput`:

1. Tap Enter (`VK_RETURN`) to open chat.
2. Wait `chat_open_delay_ms` so the input field can take focus.
3. Type the command as Unicode key events, `key_delay_ms` apart.
4. Tap Enter again to send.

`input.chat: team` holds Shift around both Enter taps. That is the usual team-chat chord in this series, and it is **not** verified against the current AoE4 build. If Shift+Enter does something else on a given patch, leave `chat: all` or keep `dry_run: true` and use TTS for the coach. Rebinding the chat key in the game's settings also breaks this path; the agent does not read the player's keybinds.

The game window has to already be in the foreground. The agent does not call `SetForegroundWindow`. Windows User Interface Privilege Isolation blocks `SendInput` when the game is elevated and the agent is not, so both should run as a normal user (or both elevated). Exclusive fullscreen often makes GDI captures black; use borderless, which is AoE4's default.

`input.dry_run` defaults to **true** in code and in the shipped config. A dry run logs `dry-run taunt under_attack: /63` and still consumes the cooldown, so the log does not spam every frame. Set it to false only to actually type.

## Coach and the guides API

Checked against the live service on 2026-11-14.

`https://aoe4guides.com/api/builds` (the list) is **not** proxied. The site responds with JSON naming the host that does serve the API:

`https://aoe4-guides-api-7h2vti5ckq-ey.a.run.app`

That host returns:

- `GET /status` → `{"status":"running"}`
- `GET /builds?civ=ENG&orderBy=score` → a JSON array of at most 10 native build orders. `orderBy` is `score`, `timeCreated`, `views`, or `likes`. `civ` is a short code (`ENG`, `FRE`, …). The set of codes grows with the game; unknown codes are passed through rather than rejected.
- `GET /builds/{id}` → one native document: `steps[]` of groups (`type` `age` or `ageUp`, `age`, `gameplan`, nested `steps` with `time`, resource fields, HTML `description`).
- `GET /builds/{id}?overlay=true` → the flat shape overlay tools use: `build_order[]` with `time`, `age`, `villager_count`, `resources`, and `notes`. Notes are `@resource/sheep.webp@` tokens, not HTML.

The client prefers the overlay document and falls back to native. Both become `BuildStep { at, age, villagers, resources, notes }`. HTML `<img title="…">` and `<br>` and the `@token@` dialect are reduced to speech by `clean_note`. Empty `ageUp` groups are skipped. A negative or blank count means "not set" and is left out of the sentence.

Fair use, from the site's own about page: the API is free, unauthenticated, and asks callers to cache, stay inside the rate limit, and not walk the whole library. The agent sends `User-Agent: aoe4-agent/0.1.0`, caches the raw JSON at `cache/<id>.json`, and reads that file if the next request fails. List calls are not crawled.

Timing:

- `mode: timer` starts a clock when the command starts. A step is due when `elapsed + lead_secs >= step.at`. `lead_secs: 2` means the 0:35 step is spoken at 0:33. `--start-at 5:00` skips steps whose speak-time is already more than a second behind, so joining late does not recite the whole build.
- `mode: hotkey` ignores the clock for firing. Enter in the agent console, or the polled `step_hotkey` (default F13, via `GetAsyncKeyState`, edge-triggered), advances one step. This is the way to keep the coach aligned with the player when the timer would drift.

Every step is printed. TTS (Windows SAPI through a short PowerShell script, no console window) runs when `channel` is `tts` or `both`. Chat injection runs when `channel` is `chat` or `both`, still subject to `dry_run`, and clipped to one 120-character line. The shipped channel is `tts`, which does not type into the game.

## Config

`config.example.yaml` is the schema. `config.yaml` is a committed copy so the VS Code launch profiles work without a copy step. Unknown keys are errors. Validation covers fraction ranges, hue wrap, cooldown bounds, duplicate taunt ids, and the hotkey name (`F1`–`F24` or `none`).

The color defaults assume unique colors and a blue player. That is a starting point, not a measurement. See calibration below.

## Windows and other targets

| Piece | Windows | Elsewhere |
| --- | --- | --- |
| `RelicCardinal.exe` via Toolhelp | yes | `process_running` is false |
| Primary-monitor size and GDI crop | yes | error, with a message that points at `--image` |
| `SendInput` | yes | error |
| SAPI | yes | `speak` is a no-op; the line is still printed |
| HSV, components, cooldowns, coach parser | yes | yes |

GDI rather than DXGI (`xcap`) and `windows-sys` rather than `enigo` is deliberate. Those crates want the Windows SDK and are a poor fit for a mingw cross build. `BitBlt` plus `SendInput` links with both mingw and MSVC, which is what the local cross compile and the `windows-latest` workflow need. DXGI is the fallback if a borderless capture ever comes back black; it is not in 0.1.0.

`.cargo/config.toml` sets the mingw linker for `x86_64-pc-windows-gnu` only. The GitHub Actions job does not use that target. It builds with the default MSVC toolchain on `windows-latest`, runs `cargo test`, uploads `target/release/aoe4-agent.exe`, and on tags `v*` publishes that file on the release.

## Milestones

**0.1.0 (this tree).** CLI, config, pure detector, coach client, Windows GDI/SendInput/SAPI, tests, release workflow.

**0.2.** A calibration aid that is better than a PPM: freeze a frame, let the user drag the crop, write the fractions back. Optional DXGI backend if BitBlt is black under a future fullscreen mode. Record a few real minimap crops (unique colors and team colors) and replace the guessed HSV bounds with measured ones.

**0.3.** In-match detection. Process-plus-foreground still types into the score screen if the game is focused. Candidates: a HUD color check on the same frame, or tailing `warnings.log`. Do not type unless one of those says a match is running.

**0.4.** Coach sync. Timer drift is structural. Next step is either sticking with hotkey mode as the recommended path, or reading the age/villager count (OCR on the HUD, or a log) and advancing the step that matches the player instead of the clock.

**Later.** Linux capture is a different project (PipeWire or X, and uinput). Leave it experimental. Do not promise Proton.

## Risks and open questions

### Typing the taunt

Decided for 0.1.0: Enter, the configured text, Enter, through `SendInput`, only when the game process is in the foreground and `dry_run` is false. Team chat via Shift is unverified. The chat hotkey is assumed to still be Enter. Taunt numbers follow the public taunt list; `/63` is the example because the project brief and the community sheet agree, and the config can hold any other command (`/12`, `/100`, a short text line).

### Anti-cheat and terms of service

AoE4 ships with anti-cheat. Synthetic keystrokes are the kind of input that can be flagged even when the only thing being typed is a taunt or a build step. Whether that happens today is not something this repo can promise. It can break on a patch, and an account penalty would be the player's, not the tool's.

Consequences we accept in the design:

- Dry-run is the default. The interesting half of the coach (TTS) never injects keys.
- Injection is rare: a multi-frame streak, then a cooldown of at least one second (the example uses 45).
- The agent does not click, move the mouse, issue unit orders, or read memory.
- This document and the `--help` epilog say so in plain language.

Microsoft's game-content rules and the AoE4 terms still apply. The project is not affiliated with Microsoft or Relic. Prefer TTS. If you turn `dry_run` off, you are accepting that risk for your own account.

### Minimap calibration across resolutions

Fractions of the primary monitor survive a resolution change: 6% across is 6% across at 1080p and at 4K, and the code clamps rounding so the crop stays on the monitor. They do **not** survive:

- the in-game UI scale slider,
- a moved or hidden HUD,
- the minimap sitting on a different monitor than the primary,
- team colors or a player color other than the configured `own` range,
- a town center that spawned somewhere else on the map (the disk is a fraction of the crop, not a detected building).

0.1.0 does not auto-calibrate. The practical loop is `taunt --dump crop.ppm`, open the image, nudge `capture.minimap`, dump again, then confirm with `taunt --once` while a custom game is on the scoreless borderless window. Set `colors.own` to the color of your units or the detector will treat your villagers as the enemy, or miss the enemy if you deleted the wrong range. For team colors, leave only the opponent's range in `colors.enemies`.

DPI: the process calls `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` before the first capture so `GetSystemMetrics` and `BitBlt` share the game's physical pixels. If that call fails, it falls back to `SetProcessDPIAware`.

### Match state

Process presence plus foreground is not "a match is running". The agent will taunt on the main menu if the menu is focused and the minimap crop happens to contain matching colors. `require_foreground: true` avoids typing into a chat window behind the game. It does not understand the HUD. That is milestone 0.3.

### Coach drift

A wall clock started when you press Enter is not the game clock. Pausing, lag, and starting the coach late all skew it. `lead_secs` only shifts every step by the same amount. Hotkey mode and `--start-at` are the 0.1.0 mitigations. OCR is explicitly later.

### API stability

The Cloud Run host is an implementation detail of aoe4guides and can change. If list calls start failing, hit `https://aoe4guides.com/api/builds` and read the `message` field; it names the current host. Put that in `coach.api_base`. The parser tolerates extra JSON fields and accepts either document shape. A response that is neither is an error, not a crash. Cached copies keep a coach session usable when the network is down.

## Testing

Unit tests cover HSV primaries and the red wrap, 4- vs 8-connectivity, blob merging, town-center distance, cooldowns per id, the streak reset, clock parsing, HTML and `@token@` cleanup, overlay and native documents, scheduler lead time and late-join skipping, build-id path rejection, the example YAML, and a synthetic 100×100 minimap through the example color config (near red sends `/63`, near blue and far red do not).

Not covered by automated tests, because this environment has no game client: a real minimap, BitBlt against borderless AoE4, `SendInput` into the chat box, SAPI audio, and anti-cheat behavior. The Windows code is type-checked by the mingw cross build and compiled again on `windows-latest` in CI.
