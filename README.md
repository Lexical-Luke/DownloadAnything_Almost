# DownloadAnything_Almost

**v2.0 - Updated for Apple Silicon & yt-dlp**

Designed for macOS, **DownloadAnything_Almost** is an AppleScript app to easily download and convert audio & video files from many different websites using a GUI that implements the [yt-dlp](https://github.com/yt-dlp/yt-dlp) command-line tool.

**_If you have the link, you can probably download it._**

Downloads are saved to `~/Downloads` in Mac-native formats:

- **Video**: MP4 (H.264 video + AAC audio)
- **Audio**: MP3

This application is capable of downloading entire playlists from Soundcloud, YouTube, and more with corresponding metadata.

**Supported Sites:** https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md

## Requirements

- [Homebrew](https://brew.sh/)
- [yt-dlp](https://github.com/yt-dlp/yt-dlp)
- [ffmpeg](https://ffmpeg.org/)

## Installation

### Manual Installation

Install Homebrew (if not already installed):

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

Then install yt-dlp and ffmpeg:

```bash
brew install yt-dlp
brew install ffmpeg
```

### Using the Installer Apps

For easy _double-click_ installation:

1. Run **"1. Brew Installer"** to install Homebrew
2. Run **"2. yt-dlp & ffmpeg Installer"** to install yt-dlp and ffmpeg
3. Use **"DownloadAnything_Almost"** to download media anytime

For convenience, drag the DownloadAnything_Almost app to your Applications folder.

## Usage

1. Launch DownloadAnything_Almost
2. Paste a URL from any supported site
3. Choose **Video (MP4)** or **Audio (MP3)**
4. The download will start in Terminal and open Finder when complete

## Troubleshooting

If the app refuses to run, you may need to allow it in:
**System Settings > Privacy & Security > Security > Open Anyway**

## Updating yt-dlp

Keep yt-dlp updated for the latest site support:

```bash
brew upgrade yt-dlp
```

## Rebuilding the Apps

If you modify the AppleScript source files, you can rebuild the apps.

### Using the Build App

Double-click **"3. Build Apps"** to compile the DownloadAnything_Almost app from source.

### Manual Rebuild (Terminal)

```bash
cd "/path/to/Youtube-dl_GUI/Distro"
osacompile -o "DownloadAnything_Almost.app" "Source code/DownloadAnything_Almost Script.applescript"
osacompile -o "1. Brew Installer.app" "Source code/Brew Installer Script.applescript"
osacompile -o "2. yt-dlp & ffmpeg Installer.app" "Source code/yt-dlp & ffmpeg Installer Script.applescript"
```

### Source Files

All source code is in `Distro/Source code/`:

| Script                                         | Purpose                   |
| ---------------------------------------------- | ------------------------- |
| `DownloadAnything_Almost Script.applescript`   | Main downloader app       |
| `Brew Installer Script.applescript`            | Homebrew installer        |
| `yt-dlp & ffmpeg Installer Script.applescript` | yt-dlp + ffmpeg installer |
| `Build Apps Script.applescript`                | Compiles the main app     |
