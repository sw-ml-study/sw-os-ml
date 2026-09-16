mlos layout emits build/storage-layout.json in sw-mlpl's columnar sw-ml-study.system-layout contract, making MLOS the second producer after SWTOS. Three spaces, 140 regions: 128 weight tiles at Disk::sector_of on the virtio-blk image (128 KiB, block 512), the object arena (32 KiB, block 16, entirely free statically), and guest RAM (512 MiB, block 4096) split into loader reserve / text / rodata / data / bss / stack with padding and free made explicit. All three tile exactly; all 140 ids unique and non-zero.

Contract adopted unchanged; MLOS's extensions ride as extra columns the contract permits: region_tier, region_object_id, region_state. rel_* are emitted but EMPTY -- statically nothing is resident so there is nothing to draw an edge between. Flag to sw-mlpl that empty arrays arrive.

Vocabulary was chosen to minimise what sw-mlpl must add. Kernel sections use their existing palette words verbatim (text/data/bss/stack/padding/free), so the whole sysram space renders with no palette change. New rows needed: rodata, reserved, and the eight ObjectClass names (weight-tile, scale, expert, kv-block, activation, embed-block, rag-block, adapter).

region_id derivation (must not change -- step 009 joins on it): space<<28 | class<<24 | layer<<16 | tensor<<8 | tile, 31 bits. Class 0 is not a valid ObjectClass so structural regions take the class-0 range of their space and count up. Model is deliberately NOT in the id; ids::object REFUSES when model != 1 or layer/tensor > 255 rather than truncating.

Three crates rather than modules in mlos-cli, which was at its module gate: mlos-elf (ELF64 section headers), mlos-layout (the contract, knows nothing about MLOS), mlos-image-map (knows MLOS, emits through it). That split is load-bearing for step 009, which emits the same contract from a no_std kernel.

De-duplicated three constants that were stated twice: image.rs's disk shape now from mlos-synth, mlsh's DEFAULT_BUDGET from mlos_lab::ARENA_BYTES, the arena's 16-byte granularity from Arena::ALIGN. QEMU/vfkit -m now from mlos_image_map::RAM_BYTES so the map and the machine cannot disagree.

Committed sample at examples/viz/storage-layout.json, sha256 cdb0cfc1fd7c9c919444712027c483fcfe9cf275881bf1a0ebba2042e1bc4b2d, revision 807adb290086 -- sibling repos can develop against it without running MLOS. The step-011 prompt promised this artifact; it was pulled forward because demo-extensions was waiting.

Bare-target aliases gained three exclusions. That list drifted once before and shipped an unlinted crate, so it is now checked by crates/mlos-cli/tests/aliases.rs: all four aliases must agree, and the excluded set must equal exactly the crates without #![no_std]. Writing that test found a hole in its own rule (mlos-kernel has no lib.rs).

KNOWN ISSUE to relay: sysram's free region is ~509 MiB against a 115 KiB .text. Linear rendering makes the kernel a hairline. Per-space block sizes take the worst cross-space ratio from 16384:1 to 512:1, but within sysram the renderer needs a log scale or an elided free tail. SWTOS does not hit this; its flash is mostly used.