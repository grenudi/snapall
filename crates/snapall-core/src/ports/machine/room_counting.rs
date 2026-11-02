// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How the disk counts the room snapshots take, which way it can be switched, and whether that can be done.

/// How a disk counts the room snapshots take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaMode {
    /// Not counted: the room a snapshot holds is not known.
    Off,
    /// Fully counted: what each snapshot alone holds is known, at a cost when snapshots are deleted.
    Full,
    /// The cheap kind (kernel 6.7 and later): room is charged to whoever wrote it first, so what a
    /// snapshot holds is not known either.
    Simple,
    /// Could not be found out.
    Unknown,
}

/// What asking for room counting to be switched on or off comes to, given how the disk counts it now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switching {
    /// It is already that way: nothing to do.
    AlreadyThatWay,
    /// The simple kind cannot be changed to the full kind in place.
    CannotBeDoneInPlace,
    Possible,
}

/// Which way the disk's room counting is being switched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counting {
    On,
    Off,
}

impl Counting {
    /// For the places that take a plain yes or no: the machine's own call.
    pub fn is_on(self) -> bool {
        self == Counting::On
    }

    /// `on` or `off`, as it is said to a person and as it is typed.
    pub fn word(self) -> &'static str {
        match self {
            Counting::On => "on",
            Counting::Off => "off",
        }
    }

    /// The other way: what switching this back would be.
    pub fn opposite(self) -> Self {
        match self {
            Counting::On => Counting::Off,
            Counting::Off => Counting::On,
        }
    }
}

impl QuotaMode {
    /// How the disk counts, in a few words: `off`, `on`, `on, the simple kind`, or that it could not be found out.
    pub fn word(self) -> &'static str {
        match self {
            QuotaMode::Off => "off",
            QuotaMode::Full => "on",
            QuotaMode::Simple => "on, the simple kind",
            QuotaMode::Unknown => "could not be found out",
        }
    }

    pub fn switching(self, to: Counting) -> Switching {
        match (self, to) {
            (QuotaMode::Simple, Counting::On) => Switching::CannotBeDoneInPlace,
            (QuotaMode::Full, Counting::On) | (QuotaMode::Off, Counting::Off) => {
                Switching::AlreadyThatWay
            }
            (QuotaMode::Off, Counting::On)
            | (QuotaMode::Full, Counting::Off)
            | (QuotaMode::Simple, Counting::Off)
            | (QuotaMode::Unknown, _) => Switching::Possible,
        }
    }
}
