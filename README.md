# pmu

![](media/icon.png)

**P**lay **mu**sic in your terminal!

## Features

### Play past songs from any directory

After you play a song, run the same command to play the same song even if you're in a different directory! Playback
history is stored in an SQLite database.

### Extract metadata from supported song folders

Metadata is automatically extracted from special song folders. Supported folders include those from:

- osu!
- Stepmania

### Discord Rich Presence

Show off the song you're playing in Discord!

![](media/presence.png)

## Installation

```
$ cargo install --locked --git https://github.com/Techno-coder/pmu
```

## Usage

### Play a song

```
$ pmu play path/to/song.mp3
```

### Print help

```
$ pmu help
```

### Print configuration directory

```
$ pmu config
```

## Configuration

The configuration file is named `config.json`. The documentation for each option can be found [here](src/config.rs).
