# https://ubports.com/de/blog/ubports-blogs-nachrichten-1/post/introduction-to-clickable-147

test: 
	export PATH=$PATH:~/.local/bin
	clickable update || clickable desktop

phone: 
	export PATH=$PATH:~/.local/bin
	clickable build --arch arm64
	sudo adb start-server
	sudo adb devices
	clickable install --arch arm64
	adb kill-server

init: 
	export PATH=$PATH:~/.local/bin
	clickable create

