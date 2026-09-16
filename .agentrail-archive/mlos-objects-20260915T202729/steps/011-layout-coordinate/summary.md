docs/layout-handoff.md is the deliverable: what MLOS emits, what the three sibling repos must add to render it, and the questions only they can answer. Saga mlos-objects is complete, 11 steps.

THE ACTIONABLE BLOCKER, for whoever relays this: sw-mlpl's u:default_palette() is STRICT -- select_rows errors on a kind with no row -- so MLOS does not render today. Three rows fix it: reserved, rodata, weight-tile. That is all. It is that small on purpose: MLOS's kernel sections were mapped onto the palette words SWTOS already needs (text/data/bss/stack/padding/free) instead of being given MLOS-shaped names, so the whole sysram space renders with no change on their side.

Later, when other classes are registered, they will also need: scale, expert, kv-block, activation, embed-block, rag-block, adapter.

OTHER CLOSED SETS handed over: spaces disk/dram/sysram (sysram is static-document-only); owners kernel / 'model N' / ''; states resident/evicted/never/fixed; tiers hot/warm/cold/stream/archive/''; next_use never / 'distance N' / 'probability P' / ''.

TWO THINGS ASKED OF demo-extensions, not assumed: (1) does set_boxes + set_highlight express two spaces drawn apart with 32 'backs' edges between them, or does that want its own primitive; (2) nothing in the proposed surface obviously drives an animation from the event stream. Also flagged honestly that MLOS's sysram is 512 MiB of which ~509 MiB is one free region against a 115 KiB .text -- linear rendering makes the kernel a hairline, and that needs a log scale or an elided tail on their side. SWTOS does not hit it.

ONE QUESTION BACK TO sw-tos: they emit space_used and program_* columns that are not in the pinned contract. Promote, or keep as producer extensions like ours? A consumer cannot tell the difference today.

THE DOCUMENT CANNOT DRIFT. crates/mlos-image-map/tests/handoff.rs checks that every word the published samples use appears in it, that every kind sw-mlpl lacks is in the 'Add now' FENCED LIST (not merely discussed nearby), and that the three sha256 digests it publishes are the digests of the files beside it. Mutation-checked four ways: renamed kind, stale checksum, kind dropped from the list, new kind in the data -- the last failing all three tests, which is the case that matters because that is what a new ObjectClass looks like.

Every list was derived from the emitted samples and the closed sets in source, not typed. An earlier summary of mine said the vocabulary was 'kernel/arena/free/padding'; it is not, and the document says what the emitters say.

Checksums published in full, not abbreviated -- three repos pin them.

The test needed sha256 and this workspace has no dependencies, so it has a 60-line implementation in the test file. Taking the first dependency in the tree would have been the larger change.

README was two milestones stale ('pre-implementation, the kernel is not written'); now states three of eight gates met and points at the handoff. Its own test caught me writing shell commands with trailing '#' comments, which is what that test exists to forbid.

status.md now answers 'is this picture of a real system?': a runtime document is a real kernel with a real virtio-blk device managing a SYNTHETIC model; a static document is a build.

NEXT SAGA is mlos-nextuse (M3) -- the milestone the project exists for. Everything built so far is mechanism; nothing has decided anything yet. First step trace-format.

sw-checklist 157/0/4, none new.