//! Reads a real device tree, not a synthetic one.
//!
//! `qemu-virt.dtb` is what `qemu-system-aarch64 -M virt -m 512 -smp 4`
//! actually hands a guest, captured with `-machine dumpdtb` and trimmed to
//! its declared size (dumpdtb pads to 1 MiB). Testing against a synthetic
//! blob would only prove the parser agrees with the test's author.

use mlos_fdt::{Event, Fdt, Header};

const BLOB: &[u8] = include_bytes!("qemu-virt.dtb");

/// One property, flattened: the node that held it, its name, its bytes.
type Prop = (String, String, Vec<u8>);

/// Collects every node name and every property, so the tests below can
/// assert against the real tree's shape.
fn walk_all() -> (Vec<String>, Vec<Prop>) {
    let fdt = Fdt::new(BLOB).expect("fixture is a valid blob");
    let mut path: Vec<String> = Vec::new();
    let (mut nodes, mut props) = (Vec::new(), Vec::new());

    fdt.walk(|event| match event {
        Event::Node { name } => {
            path.push(name.to_owned());
            nodes.push(name.to_owned());
        }
        Event::EndNode => {
            path.pop();
        }
        Event::Prop { name, value } => {
            let node = path.last().cloned().unwrap_or_default();
            props.push((node, name.to_owned(), value.to_vec()));
        }
    })
    .expect("fixture walks to FDT_END");

    (nodes, props)
}

#[test]
fn header_matches_the_blob_on_disk() {
    let header = Header::parse(BLOB).expect("valid header");
    assert_eq!(header.total_size, BLOB.len());
    assert!(header.struct_offset + header.struct_size <= BLOB.len());
    assert!(header.strings_offset + header.strings_size <= BLOB.len());
}

/// The root is anonymous and the tree balances -- every open node closed.
/// An unbalanced walk means the cursor lost alignment somewhere.
#[test]
fn the_tree_is_well_formed_and_balanced() {
    let (nodes, _) = walk_all();
    assert_eq!(
        nodes.first().map(String::as_str),
        Some(""),
        "root is unnamed"
    );
    assert!(nodes.iter().any(|n| n.starts_with("memory@")));
    assert!(nodes.iter().any(|n| n.starts_with("pl011@")));
    assert!(
        nodes.iter().filter(|n| n.starts_with("cpu@")).count() == 4,
        "-smp 4"
    );
}

/// The three facts MLOS actually needs at boot.
#[test]
fn the_boot_facts_are_readable() {
    let (nodes, props) = walk_all();

    // Root cells: reg values in memory@ and pl011@ are decoded with these.
    let cells = |name: &str| {
        props
            .iter()
            .find(|(node, prop, _)| node.is_empty() && prop == name)
            .map(|(_, _, v)| u32::from_be_bytes(v[..4].try_into().unwrap()))
    };
    assert_eq!(cells("#address-cells"), Some(2));
    assert_eq!(cells("#size-cells"), Some(2));

    // Memory: one 512 MiB region at the base of DRAM.
    let memory = nodes.iter().find(|n| n.starts_with("memory@")).unwrap();
    assert_eq!(memory, "memory@40000000");
    let reg = props
        .iter()
        .find(|(node, prop, _)| node == memory && prop == "reg")
        .map(|(_, _, v)| v.clone())
        .expect("memory has a reg");
    assert_eq!(
        u64::from_be_bytes(reg[0..8].try_into().unwrap()),
        0x4000_0000
    );
    assert_eq!(
        u64::from_be_bytes(reg[8..16].try_into().unwrap()),
        512 << 20
    );

    // Console: the address step 004 hardcoded, now discovered instead.
    let uart = nodes.iter().find(|n| n.starts_with("pl011@")).unwrap();
    assert_eq!(uart, "pl011@9000000");
}

/// A blob is firmware input. Every corruption must yield `None`, never a
/// panic and never a read past the end.
#[test]
fn malformed_blobs_are_rejected_not_trusted() {
    assert!(Fdt::new(&[]).is_none(), "empty");
    assert!(Fdt::new(&BLOB[..39]).is_none(), "truncated header");

    let mut bad_magic = BLOB.to_vec();
    bad_magic[0] = 0;
    assert!(Fdt::new(&bad_magic).is_none(), "wrong magic");

    let mut bad_version = BLOB.to_vec();
    bad_version[27] = 99; // last_comp_version, low byte
    assert!(Fdt::new(&bad_version).is_none(), "too new to read");

    // A struct block cut short must stop the walk, not run off the end.
    let mut truncated = BLOB.to_vec();
    let len = truncated.len();
    truncated.truncate(len - 200);
    if let Some(fdt) = Fdt::new(&truncated) {
        assert!(fdt.walk(|_| {}).is_none(), "truncated walk must fail");
    }
}
