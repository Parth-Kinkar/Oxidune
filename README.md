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
- Up next view: see what plays next and edit it (play next, reorder, remove)
- Pause, resume, seek, next, previous
- Mouse support: click, double-click, scroll, and drag to seek (see [Mouse](#mouse))
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
| `--no-mouse` | Leave the mouse to the terminal (normal text selection)  |

## Keys

| Key              | Action                      |
|------------------|-----------------------------|
| `↑` / `k`        | Move selection up           |
| `↓` / `j`        | Move selection down         |
| `Enter`          | Play selected track         |
| `a`              | Play selected track next    |
| `/`              | Search: type to filter list |
| `Esc`            | Clear search                |
| `Space`          | Pause / resume              |
| `n` / `p`        | Next / previous track       |
| `←` / `→`        | Seek back / forward 5s      |
| `r`              | Cycle repeat mode           |
| `z`              | Toggle shuffle              |
| `l`              | Show / hide lyrics tab      |
| `u`              | Show / hide Up next tab     |
| `Tab`            | Switch focus: library / Up next |
| `s`              | Stop                        |
| `q`              | Quit                        |

## Up next

The right-hand panel has two tabs, **Lyrics** and **Up next**. Up next lists the songs that will play after the current one, in the real order, so with shuffle on you can see exactly what's coming. Songs added with `a` are marked `+`.

Press `Tab` (or click the list) to focus Up next, then:

| Key            | Action                                  |
|----------------|-----------------------------------------|
| `↑` / `↓`, `j` / `k` | Select a song                     |
| `Enter`        | Play it now (songs before it are skipped) |
| `J` / `K`      | Move it down / up                       |
| `d` / `Delete` | Remove it from the queue                |

Removed songs stay out of the queue, also when you toggle shuffle; picking one from the library brings it back. Songs added with `a` stay at the top, in the order you added them, when the order is shuffled; a dim `shuffled ↓` line shows where the shuffled part starts.

## Mouse

| Where                 | Action                                  |
|-----------------------|-----------------------------------------|
| Library list          | Click to select, double-click to play   |
| Scroll wheel on list  | Move through the list                   |
| Progress bar          | Click or drag to seek                   |
| Synced lyrics line    | Click to jump to that line              |
| Now Playing line      | Click to pause / resume                 |
| Search box            | Click to edit the search                |
| Lyrics / Up next tabs | Click to switch                         |
| Up next list          | Click to select, double-click to play now |

While TuneTerm uses the mouse, most terminals still let you select text by holding **Shift** while dragging. Run with `--no-mouse` to turn mouse support off.

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
