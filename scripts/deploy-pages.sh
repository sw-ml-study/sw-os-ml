#!/usr/bin/env bash
# Deploy ./pages/ to the live site (GitHub Pages, build type "legacy":
# GitHub publishes the gh-pages branch directly, no Actions runner).
#
# gh-pages holds the SITE at its root while main holds it under pages/,
# so gh-pages gets its own checkout, as sw-mlpl does. Deploying is then
# plain git: mirror the files in, add, commit, push.
#
# One-time setup (done 2026-09-26; recreate only if ../sw-os-ml.pages is
# missing):
#   git fetch origin gh-pages
#   git worktree add ../sw-os-ml.pages gh-pages
#
# Usage: scripts/render-demos.sh && git add pages/ && git commit
#        && scripts/deploy-pages.sh
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
DEPLOY="../sw-os-ml.pages"
if [ ! -e "$DEPLOY/.git" ]; then
    echo "error: $DEPLOY is not a gh-pages checkout; see the setup note in this script" >&2
    exit 1
fi
rsync -a --delete --exclude='.git' pages/ "$DEPLOY/"
git -C "$DEPLOY" add -A
git -C "$DEPLOY" commit -q -m "deploy pages @ $(git rev-parse --short HEAD)" \
    || { echo "gh-pages already current; nothing to deploy."; exit 0; }
git -C "$DEPLOY" push -q
echo "deployed to gh-pages; GitHub publishes within about a minute."
