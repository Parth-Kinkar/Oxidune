# TuneTerm

A terminal music player for lossless audio, written in Rust.

TuneTerm plays the files you already own: FLAC, ALAC (`.m4a`), WAV, MP3, and Ogg. It is built around clean, readable code and a small set of well-separated parts, not a pile of features.

> **Status:** early development. Playback, queue, lyrics, and the basic TUI work. See [Upcoming_features.md](Upcoming_features.md) for what is planned.

## Features

- Plays FLAC, ALAC/AAC (`.m4a`), WAV, MP3, and Ogg Vorbis
- Terminal UI built with [ratatui](https://ratatui.rs)
- Library scanning with tags read via [lofty](https://crates.io/crates/lofty)
- Search: press `/` and type to filter by artist, title, or album
- Queue with repeat modes (off / all / one) and shuffle
- Pause, resume, seek, next, previous
- Synced lyrics, highlighted line by line as the song plays (see [Lyrics](#lyrics))
- Playback runs on its own thread, so the UI never blocks audio

## Requirements

- Rust (edition 2024, so a recent stable toolchain)
- ALSA development files
  - Arch: `sudo pacman -S alsa-lib`
  - Debian/Ubuntu: `sudo apt install libasound2-dev`

## Build and run

```bash
git clone https://github.com/Parth-Kinkar/Oxidune.git
cd Oxidune
cargo run --release -- /path/to/your/music
```

If you don't pass a directory, TuneTerm scans the current one.

| Flag        | Effect                                                    |
|-------------|-----------------------------------------------------------|
| `--offline` | Never contact the network (local `.lrc` files still work) |

## Keys

| Key              | Action                      |
|------------------|-----------------------------|
| `↑` / `k`        | Move selection up           |
| `↓` / `j`        | Move selection down         |
| `Enter`          | Play selected track         |
| `/`              | Search: type to filter list |
| `Esc`            | Clear search                |
| `Space`          | Pause / resume              |
| `n` / `p`        | Next / previous track       |
| `←` / `→`        | Seek back / forward 5s      |
| `r`              | Cycle repeat mode           |
| `z`              | Toggle shuffle              |
| `l`              | Show / hide lyrics panel    |
| `s`              | Stop                        |
| `q`              | Quit                        |

## Lyrics

When a track starts, TuneTerm looks for lyrics in this order:

1. A `.lrc` file next to the audio file with the same name (`song.m4a` → `song.lrc`).
2. If none exists, [LRCLIB](https://lrclib.net), a free and open lyrics database. TuneTerm first tries an exact match on artist, title, album, and duration, then falls back to a looser search by artist and title, preferring synced lyrics and a close duration.
3. Whatever LRCLIB returns is saved as a `.lrc` file next to the song, so each song is looked up at most once.

Lookups run on a background thread, so the UI never waits on the network. Synced lyrics (with timestamps) are highlighted as the song plays; plain lyrics are shown without highlighting.

**Privacy:** online lookups send the track's artist, title, album, and duration to `lrclib.net`. Nothing else is sent. Use `--offline` to disable this entirely.

Lyrics data is provided by [LRCLIB](https://lrclib.net) and its contributors.

## Project layout

```
src/
├── main.rs          # wires everything together
├── app.rs           # App: state and input handling
├── error.rs         # one Error type for the whole app
├── domain/          # plain data: Track, Queue, Lyrics (.lrc parser)
├── library/         # Scanner, TagReader, Library
├── audio/           # Decoder, AudioOutput, Player (playback thread)
├── lyrics/          # LyricsStore (.lrc files), LrclibClient, LyricsService
└── ui/              # ratatui views, no audio logic
```

Design rules:

- Each struct owns one job.
- Traits sit only at the seams where an implementation might be swapped (`Decoder`, `AudioOutput`).
- The UI talks to the audio engine through `Command`s and `Event`s over channels, and never touches audio directly.
- Only `lyrics/lrclib.rs` knows about the network.

## Tests

```bash
cargo test
```

## Roadmap

See [Upcoming_features.md](Upcoming_features.md).

## License

Licensed under the [Apache License 2.0](LICENSE).
