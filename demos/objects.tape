# Gates G2 and G3 from docs/PRD.md, demonstrated from the shell.
#
# G2: a model registered as ML objects across three tiers.
# G3: touching a non-resident one faults, the provider services it, and
#     the fault is counted per class.
#
# The arena is a quarter of the model on purpose. RAM is the scarce
# resource; a demonstration where everything fits demonstrates nothing.

Output demos/objects.gif
Output demos/objects.txt

Set FontSize 13
Set Width 860
Set Height 620
Set Padding 10
Set Theme "Builtin Dark"

Hide
Type "cargo build -q -p mlos-cli && clear && ./target/debug/mlos run hvf"
Enter
Sleep 5s
Show

Type "model"
Enter
Sleep 1200ms
Type "sweep"
Enter
Sleep 1200ms
Type "faults"
Enter
Sleep 2500ms
