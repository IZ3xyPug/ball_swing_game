.PHONY: run clean

DEVICE = ${DEVICE_ID}
ifndef DEVICE_ID
	DEVICE=$(shell xcrun simctl list devices 'iOS' | grep -v 'unavailable' | grep -v '^--' | grep -v '==' | head -n 1 | grep -E -o -i "([0-9a-f]{8}-([0-9a-f]{4}-){3}[0-9a-f]{12})")
endif

run: install
	xcrun simctl launch --console $(DEVICE) org.ramp.mobile
	
install: 
	xcrun simctl install $(DEVICE) build/Build/Products/Debug-iphonesimulator/apple.app

clean:
	rm -r build
	cargo clean


# cd ios

# xcrun simctl list devices

# ## -- copy device ID (e.g. iPhone 16 Pro is 7AE14C1C-EE77-47D2-A166-A7DEE4364143)

# xcrun simctl install 7AE14C1C-EE77-47D2-A166-A7DEE4364143 build/Build/Products/Debug-iphonesimulator/ios.app

# xcrun simctl launch --console 7AE14C1C-EE77-47D2-A166-A7DEE4364143 org.template.mobile