# Spotify CLI 🎧 (Rust Edition)

A blazingly fast, standalone command-line interface to control Spotify playback on any device, built with Rust and OAuth2 PKCE.

---

## Features

- **Zero Cloud Proxies**: Completely self-contained OAuth 2.0 PKCE authentication with a local TCP loopback listener (`127.0.0.1:8888`).
- **Complete Playback Control**: Play, pause, toggle, next, previous, seek (`mm:ss`, seconds, `+10`, `-15`).
- **Volume**: Relative and absolute volume control (`vol up 10`, `vol down 5`, `vol to 80`, `vol 65`).
- **Playback Status**: Live rendering of current song, artist, album, device, volume, and an ASCII progress bar:
  ```text
  ▶ Playing
    Track:  Bohemian Rhapsody
    Artist: Queen
    Album:  A Night At The Opera
    Time:   [━━━━━━━━━━━━━────────────] 02:30 / 05:55
    Device: My PC (Computer) | Vol: 75% | 🔀 off | 🔁 all
  ```
- **Device Management**: List devices and switch active player interactively.
- **Interactive Search**: Paginated results table with interactive actions:
  - `Play track`
  - `Queue track`
  - `Save track to Liked Songs`
- **Queue & Library**: View upcoming songs, queue tracks/albums, queue current track with `spotify-cli queue .`, view recently played history, and check top artists/tracks.
- **Cross-Platform**: Config and credentials stored securely in standard OS directories (`%APPDATA%/spotify-cli` on Windows, `~/.config/spotify-cli` on Linux/macOS).

---

## Quickstart (Windows)

### Option A: 1-Click Automated Setup (Recommended)
Run `build.bat` directly or from your terminal:
```cmd
build.bat
```
This script will:
1. Compile the project in release mode (`cargo build --release`).
2. Create both `spotify-cli.exe` and `spotify.cmd` commands in the output folder.
3. Automatically check if the folder is in your Windows User `PATH` and offer to add it for you.

---

### Option B: Manual Cargo Build
```bash
cargo build --release
```
The compiled executable will be located at:
```text
target/release/spotify-cli.exe
```

To use `spotify-cli` from any directory, add `<path-to-spotify-cli>\target\release` to your `PATH`, or copy `spotify-cli.exe` to any folder already in your `PATH` (such as `C:\Windows\System32` or your personal CLI bin directory).

---

## Setup & Authentication

Spotify requires a free Developer App Client ID for API access:

1. Go to the [Spotify Developer Dashboard](https://developer.spotify.com/dashboard) and click **Create App**.
2. Fill in:
   - **App Name**: `Spotify CLI`
   - **Redirect URI**: `http://127.0.0.1:8888/callback`
   - Check **Web API** under APIs used.
3. Copy your **Client ID**.
4. Run:
   ```bash
   spotify-cli auth setup
   ```
   (Paste your Client ID when prompted).
5. Log in:
   ```bash
   spotify-cli auth login
   ```
   Your default browser will open automatically. Click **Agree**, and the local loopback server will silently capture your tokens.

---

## Commands & Usage

> Note: Both `spotify-cli` and `spotify` (via the included wrapper) can be used interchangeably.
> **Dual Mode**: All commands support both **Direct CLI arguments** and **Interactive UI selection** when arguments are omitted!

### Command Menu & Cheatsheet
```bash
# Interactive menu (browse by category or view all)
spotify-cli menu                # alias: spotify-cli m, spotify-cli commands

# Direct CLI view of a specific category
spotify-cli menu playback
spotify-cli menu volume
spotify-cli menu all
```

### Status & Real-Time Player
```bash
# Interactive real-time player (updates progress locally, hotkeys for seek/vol/skip/settings)
spotify-cli status              # or just 'spotify-cli'
# Player Hotkeys:
#   ← / →     Seek back / forward (customizable step, default 10s)
#   ↑ / ↓     Volume up / down (customizable step)
#   Space / t Play / Pause toggle
#   n / p     Next / Previous track
#   f         Toggle Shuffle
#   r         Cycle Repeat mode
#   s         Open Settings menu (change seek/volume step)
#   Esc / q   Exit player

# One-shot static status (non-interactive)
spotify-cli status --static     # alias: -s, --once
```

# Resume playback
spotify-cli play

# Search and play a song
spotify-cli play bohemian rhapsody

# Play an album or playlist
spotify-cli play --album "abbey road"
spotify-cli play --playlist "lofi beats"

# Pause or toggle playback
spotify-cli pause
spotify-cli toggle              # alias: spotify-cli t

# Skip tracks
spotify-cli next                # alias: spotify-cli n, spotify-cli skip
spotify-cli previous            # alias: spotify-cli prev, spotify-cli b

# Seek to timestamp
spotify-cli seek 1:30
spotify-cli seek +15            # skip 15 seconds forward
spotify-cli seek -10            # skip 10 seconds back
```

### Volume & Modes
```bash
# Interactive volume selector (arrow keys + hit Enter to apply)
spotify-cli vol

# Direct volume control
spotify-cli vol 80              # Set volume directly to 80%
spotify-cli vol up 10           # Increase volume by 10%
spotify-cli vol down 5          # Decrease volume by 5%
spotify-cli vol to 50           # Set volume to 50%

# Shuffle & Repeat
spotify-cli shuffle on          # or off, or toggle without args
spotify-cli repeat track        # or all, off, or cycle without args
```

### Devices
```bash
# List available devices and interactively prompt to switch
spotify-cli devices             # alias: spotify-cli dev

# Direct switch to a device by name
spotify-cli devices --switch "PC"

# Pick interactively from a dedicated menu
spotify-cli devices -i
```

### Queue
```bash
# View upcoming tracks in queue and interactively select one to play directly
spotify-cli queue

# Add a track or album to queue
spotify-cli queue "hotel california"
spotify-cli queue --album "thriller"

# Queue the song that is currently playing
spotify-cli queue .
```

### Interactive Search
```bash
# Search tracks (interactive table with Play / Queue / Save actions)
spotify-cli search "daft punk"

# Search albums, artists, or playlists
spotify-cli search --album "discovery"
spotify-cli search --artist "radiohead"
spotify-cli search --playlist "workout"
```

### Library & Stats
```bash
# Save currently playing track to Liked Songs
spotify-cli save
spotify-cli save --album

# View recently played history
spotify-cli history
spotify-cli history -l 25

# View your top tracks or artists
spotify-cli top
spotify-cli top --artists
```

### Diagnostics & Automated Testing
```bash
# Interactive diagnostic test suite selector
spotify-cli test                # alias: spotify-cli tst, spotify-cli check

# Direct CLI execution for specific test suite:
spotify-cli test all            # Runs all tests in sequence
spotify-cli test playback       # Tests state query, pause & resume
spotify-cli test volume         # Tests volume change & restoration
spotify-cli test devices        # Tests device query
spotify-cli test search         # Tests search & queue queries
spotify-cli test library        # Tests history & top stats

# All results and timestamps are logged to the logs/ directory!
```
