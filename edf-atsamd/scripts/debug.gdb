target remote :3333
load
monitor reset halt
b main
continue
tui e