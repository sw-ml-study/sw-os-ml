mlos runtime boots MLOS under TCG, registers the model, sweeps it and writes build/runtime-layout.json from the live object table, in the same columnar contract as step 008. Two spaces (disk 128 KiB, dram 32 KiB), 160 regions, 32 backs edges.

THE JOIN WORKS, which is the point: 128 weight tiles carry the same region_id in both documents; 32 of them read 'never' in the static file and 'resident' in the runtime one. Verified by test, not by eye.

NEW CHANNEL, and step 010 needs it: mlos run --capture cannot drive the shell -- a log file is not a terminal, so no keystroke reaches the guest and the only way to type at mlsh was a VHS tape on a real pty. /chosen/bootargs now carries mlsh.run=model;sweep;layout (echoed as if typed) and mlos.rev=<sha> (stamped into provenance, since the kernel cannot know its own commit). mlos_machine::setting() parses them.

REAL BUG FOUND AND FIXED: mlos-fdt's string() documented that device-tree property strings are NUL-terminated, then called trim_ascii_end, which trims whitespace and leaves NUL. /chosen/bootargs had been returning 'console=hvc0\0' since step 004. Every contains/starts_with still matched, so it was invisible until a bootarg was written into JSON and the parser refused the control character. Fixed at the source; regression test in crates/mlos-fdt/tests/qemu_virt.rs.

TWO NEW no_std CRATES. mlos-spaces holds the id scheme and region vocabulary, shared by host and guest -- this is the factoring step 008's summary predicted would be needed. mlos-snapshot streams the document to a Write with no allocator: rows are resolved into a flat Row struct so each of 16 columns is a one-line field read, and each column walks the table again rather than buffering.

CONTRACT CHECK MOVED FROM TYPES TO TEXT. The two emitters share no rendering code and cannot (one builds Vecs, one cannot allocate). What they share is the bytes, so mlos_layout::validate(&str) is now the single validator; mlos runtime runs it before writing. Doc::check is gone.

MUTATION MATRIX (each validator part vs the 11 contract tests): neutering validate fails 9, aligned fails exactly a_short_column_fails, refs fails exactly the 3 id/edge tests, tiles fails exactly the 3 tiling tests.

tests/undisturbed.rs is its own file because Manager::acquire is one letter from Manager::table.get and an emitter written as 'to describe this object, get it' would inflate the counters M3 is judged on while passing every other test.

CONSOLE CEILING for anyone scaling the model: 25 KB, ~2.2 s at 115200 baud. A 64x64 model would be ~624 KiB and ~55 s -- that is where this needs framing or compression.

KNOWN LIMITS, all stated in docs/status.md: no sysram space in the runtime document (a running kernel has no symbol table and cannot say where its .text ended); region_next_use is always 'never' because nothing writes the field until M3; the snapshot is a still, not a film -- step 010 adds the events.

Samples committed at examples/viz/, both from clean tree 3fc15eaa644e:
  storage-layout.json sha256 363e6f7e1df6912cf15fb825ca17c34054cb56a40efc1d65d9e758da7adef523
  runtime-layout.json sha256 c93797d12d7c0b602dfe830f10d02fb1a12181a457c48bdbaa5b6674101f4a4c
A test refuses a sample whose revision ends in -dirty, so a sample nobody else could reproduce cannot be committed.

sw-checklist 152/0/4: three standing exceptions plus a new one, mlos-layout at 5 modules. Splitting the textual validator into a reader and a checker took three warnings to one; per AGENTS.md that is where restructuring stops.