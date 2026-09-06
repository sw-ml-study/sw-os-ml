# Drives the MLOS shell over the serial console.
#
# `vhs` gives QEMU a real pty. A piped stdin is not a terminal, so the
# PL011's receive FIFO never sees a keystroke -- there is no way to
# exercise the shell from a pipeline.

Output demos/mlsh.gif
# Text too, so a regression fails a grep rather than somebody's eyes.
Output demos/mlsh.txt

Set FontSize 14
Set Width 900
Set Height 800
Set Padding 10

Hide
Type "cargo build -q -p mlos-cli && clear && ./target/debug/mlos run hvf"
Enter
Sleep 3s
Show

Type "help"
Sleep 700ms
Enter
Sleep 900ms

Type "dev"
Sleep 700ms
Enter
Sleep 900ms

Type "mem"
Sleep 700ms
Enter
Sleep 1s

Type "ticks"
Sleep 500ms
Enter
Sleep 1s

Type "wat"
Sleep 500ms
Enter
Sleep 1.5s
