# clonehero-maker - CHTM // CLONE HERO TRACK MAKER

Drop in an audio file, get a fully playable Clone Hero song folder out.

## Requirements

`ffmpeg` must be installed (used to convert audio files to OGG format):

```bash
sudo apt install ffmpeg
```

## Building

```bash
cd chtm
cargo build --release
# The binary will be located at:
# target/release/clonehero-maker
```

## Usage

```bash
clonehero-maker <audio-file> [OPTIONS]
```

| Example                                          | What happens                                            |
| ------------------------------------------------ | ------------------------------------------------------- |
| `clonehero-maker song.mp3`                       | Creates a folder under `Output/<SongName>/`             |
| `clonehero-maker "Metallica - One.mp3"`          | Uses the ID3 title if available, otherwise the filename |
| `clonehero-maker song.flac -o ~/CloneHero/Songs` | Outputs to a custom directory                           |
| `clonehero-maker song.mp3 -d hard`               | Generates charts up to "Hard" difficulty                |
| `clonehero-maker song.mp3 -f`                    | Overwrites an existing song folder                      |

## Options

| Flag                   | Short | Default  | Description                                                           |
| ---------------------- | ----- | -------- | --------------------------------------------------------------------- |
| `--output <DIR>`       | `-o`  | `Output` | Target directory (the song folder will be created inside it)          |
| `--difficulty <LEVEL>` | `-d`  | `expert` | Highest difficulty to generate: `easy` / `medium` / `hard` / `expert` |
| `--force`              | `-f`  | disabled | Overwrite an existing song folder                                     |
| `--help`               | `-h`  |          | Show help                                                             |
| `--version`            | `-V`  |          | Show version information                                              |

> `--difficulty` generates **all difficulty levels up to and including**
> the selected level, ensuring the song is always playable in-game
> (`expert` = all four difficulty levels).

## Supported Input Formats

`mp3` · `wav` · `flac` · `ogg`

## Generated Output

```text
Output/
└── <SongName>/
    ├── song.ogg     ← converted audio
    ├── guitar.ogg   ← copy of song.ogg
    ├── notes.mid    ← guitar note chart (based on detected beats)
    ├── song.ini     ← metadata (from ID3 tags or filename)
    └── album.png    ← embedded cover art or generated placeholder
```

## Using in Clone Hero

Simply copy the generated song folder into your Clone Hero songs directory:

```bash
cp -r "Output/<SongName>" ~/.config/clonehero/songs/
```

(The path may vary depending on your installation. You may need to run
**Scan Songs** inside the game afterward.)

## Note

The note chart is automatically generated from detected onsets and tempo
information. The notes follow the music rhythmically, but this is not a
professionally hand-charted track. The goal is to ensure every supported
audio file can be converted successfully and is immediately playable in
Clone Hero.
