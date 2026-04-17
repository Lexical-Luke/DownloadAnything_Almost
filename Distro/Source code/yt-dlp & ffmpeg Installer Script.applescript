-- yt-dlp & ffmpeg Installer
-- Updated for yt-dlp (youtube-dl is deprecated)

set dialog to display dialog "yt-dlp & ffmpeg Installer

This will install yt-dlp and ffmpeg using Homebrew.
Make sure Homebrew is installed first!" buttons {"Install", "Exit"} default button "Exit"
set returnButton to button returned of dialog

-- Install yt-dlp and ffmpeg
if returnButton is equal to "Install" then
	tell application "Terminal"
		activate
		set currentTab to do script ("brew install yt-dlp && brew install ffmpeg && echo '✅ Installation complete!'")
	end tell
	
-- Quit the Script
else if returnButton is equal to "Exit" then
	return
end if
