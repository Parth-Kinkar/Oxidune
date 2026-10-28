# Upcoming Features

Planned work, in the order I intend to build it. Tick the boxes as things land.

## Done

- [x] Library scanning and tag reading
- [x] Decoding (Symphonia) and output (cpal)
- [x] Playback engine on its own thread (play, pause, resume, seek, stop)
- [x] Queue with repeat modes
- [x] Basic TUI: library list, now playing, progress bar
- [x] **Lyrics**: `.lrc` parser, line highlighting, local `.lrc` first, then fetched from [LRCLIB](https://lrclib.net) and cached next to the song, `--offline` flag
- [x] **Gapless playback**: preload the next track so albums flow without a gap
- [x] **Search / filter**: press `/` and type to narrow the list
- [x] **Mouse support**: click to select, double-click to play, scroll the list, click or drag the progress bar to seek, click a lyric line to jump to it, `--no-mouse` to turn it off
- [x] **Lyrics lookup fixes**: retry when LRCLIB is busy, looser searches for titles with "(feat. …)" / "(From …)" and multi-artist tags, and the lyrics panel tells "offline", "network error", and "not in LRCLIB" apart
- [x] **Shuffle**: `z` toggles a random order; every track plays once before it ends

## Next up

- [ ] **Up next / queue view**: see which songs play next (especially with shuffle on), and edit the queue: reorder, remove, or pick what plays next
- [ ] **Collapsible panels**: some sections (e.g. lyrics, queue, output devices) only show up when clicked or selected
- [ ] **Output devices and bit-perfect mode** (needs panels + mouse first): a device picker panel listing each output with what it supports (e.g. "up to 384 kHz / 32-bit"; Bluetooth shows its codec and is shared-only), and a choice of:
  - *Shared*: through PipeWire, other apps can play too; PipeWire may resample and apply system volume
  - *Exclusive (bit-perfect)*: open the ALSA `hw:` device directly at the song's exact rate and bit depth; no other audio, no software volume

## Lyrics follow-ups

- [ ] Scrolling for plain (unsynced) lyrics
- [ ] Config option for online lookups, replacing the `--offline` flag

## Audio quality

- [ ] Optional volume control (hardware volume or no gain in exclusive mode)
- [ ] ReplayGain support

## Library and polish

- [ ] Browse by album / artist
- [ ] Cover art (embedded first, then `cover.jpg` in the folder), shown in the terminal
- [ ] Config file (TOML): music directory, output device, keybinds
- [ ] Library cache so launches stay fast with large libraries
- [ ] Remember last queue and position

## Integration and release

- [ ] MPRIS support (media keys, `playerctl`, desktop widgets)
- [ ] More tests
- [ ] CI
- [ ] AUR package
- [x] License file

## Deliberately skipped

Things that would pull the project away from being small and readable:

- Equalizer (conflicts with bit-perfect output)
- Streaming sources
- Plugin system
