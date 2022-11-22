# https://ubports.com/de/blog/ubports-blogs-nachrichten-1/post/introduction-to-clickable-147
# https://docs.ubports.com/en/latest/appdev/platform/apparmor.html
# In desktop mode /home/phablet is mounted to ~/.clickable/home/. You can manipulate and add data there.

test: 
	export all_proxy=""
	export ALL_PROXY=""
	export PATH=$PATH:~/.local/bin
	clickable update || clickable desktop

phone: 
	export all_proxy=""
	export ALL_PROXY=""
	export PATH=$PATH:~/.local/bin
	clickable build --arch arm64
	sudo adb start-server
	sudo adb devices
	clickable install --arch arm64
	adb kill-server

logs:
	clickable logs --arch arm64

init: 
	export PATH=$PATH:~/.local/bin
	clickable create

