# terminOS
a deadly fast, terminal-native desktop environment for Linux.

terminOS replaces the Linux console with a modern, pixel-capable terminal
running straight on DRM, with no X11 or Wayland underneath. On top of it
sits an ecosystem of programs that make the console feel like a full OS.

## status
early and unstable. right now: input handling via libseat + evdev.
the DRM canvas is not wired up yet.

## goals
- drop-in replacement for the Linux TTY
- real fonts, full color, Unicode
- session management: multiple shells, switching, splits
- native pixel graphics 
- an open ecosystem of terminOS programs

## dev
start a virtual test display (vkms) with
`sudo modprobe vkms`
and remove it with
`sudo modprobe -r vkms

run with debug logging (`-d`) and send logs off-screen, since the
terminal is taken over:
`./target/debug/terminos -d > /tmp/terminos.log 2>&1`
then `tail -f /tmp/terminos.log` from SSH or another terminal.

## troubleshoot
- startup can fail if the seat manager (`logind`) errors. switch to a fresh
  terminal or reboot.
