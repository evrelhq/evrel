//! Memory effects of IR operations.

use bitflags::bitflags;

bitflags! {
    /// Whether evaluating an operation may read or write memory.
    ///
    /// These are context-independent, location-insensitive facts. Use a
    /// ModRef query when asking about one particular abstract location.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct MemoryEffects: u8 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
    }
}

impl MemoryEffects {
    /// An operation proven not to access memory.
    pub const NONE: Self = Self::empty();

    /// An operation that may read and write arbitrary memory.
    pub const UNKNOWN: Self = Self::READ.union(Self::WRITE);

    /// Returns whether the operation may read memory.
    pub const fn may_read(self) -> bool {
        self.contains(Self::READ)
    }

    /// Returns whether the operation may write memory.
    pub const fn may_write(self) -> bool {
        self.contains(Self::WRITE)
    }
}

#[cfg(test)]
mod tests {
    use super::MemoryEffects;

    #[test]
    fn combines_independent_memory_effects() {
        let effects = MemoryEffects::READ.union(MemoryEffects::WRITE);

        assert_eq!(effects, MemoryEffects::UNKNOWN);
        assert!(effects.may_read());
        assert!(effects.may_write());
        assert_eq!(MemoryEffects::NONE, MemoryEffects::empty());
    }
}
