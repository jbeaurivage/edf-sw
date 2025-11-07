target remote :3333
file target/thumbv7em-none-eabihf/debug/edf-sw
load
monitor reset halt
tui e
step