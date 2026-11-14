# Per-taunt schema (draft)

This is a proposal for one YAML entry per taunt the agent may send. It is not loaded by the program. `config.yaml` still uses the short `taunts:` list (`id`, `command`, `label`, `cooldown_secs`, `min_clusters`).

The numbered lines themselves are catalogued in `docs/TAUNTS.md` (pull request #6; not on `main` yet). Codes in `taunts.example.yaml` are taken from that catalog. `/63` is "I'm under attack!" and `/12` is "Ah! Being rushed."

## What the code does today

`TauntBrain::observe` in `crates/aoe4-agent/src/taunt.rs` sees one number: how many enemy blobs sit inside the town-center disk. That count already passed the global HSV mask, the minimum blob size, and the crop in `config.yaml`.

The brain then:

1. Treats the frame as active when any rule's `min_clusters` is met.
2. Counts consecutive active frames. The required count is `detection.consecutive_frames` for the whole file, not per taunt. A quiet frame clears the streak.
3. Walks the list from the top and sends the first rule that is over its cluster minimum and off cooldown. Cooldown is stored by `id`.
4. Sends at most one taunt per frame, then clears the streak.

Chat destination (`all` or `team`) is `input.chat` for every taunt. The agent does not know the match clock, whether a match has started, or whether the player is idle. The coach hotkey (F13 or Enter on stdin) advances build-order steps. It does not fire taunts.

## Document shape

A file is a mapping with a `taunts` list. Unknown keys should be rejected once this is wired up, same as `config.yaml`.

```yaml
version: 1
taunts:
  - id: under_attack
    name: Under attack
    number: 63
    # ...
```

`version` is the schema version, not the agent version. This draft is version 1.

## Fields

| Field | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `id` | string | required | | Stable key. Today's cooldown map uses this, not the taunt number. Letters, digits, `_`, `-`. |
| `name` | string | optional | `id` | Short name for logs and docs. Not typed into the game. |
| `number` | integer | required | | Taunt code from `docs/TAUNTS.md`, such as `63`. Must be ≥ 1. |
| `command` | string | optional | `/{number}` | Exact chat text. Default is a slash plus the code. Set this only to override that. |
| `text` | string | optional | | Published spoken line, for the log and for optional speech. The game speaks its own line when the command is sent. |
| `description` | string | optional | | Why the rule exists. Not shown in game. |
| `enabled` | bool | optional | `true` | Skip the rule when false. |
| `chat` | `all`, `team`, or `inherit` | optional | `inherit` | `inherit` uses `input.chat`. `team` is the Shift+Enter guess and is not verified in a live client. |
| `condition` | object | required | | When the rule is eligible. See below. |
| `cooldown_secs` | number | required | | Seconds before this `id` may fire again. Minimum 1, matching today's check. This is the "cooldown" field. |
| `max_per_game` | integer | optional | no limit | Cap on sends after a match start. Proposed: the agent does not yet know match boundaries. This is the "max" field. |
| `priority` | integer | optional | `100` | Lower number is considered first. Equal priority keeps file order, which is what the loop does now. |
| `chance` | number | optional | `1` | Probability in `0..=1` of sending when the rule is otherwise due. `1` means always. Proposed. |
| `min_game_time_secs` | number | optional | `0` | Do not send before this many seconds of match time. Proposed: there is no match clock yet. |
| `consecutive_frames` | integer | optional | `detection.consecutive_frames` | Per-rule streak. Proposed. Today one streak covers every rule. |
| `also_speak` | bool | optional | `false` | Also speak `text` with TTS. Proposed. Off by default so the game's own taunt voice is not doubled. |

## Conditions

`condition.type` picks the parameters. Extra parameters for another type should be rejected.

### `minimap_enemy_near_tc` (implemented)

The only condition the 0.1 loop can evaluate.

| Parameter | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `min_clusters` | integer | optional | `1` | Enemy blobs inside the town-center disk. Today's `min_clusters`. |

Blob size, colors, and whether the disk is used at all stay on `detection` and `colors` in `config.yaml`. They are properties of the camera, not of one line.

### `hotkey` (proposed)

Coach mode already polls one global hotkey and stdin. Taunts do not.

| Parameter | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `key` | string | required | | `F1`–`F24`, or `stdin` for Enter in the agent console. |

### `game_start` (proposed)

No signal today distinguishes the main menu from the first second of a match.

| Parameter | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `delay_secs` | number | optional | `0` | Wait this long after the detected start. |

### `idle` (proposed)

"Nothing enemy-colored near the town center for a while" could be built from the same blob count. A true idle (player not clicking) cannot.

| Parameter | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `quiet_secs` | number | required | | Seconds with `near_clusters == 0`. |

### `coach_event` (proposed)

The coach and the taunt loop share a key lock so they do not type over each other. The taunt loop does not see build-order steps.

| Parameter | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `event` | string | required | | `age_up`, `step`, or `finished`. |

## Mapping the rules that exist now

| Today (`config.yaml`) | Draft |
| --- | --- |
| `id: under_attack` | `id: under_attack` |
| `command: "/63"` | `number: 63` (command defaults to `/63`) |
| `label: "I'm under attack!"` | `text` |
| `cooldown_secs: 45` | `cooldown_secs: 45` |
| `min_clusters: 1` | `condition: { type: minimap_enemy_near_tc, min_clusters: 1 }` |
| `detection.consecutive_frames: 3` | still global until `consecutive_frames` on the rule is implemented |
| commented `/12` with `min_clusters: 2` and `cooldown_secs: 60` | `id: being_rushed`, `number: 12`, same thresholds |

List order is the priority. `under_attack` is first, so a frame with two clusters still sends `/63` while `/63` is off cooldown. `/12` is only reached when `/63` is cooling down. That matches `TauntBrain`.

## Design notes

- One file, `taunts.yaml`, next to `config.yaml`, so the capture setup and the line list can change separately. A later loader would read it only when present and otherwise keep today's `taunts:` block.
- The chat command stays `/` plus the code. `text` is for humans and for TTS. Putting the sentence in `command` would type the words instead of the taunt.
- `chance` defaults to 1 so a file does not randomly drop the under-attack call. A value below 1 needs a seeded draw if tests are going to pin it.
- `max_per_game` and `min_game_time_secs` are written down now so the file format does not have to break when match detection exists. Until then a loader should ignore them or refuse the file, and the draft does not pick which.
- `priority` is an integer so two authors can insert a rule without reordering the whole list. File order remains the tie break.
- Team chat is per rule because "retreat" is for allies and "GG" is for everyone. The Shift chord is still the unverified one described in `plan.md`.

## Open questions

1. Should a missing `taunts.yaml` mean "use `config.yaml` as now", or should the loader require the new file?
2. Is cooldown keyed by `id` or by `number`? Two rules can share `/63` with different `id`s today and cool down separately. Keying by number would stop that.
3. The streak is one counter for every rule. If `/63` wants 3 frames and `/12` wants 8, whose counter runs when both cluster minimums are met?
4. On a frame where several rules match, is it lowest `priority` only (today's behavior), or may a second rule fire on the next frame without a quiet gap? Today the streak resets after a send, so the next rule waits out `consecutive_frames` again.
5. `max_per_game` needs a start and an end. Process launch is not a match start, and quitting to the menu does not exit the agent. What event resets the counter?
6. `min_game_time_secs` and `game_start` need a clock. Is that the agent's own timer from "match detected", the coach's `--start-at` clock, or an in-game read that does not exist yet?
7. `idle` as "no enemy blobs" will fire all game if the opponent never walks into the disk, including while the player is busy. Is that the intended quiet signal?
8. `coach_event` can fire from a timer that has drifted. Should those taunts be limited to hotkey coach mode?
9. `chance` below 1 makes the under-attack call unreliable. Is it only for flavor lines (`/11`, `/39`), with detection lines fixed at 1?
10. `also_speak` plays the line twice if the game already voices the taunt. Should TTS be limited to `dry_run`?
11. Locked taunts (`/106`–`/122`, and anything still unpublished) fail in chat if the account does not own them. Should the schema carry `unlocked: true` as a reminder, or is that only a note in `docs/TAUNTS.md`?
12. Editors can validate `schemas/taunts.schema.json`. Do we want CI to check `taunts.example.yaml` against it before the Rust loader exists?
