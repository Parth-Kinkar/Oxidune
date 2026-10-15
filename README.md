# TuneTerm

A terminal music player for lossless audio, written in Rust.

TuneTerm plays the files you already own: FLAC, ALAC (`.m4a`), WAV, MP3, and Ogg. It is built around clean, readable code and a small set of well-separated parts, not a pile of features.

> **Status:** early development. Playback, queue, and the basic TUI work. See [upcoming_features.md](upcoming_features.md) for what is planned.

## Features

- Plays FLAC, ALAC/AAC (`.m4a`), WAV, MP3, and Ogg Vorbis
- Terminal UI built with [ratatui](https://ratatui.rs)
- Library scanning with tags read via [lofty](https://crates.io/crates/lofty)
- Queue with repeat modes (off / all / one)
- Pause, resume, seek, next, previous
- Playback runs on its own thread, so the UI never blocks audio

## Requirements

- Rust (edition 2024, so a recent stable toolchain)
- ALSA development files
  - Arch: `sudo pacman -S alsa-lib`
  - Debian/Ubuntu: `sudo apt install libasound2-dev`

## Build and run

```bash
git clone <your-repo-url>
cd TuneTerm
cargo run --release -- /path/to/your/music
```

If you don't pass a directory, TuneTerm scans the current one.

## Keys

| Key              | Action                      |
|------------------|-----------------------------|
| `↑` / `k`        | Move selection up           |
| `↓` / `j`        | Move selection down         |
| `Enter`          | Play selected track         |
| `Space`          | Pause / resume              |
| `n` / `p`        | Next / previous track       |
| `←` / `→`        | Seek back / forward 5s      |
| `r`              | Cycle repeat mode           |
| `s`              | Stop                        |
| `q`              | Quit                        |

## Project layout

```
src/
├── main.rs          # wires everything together
├── app.rs           # App: state and input handling
├── error.rs         # one Error type for the whole app
├── domain/          # plain data: Track, Queue
├── library/         # Scanner, TagReader, Library
├── audio/           # Decoder, AudioOutput, Player (playback thread)
└── ui/              # ratatui views, no audio logic
```

Design rules:

- Each struct owns one job.
- Traits sit only at the seams where an implementation might be swapped (`Decoder`, `AudioOutput`).
- The UI talks to the audio engine through `Command`s and `Event`s over channels, and never touches audio directly.

## Tests

```bash
cargo test
```

## Roadmap

See [Upcoming_features.md](Upcoming_features.md).

## License

Choose a license before publishing, for example `MIT OR Apache-2.0`, and add a `LICENSE` file.
