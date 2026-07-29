//! Abstract domain for ECMAScript values.

use evrel_ir::ConstantValue;

/// Statically known information about one JavaScript SSA value.
///
/// An abstract value describes a result when its defining operation completes
/// normally. It does not imply that the operation is non-throwing, removable,
/// or free of observable effects.
#[derive(Debug, Clone)]
pub struct AbstractValue {
    facts: Option<ValueFacts>,
}

/// Shared bottom value used by sibling analysis modules.
pub(super) static BOTTOM: AbstractValue = AbstractValue::bottom();

/// Facts shared by every runtime value represented by an abstract value.
///
/// More facts—such as possible ECMAScript types, truthiness, numeric ranges,
/// and object behavior—can be added without changing `AbstractValue`'s public
/// representation.
#[derive(Debug, Clone)]
struct ValueFacts {
    constant: Option<ConstantValue>,
}

impl AbstractValue {
    /// No executable path has produced this value.
    pub const fn bottom() -> Self {
        Self { facts: None }
    }

    /// A reachable runtime value about which nothing is known.
    pub const fn unknown() -> Self {
        Self {
            facts: Some(ValueFacts { constant: None }),
        }
    }

    /// One exact JavaScript primitive constant.
    pub fn from_constant(constant: ConstantValue) -> Self {
        Self {
            facts: Some(ValueFacts {
                constant: Some(constant),
            }),
        }
    }

    /// Returns whether no executable path has produced this value.
    pub const fn is_bottom(&self) -> bool {
        self.facts.is_none()
    }

    /// Returns the exact JavaScript constant, when known.
    pub const fn constant(&self) -> Option<&ConstantValue> {
        match &self.facts {
            Some(facts) => facts.constant.as_ref(),
            None => None,
        }
    }

    /// Conservatively combines facts from independent executable paths.
    pub fn join(&self, incoming: &Self) -> Self {
        let (Some(left), Some(right)) = (&self.facts, &incoming.facts) else {
            return if self.is_bottom() {
                incoming.clone()
            } else {
                self.clone()
            };
        };

        match (&left.constant, &right.constant) {
            (Some(left), Some(right)) if same_constant(left, right) => {
                Self::from_constant(left.clone())
            }

            _ => Self::unknown(),
        }
    }
}

impl PartialEq for AbstractValue {
    fn eq(&self, other: &Self) -> bool {
        match (&self.facts, &other.facts) {
            (None, None) => true,

            (Some(left), Some(right)) => match (&left.constant, &right.constant) {
                (Some(left), Some(right)) => same_constant(left, right),
                (None, None) => true,
                _ => false,
            },

            _ => false,
        }
    }
}

/// Compares constants as abstract interpreter facts.
///
/// Signed zero remains distinct because JavaScript can observe it. All NaN
/// representations are treated as one fact so fixed-point iteration remains
/// stable independently of NaN payload bits.
fn same_constant(left: &ConstantValue, right: &ConstantValue) -> bool {
    match (left, right) {
        (ConstantValue::Number(left), ConstantValue::Number(right)) => {
            left.to_bits() == right.to_bits() || (left.is_nan() && right.is_nan())
        }

        _ => left == right,
    }
}

#[cfg(test)]
mod tests {
    use evrel_ir::ConstantValue;

    use super::AbstractValue;

    #[test]
    fn bottom_contributes_no_runtime_value() {
        let constant = AbstractValue::from_constant(ConstantValue::Number(1.0));

        assert_eq!(AbstractValue::bottom().join(&constant), constant);
        assert_eq!(constant.join(&AbstractValue::bottom()), constant);
    }

    #[test]
    fn equal_constants_survive_a_join() {
        let left = AbstractValue::from_constant(ConstantValue::Boolean(true));
        let right = AbstractValue::from_constant(ConstantValue::Boolean(true));

        assert_eq!(left.join(&right), left);
    }

    #[test]
    fn conflicting_constants_become_unknown() {
        let left = AbstractValue::from_constant(ConstantValue::Number(1.0));
        let right = AbstractValue::from_constant(ConstantValue::Number(2.0));

        assert_eq!(left.join(&right), AbstractValue::unknown());
    }

    #[test]
    fn unknown_information_remains_unknown() {
        let constant = AbstractValue::from_constant(ConstantValue::Number(1.0));

        assert_eq!(
            AbstractValue::unknown().join(&constant),
            AbstractValue::unknown(),
        );
    }

    #[test]
    fn signed_zero_values_remain_distinct() {
        let positive = AbstractValue::from_constant(ConstantValue::Number(0.0));
        let negative = AbstractValue::from_constant(ConstantValue::Number(-0.0));

        assert_eq!(positive.join(&negative), AbstractValue::unknown());
    }

    #[test]
    fn nan_is_stable_across_fixed_point_iterations() {
        let left = AbstractValue::from_constant(ConstantValue::Number(f64::NAN));
        let right = AbstractValue::from_constant(ConstantValue::Number(f64::NAN));

        assert_eq!(left.join(&right), left);
    }
}
