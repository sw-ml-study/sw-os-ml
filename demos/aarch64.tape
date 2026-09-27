# M3 from the shell: the kernel chooses what to keep, and says what it cost.
#
# Boots MLOS under HVF, registers the synthetic model against an arena a
# quarter its size, shows a hit costing nothing, then replays the M3
# workload under two policies IN THE KERNEL -- the same policy crates the
# simulator runs, producing the same integers (step 009). The last line
# is the one the milestone exists for: next-use reads less than LRU.
#
# Rendered by scripts/render-demos.sh: VHS writes the raw GIF here, gifsicle
# optimises it into pages/aarch64.gif, and the README shows it from the
# published site. The optimised GIF (37 KB) beat the WebP (47 KB).

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
