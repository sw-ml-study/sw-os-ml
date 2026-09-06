# Types into MLOS over the serial console and records the result.
#
# `vhs` gives QEMU a real pty. That is the point: a piped stdin is not a
# terminal, so the PL011's receive FIFO never sees the keystrokes and the
# input path cannot be exercised at all without one.
#
# `mlos run` builds, objcopies and boots; the build is hidden because
# cargo output is not the demo.

Output demos/echo.gif
# Also as text, so a change to the echo path fails a grep rather than
# somebody's eyes.
Output demos/echo.txt

Set FontSize 14
Set Width 820
Set Height 760
Set Padding 10

Hide
Type "cargo build -q -p mlos-cli && clear && ./target/debug/mlos run hvf"
Enter
Sleep 3s
Show

Type "hello mlos"
Sleep 1.5s
Enter
Sleep 800ms
Type "typing works"
Sleep 2s
