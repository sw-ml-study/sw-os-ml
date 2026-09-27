//! A real model's tensor inventory, read from emufpga's sidecar.
//!
//! The thing `docs/architecture.md` s.12 says a trace must not be
//! without: a shape taken from a real checkpoint rather than invented.
//! The sidecar is what `emufpga import` writes beside a `.spm` -- one
//! line per stream with rows, columns and element count, and a
//! `rotating-streams` line saying how many of them are swept once per
//! operation and rewound. Everything after that boundary is read once
//! into RAM. That is an access pattern declared by the model's own
//! forward pass, and this module only reads it.
//!
//! Kilobytes of text. The weights themselves are never opened here, and
//! never will be: what a residency policy needs to know about a model is
//! what it IS, not its bytes.

/// One stream of the sidecar: a tensor, where it sits, and how it is used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stream<'a> {
    /// The checkpoint's own name for it.
    pub name: &'a str,
    /// Its shape as the stream holds it.
    pub rows: u32,
    /// Columns, so `rows * cols` is the element count.
    pub cols: u32,
    /// What it weighs, at the precision the shape was read at.
    pub bytes: u32,
    /// Which layer, or [`Shape::layers`] for the streams outside any --
    /// embeddings, the final norm, the output head.
    pub layer: u16,
    /// Its ordinal among the streams of that layer, which is what the
    /// `tensor` field of an `ObjectId` carries.
    pub tensor: u16,
    /// Swept once per token, or read once and kept.
    pub rotating: bool,
}

/// A model's inventory: every stream, and where the rotating region ends.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Shape<'a> {
    /// What the sidecar said it was for.
    pub name: &'a str,
    /// Every stream, in sidecar order -- which is consumption order for
    /// the rotating region, because that is what the order file is.
    pub streams: Vec<Stream<'a>>,
    /// How many layers the names spoke of. Streams outside any layer
    /// carry this as theirs.
    pub layers: u16,
    /// What one element weighs, so a stream has a size in bytes.
    pub bytes_per_element: u32,
}

/// The layer a name belongs to, if it is inside one.
const OUTSIDE: u16 = u16::MAX;

impl<'a> Shape<'a> {
    /// Reads a sidecar.
    ///
    /// `bytes_per_element` is the caller's to say: the sidecar counts
    /// elements and does not know what they will be stored as. Two for
    /// the checkpoint's own F16; a policy comparison at another
    /// precision changes that one number and nothing else.
    pub fn parse(text: &'a str, bytes_per_element: u32) -> Result<Self, &'static str> {
        let mut rotating = None;
        let mut streams = Vec::new();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("# rotating-streams\t") {
                rotating = Some(rest.trim().parse().map_err(|_| "bad rotating count")?);
            } else if !line.starts_with('#') && !line.trim().is_empty() {
                streams.push(Self::row(line, bytes_per_element)?);
            }
        }
        let rotating = rotating.ok_or("no rotating-streams line")?;
        let name = text
            .lines()
            .next()
            .and_then(|first| first.strip_prefix("# Stream names for "))
            .unwrap_or("unnamed");
        Ok(Self::place(name, streams, rotating, bytes_per_element))
    }

    /// One `stream / name / rows / cols / elements` line.
    ///
    /// `model.layers.N.` in the name says which layer; anything without
    /// it is marked as outside any, and `place` files it.
    fn row(line: &'a str, bytes_per_element: u32) -> Result<Stream<'a>, &'static str> {
        let fields: Vec<&str> = line.split('\t').collect();
        let [_, name, rows, cols, ..] = fields.as_slice() else {
            return Err("expected stream, name, rows, cols, elements");
        };
        let rows: u32 = rows.parse().map_err(|_| "bad rows")?;
        let cols: u32 = cols.parse().map_err(|_| "bad cols")?;
        let layer = name
            .strip_prefix("model.layers.")
            .and_then(|rest| rest.split('.').next()?.parse().ok())
            .unwrap_or(OUTSIDE);
        let bytes = (u64::from(rows) * u64::from(cols) * u64::from(bytes_per_element))
            .try_into()
            .unwrap_or(u32::MAX);
        Ok(Stream {
            name,
            rows,
            cols,
            bytes,
            layer,
            tensor: 0,
            rotating: false,
        })
    }

    /// Gives every stream an ordinal within its layer, and files the
    /// streams outside any layer under one past the last real one, so an
    /// `ObjectId` can name them without a class of their own.
    fn place(name: &'a str, mut streams: Vec<Stream<'a>>, rotating: usize, bpe: u32) -> Self {
        let layers = streams
            .iter()
            .filter(|s| s.layer != OUTSIDE)
            .map(|s| s.layer + 1)
            .max()
            .unwrap_or(0);
        let mut ordinal = vec![0u16; usize::from(layers) + 1];
        for (at, stream) in streams.iter_mut().enumerate() {
            if stream.layer == OUTSIDE {
                stream.layer = layers;
            }
            stream.tensor = ordinal[usize::from(stream.layer)];
            ordinal[usize::from(stream.layer)] += 1;
            stream.rotating = at < rotating;
        }
        Self {
            name,
            streams,
            layers,
            bytes_per_element: bpe,
        }
    }

    /// What one token adds to one layer's KV cache, in bytes.
    ///
    /// Read off the model rather than assumed: the key and value
    /// projections' output widths ARE the per-token cache row, so the
    /// number is `rows(k_proj) + rows(v_proj)` elements of layer zero.
    /// Zero for a shape with no attention in it, which a caller should
    /// treat as "this is not a decoder".
    #[must_use]
    pub fn kv_block_bytes(&self) -> u32 {
        self.streams
            .iter()
            .filter(|s| s.layer == 0 && (s.name.contains("k_proj") || s.name.contains("v_proj")))
            .map(|s| s.rows * self.bytes_per_element)
            .sum()
    }
}
