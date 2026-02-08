# Terminal Pac‑Man (Rust)

A fast, full‑screen, terminal‑only Pac‑Man clone written in Rust. It renders with UTF‑8/emoji, runs at high FPS, and uses randomized maze generation with guaranteed connectivity.

## Features

- Terminal rendering with UTF‑8 + emoji + colors
- Smooth 120 FPS rendering (configurable)
- Randomized, fully connected maze with loops
- Classic ghost pen with a gate and staggered releases
- Bonus treats that spawn occasionally
- Vim‑style movement (`h`, `j`, `k`, `l`)

## Requirements

- Rust 1.70+
- A terminal that supports UTF‑8 and ANSI colors

> Tip (Windows): use Windows Terminal or PowerShell 7+. If emoji widths look off, switch to a monospace font with emoji support (e.g., Cascadia Code PL).

## Run

```bash
cargo run --bin pacman
```

## Controls

- Move: `h` `j` `k` `l`
- Quit: `q`

## Gameplay Tuning

You can tune speed with environment variables:

```bash
PACMAN_TICK_MS=70 PACMAN_FPS=120 cargo run --bin pacman
```

- `PACMAN_TICK_MS`: movement tick (lower = faster)
- `PACMAN_FPS`: render rate
- `PACMAN_FULLSCREEN`: set to `0` to disable alternate‑screen fullscreen
- `PACMAN_FULL_MAZE`: set to `1` to scale the maze to your terminal size (regenerates on resize)

Additional gameplay constants are in `src/main.rs`:

- `GHOST_MOVE_INTERVAL` (ghost speed)
- `PEN_W`, `PEN_H` (ghost pen size)
- `GHOST_RELEASE_INTERVAL`
- `BONUS_*` (bonus treat behavior)

## Notes

- The maze is always fully connected (excluding the pen walls/gate).
- The pen gate is passable by ghosts after their release, but not by Pac‑Man.
