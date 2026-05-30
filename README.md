# clonehero-maker — Cheatsheet

Wirf eine Audiodatei rein, bekomme einen fertigen Clone-Hero-Songordner raus.

## Voraussetzung

`ffmpeg` muss installiert sein (wird zum Konvertieren nach OGG aufgerufen):

```bash
sudo apt install ffmpeg
```

## Bauen

```bash
cd chtm
cargo build --release
# Binary liegt dann unter: target/release/clonehero-maker
```

## Benutzung

```bash
clonehero-maker <audiodatei> [OPTIONEN]
```

| Beispiel | Was passiert |
|---|---|
| `clonehero-maker song.mp3` | Ordner unter `Output/<Songname>/` |
| `clonehero-maker "Metallica - One.mp3"` | Name aus ID3-Titel, sonst Dateiname |
| `clonehero-maker song.flac -o ~/CloneHero/Songs` | Ausgabe in eigenen Ordner |
| `clonehero-maker song.mp3 -d hard` | Chart bis Schwierigkeit „Hard" |
| `clonehero-maker song.mp3 -f` | Bestehenden Ordner überschreiben |

## Optionen

| Flag | Kurz | Default | Bedeutung |
|---|---|---|---|
| `--output <DIR>` | `-o` | `Output` | Zielverzeichnis (Songordner wird darin angelegt) |
| `--difficulty <STUFE>` | `-d` | `expert` | Höchste Schwierigkeit: `easy` / `medium` / `hard` / `expert` |
| `--force` | `-f` | aus | Vorhandenen Songordner überschreiben |
| `--help` | `-h` | | Hilfe anzeigen |
| `--version` | `-V` | | Version anzeigen |

> `--difficulty` chartet **alle Stufen bis einschließlich** der gewählten,
> damit der Song im Spiel immer spielbar ist (`expert` = alle vier).

## Eingabeformate

`mp3` · `wav` · `flac` · `ogg`

## Was erzeugt wird

```text
Output/
└── <Songname>/
    ├── song.ogg     ← konvertiertes Audio
    ├── guitar.ogg   ← Kopie von song.ogg
    ├── notes.mid    ← Gitarrennoten (folgen den erkannten Beats)
    ├── song.ini     ← Metadaten (aus ID3-Tags, sonst Dateiname)
    └── album.png    ← eingebettetes Cover oder generierter Platzhalter
```

## In Clone Hero nutzen

Den erzeugten Songordner einfach in deinen Clone-Hero-Songs-Ordner kopieren,
z.B.:

```bash
cp -r "Output/<Songname>" ~/.config/clonehero/songs/
```

(Pfad je nach Installation; im Spiel ggf. **Scan Songs** ausführen.)

## Hinweis

Die Noten werden automatisch aus erkannten Onsets/Tempo generiert — sie folgen
der Musik, sind aber kein handgechartetes Profi-Chart. Ziel ist: jede Datei
lädt fehlerfrei und ist sofort spielbar.
