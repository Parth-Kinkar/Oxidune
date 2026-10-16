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

## Next up

- [ ] **Search / filter**: press `/` and type to narrow the list
- [ ] **Shuffle**
- [ ] **Bit-perfect output**: open an ALSA `hw:` device directly, with device selection

## Lyrics follow-ups

- [ ] Tell "offline / network error" apart from "not in LRCLIB" in the lyrics panel
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
- [ ] License file

## Deliberately skipped

Things that would pull the project away from being small and readable:

- Equalizer (conflicts with bit-perfect output)
- Streaming sources
- Plugin system
