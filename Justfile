openocd:
	openocd -s scripts -f edf-atsamd/scripts/openocd.cfg

gdb:
	DEFMT_LOG=trace cargo b
	arm-none-eabi-gdb -x edf-atsamd/scripts/debug.gdb target/thumbv7em-none-eabihf/debug/edf-atsamd