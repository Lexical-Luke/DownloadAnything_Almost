# DownloadAnything_Almost




Designed for macOS, **DownloadAnything_Almost** is an Apple Script, built as an App to easily download and convert audio & video files from many different websites using a GUI that implements the [youtube-dl](https://youtube-dl.org/ ) command-line tool. 

***If you have the link, you can probably download it.***

This application is capable of downloading entire playlists from Soundcloud with corresponding metadata.

Supported Sites: http://ytdl-org.github.io/youtube-dl/supportedsites.html

**Requirements:**
- [Brew](https://brew.sh/)
- [youtube-dl](https://youtube-dl.org/)
- [ffmpeg](https://ffmpeg.org/)

The most simple way to install the requirements would be to download and install [*Brew*](https://brew.sh/) with  `$/usr/bin/ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)"` in *Terminal* and then use [*Brew*](https://brew.sh/) to install youtube-dl & ffmpeg this can be done simply with the use of:
```bash
$brew install youtube-dl
$brew install ffmpeg
```
	
In order to streamline this process I have made 2 installer applications for the less tehnically inclined folk and the lazier ones around here, such as myself, for *double-click* installation.

Installation is simple, first run "1. Brew Installer" then run "2. Youtube-dl & ffmpeg Installer" after that you DownloadAnything_Almost at any time using the "DownloadAnything_Almost" application. For ease of use you may want to drag and drop the DownloadAnything_Almost application in the Application folder.


If the app refuses to run initially (it probably will) you may need to check: System Preferences > Security & Privacy > General > Open Anyway
