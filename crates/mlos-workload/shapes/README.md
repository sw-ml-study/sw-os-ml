# Shapes

A real model's tensor inventory, as emufpga's importer describes it.

`minicpm5-1b.names.tsv` is the sidecar `emufpga import --sidecar-only`
wrote for `ewinregirgojr/MiniCPM5-1B-Agentic-Tooluse-Merged-FP16`: one
line per stream with rows, columns and element count, and a
`rotating-streams` count saying how many of them are swept once per
token and rewound. `minicpm5-1b.order` is the order file that produced
it -- the Llama forward pass, per layer q, k, v, o, gate, up, down --
and its header carries the provenance.

Both are copies of `emufpga/layouts/` at emufpga commit named in
`docs/status.md`; the source of truth is there (external ask A1). They
are text, kilobytes each, and the weights they describe are never read
by anything in this repository: what M3 needs is what a model IS, not
its bytes.
