# The x86-64 guest (saga mlos-x86-64): boot it under QEMU microvm via PVH
# and walk mlsh through what the port had to earn -- the machine from the
# PVH loader, interrupts, the model faulting in over virtio-blk with its
# provenance mark, a timed sweep, then Ctrl-D.
#
# `--arch x86-64` is explicit, so the recording is the same on an Apple
# Silicon Mac (where plain `mlos run` means aarch64) as on a Linux PC.
#
# VHS writes GIF, not WebP, so render and convert:
#
#     vhs demos/x86-64.tape
#     gif2webp -min_size -mixed videos/x86-64.gif -o videos/x86-64.webp
#     rm videos/x86-64.gif
#
# Measured, as tour.tape measured: the WebP is 144 KB, the raw VHS GIF
# 339 KB, a lossless WebP 260 KB, and a gifsicle-optimised GIF 83 KB --
# WebP is 1.7x the optimised GIF, the same finding tour.tape records. The
# WebP is what ./videos holds, as in the other Software Wrighter repos.

Output videos/x86-64.gif

Set FontSize 13
Set Width 820
Set Height 640
Set Padding 10
Set Theme "Builtin Dark"

# 10 fps, as tour.tape: a terminal is static most of the time, and frame
# count is almost the whole file size.
Set Framerate 10

Hide
Type "export PATH=$PWD/target/debug:$PATH"
Enter
Type "cargo build -q -p mlos-cli && mlos --arch x86-64 build >/dev/null && clear"
Enter
Sleep 20s
Show

Type "mlos --arch x86-64 run"
Sleep 800ms
Enter
Sleep 5s

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
Sleep 1.5s

Type "model 32"
Sleep 600ms
Enter
Sleep 2s

Type "get 3 5"
Sleep 600ms
Enter
Sleep 1.5s

Type "sweep"
Sleep 500ms
Enter
Sleep 2s

Type "ticks"
Sleep 500ms
Enter
Sleep 1.5s

Ctrl+D
Sleep 2s
