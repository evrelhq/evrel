//! Observable effects of IR operations.

/// Observable operation effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperationEffects {
    may_throw: bool,
    may_suspend: bool,
    may_have_observable_effects: bool,
}

impl OperationEffects {
    /// An operation with no represented observable effects.
    pub const NONE: Self = Self::new(false, false, false);

    /// An operation that may terminate through a JavaScript exception.
    pub const MAY_THROW: Self = Self::new(true, false, false);

    /// An operation that may throw or suspend its current execution.
    pub const MAY_THROW_OR_SUSPEND: Self = Self::new(true, true, false);

    /// An operation whose execution may be observable without throwing.
    pub const OBSERVABLE: Self = Self::new(false, false, true);

    /// An operation that may both throw and otherwise be observable.
    pub const MAY_THROW_AND_OBSERVABLE: Self = Self::new(true, false, true);

    /// An operation that may throw, suspend, or otherwise be observable.
    pub const MAY_THROW_OR_SUSPEND_AND_OBSERVABLE: Self = Self::new(true, true, true);

    const fn new(may_throw: bool, may_suspend: bool, may_have_observable_effects: bool) -> Self {
        Self {
            may_throw,
            may_suspend,
            may_have_observable_effects,
        }
    }

    /// Conservatively combines two effect sets.
    pub const fn union(self, other: Self) -> Self {
        Self::new(
            self.may_throw || other.may_throw,
            self.may_suspend || other.may_suspend,
            self.may_have_observable_effects || other.may_have_observable_effects,
        )
    }

    /// Returns whether evaluating the operation has no represented effects.
    pub const fn is_empty(self) -> bool {
        !self.may_throw && !self.may_suspend && !self.may_have_observable_effects
    }

    /// Returns whether evaluating the operation may throw.
    pub const fn may_throw(self) -> bool {
        self.may_throw
    }

    /// Returns whether evaluating the operation may suspend execution.
    pub const fn may_suspend(self) -> bool {
        self.may_suspend
    }

    /// Returns whether execution may otherwise be observable to JavaScript.
    pub const fn may_have_observable_effects(self) -> bool {
        self.may_have_observable_effects
    }
}

impl Default for OperationEffects {
    fn default() -> Self {
        Self::NONE
    }
}

#[cfg(test)]
mod tests {
    use super::OperationEffects;

    #[test]
    fn unions_independent_effects() {
        let effects = OperationEffects::MAY_THROW.union(OperationEffects::OBSERVABLE);

        assert_eq!(effects, OperationEffects::MAY_THROW_AND_OBSERVABLE);
    }

    #[test]
    fn identifies_only_effect_free_summaries_as_empty() {
        assert!(OperationEffects::NONE.is_empty());
        assert!(!OperationEffects::MAY_THROW.is_empty());
        assert!(!OperationEffects::MAY_THROW_OR_SUSPEND.is_empty());
        assert!(!OperationEffects::OBSERVABLE.is_empty());
    }
}
