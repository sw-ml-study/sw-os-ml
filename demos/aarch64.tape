# M3 from the shell: the kernel chooses what to keep, and says what it cost.
#
# Boots MLOS under HVF, registers the synthetic model against an arena a
# quarter its size, shows a hit costing nothing, then replays the M3
# workload under two policies IN THE KERNEL -- the same policy crates the
# simulator runs, producing the same integers (step 009). The last line
# is the one the milestone exists for: next-use reads less than LRU.
#
# VHS writes GIF, not WebP, so render and convert, as x86-64.tape does:
#
#     vhs demos/aarch64.tape
#     gif2webp -min_size -mixed videos/aarch64.gif -o videos/aarch64.webp
#     rm videos/aarch64.gif
#
# The WebP is what ./videos holds and what the README shows.

Output videos/aarch64.gif

Set FontSize 13
Set Width 900
Set Height 640
Set Padding 10
Set Theme "Builtin Dark"
Set Framerate 10

Hide
Type "cargo build -q -p mlos-cli && clear && ./target/debug/mlos run hvf"
Enter
Sleep 6s
Show

Sleep 1s
Type "model 32"
Sleep 500ms
Enter
Sleep 1500ms

Type "get 3 7"
Sleep 500ms
Enter
Sleep 1200ms

Type "get 3 7"
Sleep 500ms
Enter
Sleep 1500ms

Type "replay lru"
Sleep 500ms
Enter
Sleep 4s

Type "model 32"
Sleep 400ms
Enter
Sleep 1s

Type "replay next-use"
Sleep 500ms
Enter
Sleep 4s

Type "arena"
Sleep 500ms
Enter
Sleep 3s
