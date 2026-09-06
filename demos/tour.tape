# The README's front page: boot MLOS and ask it what it is.
#
# `vhs` gives QEMU a real pty, which is the only way to reach the guest's
# receive FIFO -- a piped stdin is not a terminal.
#
# Unlike demos/*.gif, this one is TRACKED: it is documentation, and a
# README that shows the thing earns its bytes. Re-record deliberately, not
# on every change, and ALWAYS optimise afterwards:
#
#     vhs demos/tour.tape
#     gifsicle -O3 --colors 16 --lossy=60 docs/tour.gif -o docs/tour.gif
#
# That is a 3x reduction and it is invisible: terminal output is two
# colours and a cursor. A test enforces the budget, so a forgotten
# optimise fails the gate rather than growing the repository quietly.
#
# WebP was measured and is WORSE here -- 107% to 258% of the GIF. Lossy
# WebP spends bits on gradients, and a terminal recording has none;
# GIF's palette and run-length coding are a better fit for flat colour.

Output docs/tour.gif
Output docs/tour.txt

Set FontSize 13
Set Width 820
Set Height 640
Set Padding 10
Set Theme "Builtin Dark"

# 10 fps, not the default 25. A terminal demo is static most of the time
# and its frames are flat colour, so frame count is almost the entire file
# size -- 25 fps spends 2.5x the bytes to show the same thing.
Set Framerate 10

Hide
Type "cargo build -q -p mlos-cli && clear"
Enter
Sleep 2s
Show

Type "mlos run"
Sleep 800ms
Enter
Sleep 3500ms

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
