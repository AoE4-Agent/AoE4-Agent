# AoE4-Agent
Age Of Empires 4 Agent

## What it does

1. Taunt module:
  - Background process that watches RelicCardinal.exe, captures the minimap via screen grab, uses pure-Rust HSV color masking + connected components to detect enemy unit clusters near the Town Center, and auto-sends taunts (e.g. /63 “I’m under attack!”) based on a YAML config with cooldowns.
2. Coach module:
  - Pulls build orders from the aoe4guides.com API and announces steps via TTS or in-game chat on a timer.
