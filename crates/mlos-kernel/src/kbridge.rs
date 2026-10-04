//! MLOS callbacks for K's fixed-arity OS boundary.
//!
//! Invariant: K blocks by waiting for the interrupt-driven input queue; it
//! never reads the UART directly or dispatches through the shell.

use core::slice;

use mlos_device::Console;

use crate::handlers::CONSOLE;

/// Writes K output through the boot-published terminal.
pub unsafe extern "C" fn write(ptr: *const u8, len: usize) {
    // SAFETY: K passes a readable buffer for exactly `len` bytes.
    let bytes = unsafe { slice::from_raw_parts(ptr, len) };
    // SAFETY: the terminal was published before the shell started.
    if let Some(console) = unsafe { (*CONSOLE.0.get()).as_ref() } {
        console.write(bytes);
    }
}

/// Reads a line's bytes from the interrupt-driven MLOS queue.
pub unsafe extern "C" fn read(ptr: *mut u8, len: usize) -> usize {
    let mut count = 0;
    while count < len {
        if let Some(byte) = mlos_queue::pop() {
            let byte = if byte == b'\r' { b'\n' } else { byte };
            // SAFETY: K supplied a writable buffer of `len` bytes.
            unsafe { ptr.add(count).write(byte) };
            count += 1;
            write_byte(byte);
            if byte == b'\n' {
                break;
            }
        } else {
            mlos_hal_aarch64::wait_for_interrupt();
        }
    }
    count
}

/// Echoes input using the same terminal as K's writes.
fn write_byte(byte: u8) {
    // SAFETY: the terminal was published before the shell started.
    if let Some(console) = unsafe { (*CONSOLE.0.get()).as_ref() } {
        console.write(core::slice::from_ref(&byte));
    }
}
