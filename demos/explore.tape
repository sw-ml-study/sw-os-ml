# Poking the object table by hand.
#
# `get` twice on the same tile is the shortest demonstration of what an
# object table is for: the second time costs nothing. `model 8` sets the
# arena to 8 KiB against a 128 KiB model, because the subject is what
# happens when memory is smaller than what you want to put in it.

Output demos/explore.gif
Output demos/explore.txt

Set FontSize 13
Set Width 860
Set Height 660
Set Padding 10
Set Theme "Builtin Dark"

Hide
Type "cargo build -q -p mlos-cli && clear && ./target/debug/mlos run hvf"
Enter
Sleep 5s
Show

Type "model 8"
Enter
Sleep 900ms
Type "get 3 7"
Enter
Sleep 800ms
Type "get 3 7"
Enter
Sleep 800ms
Type "sweep"
Enter
Sleep 900ms
Type "objs"
Enter
Sleep 900ms
Type "arena"
Enter
Sleep 2s
