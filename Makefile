# https://ubports.com/de/blog/ubports-blogs-nachrichten-1/post/introduction-to-clickable-147
# https://docs.ubports.com/en/latest/appdev/platform/apparmor.html
# In desktop mode /home/phablet is mounted to ~/.clickable/home/. You can manipulate and add data there.
# If the logs target doesn't show the logs, check the following file on the device:  ~/.cache/upstart/application-click-utwallet.ulrichard_utwallet_1.0.0.log

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

setup:
	sudo apt install docker.io adb git python3 python3-pip mesa-utils libgl1-mesa-glx
	pip3 install --user --upgrade clickable-ut
