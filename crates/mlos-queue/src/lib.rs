//! A byte queue from an interrupt handler to the loop it interrupted.
//!
//! Invariant: single producer, single consumer, same core. `HEAD` is
//! advanced only by the producer and `TAIL` only by the consumer, and the
//! Release/Acquire pair on each is what publishes a slot. Design and
//! history: docs/notes/mlos-queue.md.

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

/// Adds a byte. `false` if the queue is full, and the byte is dropped:
/// an interrupt handler cannot block.
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
