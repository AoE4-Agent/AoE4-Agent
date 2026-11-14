# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-11-14

### Added

- README instructions for starting the agent with Age of Empires IV on Steam. The launch option that works is `cmd /c start ... & %command%`. A bare `"aoe4-agent.exe" & %command%` does not, because Steam passes that string to `CreateProcess` and `&` is not a shell operator there.
- `scripts/start-aoe4-agent.bat`, a shortcut fallback that does not go through Steam launch options.
- A staged Testing section: unit tests, `taunt --image` on a saved PPM crop, a dry-run skirmish against the AI, then live chat.

## [0.1.0] - 2026-11-14

### Added

- Implementation plan in `plan.md` for the taunt watcher and the build-order coach, including chat injection, anti-cheat, and minimap calibration.
- `aoe4-agent` CLI with `taunt`, `coach list|run|show`, `both`, and `check`.
- YAML config (`config.example.yaml`) for the minimap crop, HSV ranges, taunt cooldowns, and coach timing.
- Pure-Rust HSV masking, connected-component labeling, cooldown and consecutive-frame gates, and build-order parsing for the aoe4guides native and overlay documents.
- Windows process watch, GDI minimap capture, `SendInput` chat, and SAPI speech, compiled only on Windows. Other targets still build and run the unit tests.
- GitHub Actions workflow that tests on Linux, builds `aoe4-agent.exe` on `windows-latest`, uploads it as an artifact, and attaches it to a release when a `v*` tag is pushed.
