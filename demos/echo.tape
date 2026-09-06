# Types into MLOS over the serial console and records the result.
#
# `vhs` gives QEMU a real pty. That is the point: a piped stdin is not a
# terminal, so the PL011's receive FIFO never sees the keystrokes and the
# input path cannot be exercised at all without one.
#
# Build first, outside the recording -- `cargo` output is not the demo.

Output demos/echo.gif
# Also as text, so a change to the echo path fails a grep rather than
# somebody's eyes.
Output demos/echo.txt

Set FontSize 14
Set Width 820
Set Height 760
Set Padding 10

Hide
Type "./scripts/image.sh >/dev/null && clear && qemu-system-aarch64 -M virt,gic-version=3 -cpu host -accel hvf -m 512 -display none -serial mon:stdio -kernel target/aarch64-unknown-none-softfloat/debug/mlos.img"
Enter
Sleep 3s
Show

Type "hello mlos"
Sleep 1.5s
Enter
Sleep 800ms
Type "typing works"
Sleep 2s
