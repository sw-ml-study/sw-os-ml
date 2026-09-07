//! The resident provider, exercised against a real buffer on the host.
//!
//! Its whole job is a bounds-checked copy, and the interesting cases are
//! the ones where the bounds are wrong -- which is exactly what a kernel
//! never gets to observe safely.

use mlos_abi::{Error, Fields, ObjectClass, ObjectId};
use mlos_objtab::ProviderId;
use mlos_provider::{Located, Provider};
use mlos_provider_dram::Dram;

/// A buffer standing in for a window of physical memory.
///
/// Leaked so its address stays valid for the provider's lifetime, which
/// is what a real memory window is.
fn window(bytes: &[u8]) -> (Dram, u64) {
    let leaked: &'static mut [u8] = Vec::from(bytes).leak();
    let base = leaked.as_ptr() as u64;
    // SAFETY: the buffer is leaked, so the range stays mapped and
    // readable for the rest of the process.
    (
        unsafe { Dram::new(ProviderId(1), base, leaked.len() as u64) },
        base,
    )
}

fn object(handle: u64, size: u32) -> Located {
    let id = ObjectId::new(
        ObjectClass::WeightTile,
        Fields {
            model: 1,
            layer: 2,
            tensor: 3,
            tile: 0,
        },
    );
    Located { id, handle, size }
}

#[test]
fn it_reads_what_is_there() {
    let source: Vec<u8> = (0..64).collect();
    let (dram, base) = window(&source);

    let mut into = [0u8; 16];
    assert_eq!(dram.read(object(base, 64), 0, &mut into), Ok(16));
    assert_eq!(into, source[..16]);

    assert_eq!(dram.read(object(base, 64), 32, &mut into), Ok(16));
    assert_eq!(into, source[32..48]);
}

/// A partial read is normal, not an error: an object can be larger than
/// the buffer a caller is willing to give.
#[test]
fn a_short_object_yields_a_short_read() {
    let (dram, base) = window(&[7u8; 8]);
    let mut into = [0u8; 32];

    assert_eq!(dram.read(object(base, 8), 0, &mut into), Ok(8));
    assert_eq!(into[..8], [7u8; 8]);
    assert_eq!(
        into[8..],
        [0u8; 24],
        "nothing beyond the object was touched"
    );
}

/// Reading past the end of an object stops at the object, not at the
/// buffer -- otherwise one object's read leaks the next one's bytes.
#[test]
fn a_read_past_the_end_returns_nothing() {
    let (dram, base) = window(&[1u8; 32]);
    let mut into = [0u8; 8];
    assert_eq!(dram.read(object(base, 8), 8, &mut into), Ok(0));
}

/// The case that matters. The handle comes from a table that object
/// metadata populated; if that is wrong, an unchecked provider turns it
/// into an arbitrary memory read.
#[test]
fn a_handle_outside_the_window_is_refused() {
    let (dram, base) = window(&[0u8; 32]);
    let mut into = [0u8; 8];

    assert_eq!(
        dram.read(object(base - 4096, 8), 0, &mut into),
        Err(Error::BadObject)
    );
    assert_eq!(
        dram.read(object(base + 4096, 8), 0, &mut into),
        Err(Error::BadObject)
    );
    assert_eq!(
        dram.read(object(base + 28, 8), 0, &mut into),
        Err(Error::BadObject),
        "straddles the end"
    );
    assert_eq!(
        dram.read(object(u64::MAX - 1, 8), 0, &mut into),
        Err(Error::BadObject),
        "overflows"
    );
}

/// Resident memory reports no transfer time, so cost comparisons against
/// a real medium come out the right way round.
#[test]
fn resident_costs_nothing_against_a_real_medium() {
    let (dram, base) = window(&[0u8; 32]);
    let resident = dram.cost(object(base, 32)).for_bytes(32);

    let nvme = mlos_provider::Cost {
        latency: mlos_objtab::CostNs(3_000_000),
        bytes_per_ms: 1_000_000,
    };
    assert!(resident < nvme.for_bytes(32), "resident should be cheaper");
    assert!(
        nvme.for_bytes(40 << 20) > nvme.for_bytes(2 << 20),
        "size must matter too"
    );
}
