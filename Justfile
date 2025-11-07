openocd:
	openocd -s scripts -f scripts/openocd.cfg

gdb:
	DEFMT_LOG=error cargo b
	arm-none-eabi-gdb -x scripts/launch.gdb