//! A byte queue for handing input from an interrupt handler to a loop.
//!
//! Its own crate because nothing about it is shell-specific: any device
//! whose interrupt produces bytes faster than something wants to consume
//! them needs exactly this, and a shell is only the first such consumer.
//!
//! Bytes arrive in an interrupt handler and are consumed by the idle loop.
//! An interrupt handler is the wrong place to run a command: it holds the
//! interrupt active, so the console cannot report anything that happens
//! while it works, and a slow command stops the timer. The handler's whole
//! job is to move the byte somewhere and get out.
//!
//! Single producer, single consumer, and both are on the same core -- the
//! producer just happens to have interrupted the consumer. That is what
//! makes two atomics sufficient and a lock unnecessary.

#![no_std]

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Queue capacity. A power of two so the wrap is a mask.
const CAPACITY: usize = 64;

/// The bytes.
struct Buffer(UnsafeCell<[u8; CAPACITY]>);

// SAFETY: written only by the producer, read only by the consumer, each at
// an index the other does not touch. The atomics below order the handoff.
unsafe impl Sync for Buffer {}

/// Storage.
static BUFFER: Buffer = Buffer(UnsafeCell::new([0; CAPACITY]));
/// Next slot to write, only ever advanced by the producer.
static HEAD: AtomicUsize = AtomicUsize::new(0);
/// Next slot to read, only ever advanced by the consumer.
static TAIL: AtomicUsize = AtomicUsize::new(0);

/// Adds a byte. Returns `false` if the queue is full, dropping it.
///
/// Dropping is the right failure: a keystroke lost when 64 are already
/// waiting is a keystroke nobody was going to read in time anyway, and the
/// alternative -- blocking in an interrupt handler -- is a hang.
pub fn push(byte: u8) -> bool {
    let head = HEAD.load(Ordering::Relaxed);
    let next = (head + 1) % CAPACITY;
    if next == TAIL.load(Ordering::Acquire) {
        return false;
    }
    // SAFETY: `head` is the producer's own index, which the consumer never
    // writes, and the slot is not published until the store below.
    unsafe { (*BUFFER.0.get())[head] = byte };
    HEAD.store(next, Ordering::Release);
    true
}

/// Takes the oldest byte, if any.
pub fn pop() -> Option<u8> {
    let tail = TAIL.load(Ordering::Relaxed);
    if tail == HEAD.load(Ordering::Acquire) {
        return None;
    }
    // SAFETY: the producer's Release store published this slot, and it
    // will not reuse it until the store below frees it.
    let byte = unsafe { (*BUFFER.0.get())[tail] };
    TAIL.store((tail + 1) % CAPACITY, Ordering::Release);
    Some(byte)
}
