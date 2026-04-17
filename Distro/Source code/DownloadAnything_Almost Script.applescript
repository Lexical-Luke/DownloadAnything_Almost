-- DownloadAnything_Almost v2.0
-- Updated for yt-dlp and Apple Silicon Macs
-- Downloads video as MP4 (h264/aac) or audio as MP3 - native Mac formats

set dialog to display dialog "Enter a URL to download" default answer "" buttons {"Audio (MP3)", "Video (MP4)", "Exit"} default button "Exit"
set returnText to text returned of dialog
set returnButton to button returned of dialog

-- Download Video (MP4 with h264/aac for Mac compatibility)
if returnButton is equal to "Video (MP4)" then
	tell application "Terminal"
		activate
		set currentTab to do script ("yt-dlp --merge-output-format mp4 --postprocessor-args \"ffmpeg:-vcodec h264 -acodec aac\" --add-metadata -o \"~/Downloads/%(title)s.%(ext)s\" \"" & returnText & "\" && open -R ~/Downloads")
	end tell
	
-- Download Audio (MP3 for universal compatibility)
else if returnButton is equal to "Audio (MP3)" then
	tell application "Terminal"
		activate
		set currentTab to do script ("yt-dlp -x --audio-format mp3 --audio-quality 0 --add-metadata -o \"~/Downloads/%(title)s.%(ext)s\" \"" & returnText & "\" && open -R ~/Downloads")
	end tell
	
-- Quit the Script
else if returnButton is equal to "Exit" then
	return
end if

-- Supported sites: https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md
