-- Homebrew Installer
-- Updated install command for modern macOS and Apple Silicon

set dialog to display dialog "Homebrew Installer

This will install Homebrew, the macOS package manager.
You may need to enter your password during installation." buttons {"Install", "Exit"} default button "Exit"
set returnButton to button returned of dialog

-- Install Homebrew
if returnButton is equal to "Install" then
	tell application "Terminal"
		activate
		set currentTab to do script ("/bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"")
	end tell
	
-- Quit the Script
else if returnButton is equal to "Exit" then
	return
end if
