#!/usr/bin/env bash
# Re-record the two architecture demos and put the optimised GIFs where
# the site serves them from.
#
#   scripts/render-demos.sh            # both
#   scripts/render-demos.sh aarch64    # one
#
# VHS writes a raw GIF into videos/; gifsicle then cuts it to 16 colours
# with light lossy coding, which on a two-colour terminal is invisible and
# a 3x reduction (docs/tour.gif measured it first). The optimised GIF is
# smaller than an animated WebP of the same recording -- 37 KB against
# 47 KB for aarch64, 82 KB against 144 KB for x86-64 -- so GIF is what
# pages/ holds and the README shows. Deploy with scripts/deploy-pages.sh.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
for arch in "${@:-aarch64 x86-64}"; do
    vhs "demos/$arch.tape"
    gifsicle -O3 --colors 16 --lossy=60 "videos/$arch.gif" -o "pages/$arch.gif"
    rm -f "videos/$arch.gif" "videos/$arch.txt"
    ls -la "pages/$arch.gif"
done
