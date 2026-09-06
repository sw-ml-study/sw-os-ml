//! Error codes crossing the syscall boundary.

/// What a syscall reports when it does not succeed.
///
/// Discriminants are part of the ABI and are never reused or renumbered.
/// Zero is deliberately absent: it is success, and never an `Error`.
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// The raw `u64` does not decode to a well-formed [`ObjectId`].
    ///
    /// [`ObjectId`]: crate::ObjectId
    BadObject = 1,
    /// The class byte is not one of [`ObjectClass`](crate::ObjectClass).
    BadClass = 2,
    /// The object exists but is not resident, and the caller asked not to
    /// block on a model fault.
    NotResident = 3,
    /// Honouring this would put the session over its residency budget.
    NoBudget = 4,
    /// Admission control declined. **Not a failure**: refusing a session
    /// the system cannot serve within its contract is the designed
    /// outcome, and is what MLOS does instead of overcommitting and
    /// thrashing. See `docs/PRD.md` F4.
    Refused = 5,
    /// No provider can resolve the object -- it is not in any tier and
    /// cannot be recomputed.
    NoProvider = 6,
    /// The lease was revoked before the holder released it.
    Revoked = 7,
    /// The capability presented does not carry the right required.
    Denied = 8,
}

impl Error {
    /// The wire value.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    /// Decodes a wire value, rejecting anything this ABI does not define.
    #[must_use]
    pub const fn from_i32(value: i32) -> Option<Self> {
        Some(match value {
            1 => Self::BadObject,
            2 => Self::BadClass,
            3 => Self::NotResident,
            4 => Self::NoBudget,
            5 => Self::Refused,
            6 => Self::NoProvider,
            7 => Self::Revoked,
            8 => Self::Denied,
            _ => return None,
        })
    }
}

/// The result of a syscall.
pub type Result<T> = core::result::Result<T, Error>;
