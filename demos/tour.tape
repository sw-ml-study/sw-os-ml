# The README's front page: boot MLOS and ask it what it is.
#
# `vhs` gives QEMU a real pty, which is the only way to reach the guest's
# receive FIFO -- a piped stdin is not a terminal.
#
# Unlike demos/*.gif, this one is TRACKED: it is documentation, and a
# README that shows the thing is worth half a megabyte. Re-record
# deliberately, not on every change.

Output docs/tour.gif
Output docs/tour.txt

Set FontSize 13
Set Width 880
Set Height 700
Set Padding 12
Set Theme "Builtin Dark"

Hide
Type "cargo build -q -p mlos-cli && clear"
Enter
Sleep 2s
Show

Type "mlos run"
Sleep 800ms
Enter
Sleep 4s

Type "dev"
Sleep 600ms
Enter
Sleep 1s

Type "mem"
Sleep 600ms
Enter
Sleep 1.5s

Type "ticks"
Sleep 500ms
Enter
Sleep 2s
