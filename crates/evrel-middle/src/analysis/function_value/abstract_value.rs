//! Abstract domain for ECMAScript values.

use bitflags::bitflags;
use evrel_ir::ConstantValue;

bitflags! {
    /// Possible ECMAScript language types represented by an abstract value.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ValueTypeSet: u16 {
        const UNDEFINED = 1 << 0;
        const NULL = 1 << 1;
        const BOOLEAN = 1 << 2;
        const NUMBER = 1 << 3;
        const BIGINT = 1 << 4;
        const STRING = 1 << 5;
        const SYMBOL = 1 << 6;
        const OBJECT = 1 << 7;

        const NULLISH = Self::UNDEFINED.bits() | Self::NULL.bits();

        const PRIMITIVE = Self::UNDEFINED.bits()
            | Self::NULL.bits()
            | Self::BOOLEAN.bits()
            | Self::NUMBER.bits()
            | Self::BIGINT.bits()
            | Self::STRING.bits()
            | Self::SYMBOL.bits();

        const ANY = Self::PRIMITIVE.bits() | Self::OBJECT.bits();
    }
}

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
/// Additional correlated facts, such as truthiness and numeric ranges, can be
/// added here without introducing a separate dataflow analysis.
#[derive(Debug, Clone)]
struct ValueFacts {
    types: ValueTypeSet,
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
            facts: Some(ValueFacts {
                types: ValueTypeSet::ANY,
                constant: None,
            }),
        }
    }

    /// A value constrained to one or more ECMAScript language types.
    pub fn of_types(types: ValueTypeSet) -> Self {
        if types.is_empty() {
            return Self::bottom();
        }

        Self::from_facts(types, None)
    }

    /// One exact JavaScript primitive constant.
    pub fn from_constant(constant: ConstantValue) -> Self {
        Self::from_facts(type_of_constant(&constant), Some(constant))
    }

    /// Returns whether no executable path has produced this value.
    pub const fn is_bottom(&self) -> bool {
        self.facts.is_none()
    }

    /// Returns all possible ECMAScript language types.
    pub const fn types(&self) -> ValueTypeSet {
        match &self.facts {
            Some(facts) => facts.types,
            None => ValueTypeSet::empty(),
        }
    }

    /// Returns whether every represented runtime value has an allowed type.
    ///
    /// Bottom does not establish a runtime type and therefore returns `false`.
    pub const fn is_definitely(&self, allowed: ValueTypeSet) -> bool {
        let types = self.types();

        !types.is_empty() && allowed.contains(types)
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

        let constant = match (&left.constant, &right.constant) {
            (Some(left), Some(right)) if same_constant(left, right) => Some(left.clone()),
            _ => None,
        };

        Self::from_facts(left.types | right.types, constant)
    }

    fn from_facts(types: ValueTypeSet, constant: Option<ConstantValue>) -> Self {
        debug_assert!(!types.is_empty());

        if let Some(constant) = &constant {
            debug_assert!(types.contains(type_of_constant(constant)));
        }

        Self {
            facts: Some(ValueFacts { types, constant }),
        }
    }
}

impl PartialEq for AbstractValue {
    fn eq(&self, other: &Self) -> bool {
        match (&self.facts, &other.facts) {
            (None, None) => true,

            (Some(left), Some(right)) => {
                left.types == right.types
                    && match (&left.constant, &right.constant) {
                        (Some(left), Some(right)) => same_constant(left, right),
                        (None, None) => true,
                        _ => false,
                    }
            }

            _ => false,
        }
    }
}

fn type_of_constant(constant: &ConstantValue) -> ValueTypeSet {
    match constant {
        ConstantValue::Undefined => ValueTypeSet::UNDEFINED,
        ConstantValue::Null => ValueTypeSet::NULL,
        ConstantValue::Boolean(_) => ValueTypeSet::BOOLEAN,
        ConstantValue::Number(_) => ValueTypeSet::NUMBER,
        ConstantValue::BigInt(_) => ValueTypeSet::BIGINT,
        ConstantValue::String(_) => ValueTypeSet::STRING,
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

    use super::{AbstractValue, ValueTypeSet};

    #[test]
    fn bottom_contributes_no_runtime_value() {
        let constant = AbstractValue::from_constant(ConstantValue::Number(1.0));

        assert_eq!(AbstractValue::bottom().join(&constant), constant);
        assert_eq!(constant.join(&AbstractValue::bottom()), constant);
    }

    #[test]
    fn constants_record_their_ecmascript_type() {
        let number = AbstractValue::from_constant(ConstantValue::Number(1.0));
        let boolean = AbstractValue::from_constant(ConstantValue::Boolean(true));
        let null = AbstractValue::from_constant(ConstantValue::Null);

        assert_eq!(number.types(), ValueTypeSet::NUMBER);
        assert_eq!(boolean.types(), ValueTypeSet::BOOLEAN);
        assert_eq!(null.types(), ValueTypeSet::NULL);
    }

    #[test]
    fn empty_type_set_is_bottom() {
        assert_eq!(
            AbstractValue::of_types(ValueTypeSet::empty()),
            AbstractValue::bottom(),
        );
    }

    #[test]
    fn checks_definite_type_membership() {
        let number = AbstractValue::from_constant(ConstantValue::Number(1.0));

        assert!(number.is_definitely(ValueTypeSet::NUMBER));
        assert!(number.is_definitely(ValueTypeSet::NUMBER | ValueTypeSet::STRING));
        assert!(!number.is_definitely(ValueTypeSet::STRING));
        assert!(!AbstractValue::bottom().is_definitely(ValueTypeSet::ANY));
    }

    #[test]
    fn equal_constants_survive_a_join() {
        let left = AbstractValue::from_constant(ConstantValue::Boolean(true));
        let right = AbstractValue::from_constant(ConstantValue::Boolean(true));

        assert_eq!(left.join(&right), left);
    }

    #[test]
    fn conflicting_constants_retain_their_shared_type() {
        let left = AbstractValue::from_constant(ConstantValue::Number(1.0));
        let right = AbstractValue::from_constant(ConstantValue::Number(2.0));
        let joined = left.join(&right);

        assert_eq!(joined, AbstractValue::of_types(ValueTypeSet::NUMBER));
        assert_eq!(joined.constant(), None);
    }

    #[test]
    fn different_constant_types_form_a_type_union() {
        let number = AbstractValue::from_constant(ConstantValue::Number(1.0));
        let boolean = AbstractValue::from_constant(ConstantValue::Boolean(true));
        let joined = number.join(&boolean);

        assert_eq!(joined.types(), ValueTypeSet::NUMBER | ValueTypeSet::BOOLEAN,);
        assert_eq!(joined.constant(), None);
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
    fn signed_zero_values_remain_distinct_but_retain_number_type() {
        let positive = AbstractValue::from_constant(ConstantValue::Number(0.0));
        let negative = AbstractValue::from_constant(ConstantValue::Number(-0.0));

        assert_eq!(
            positive.join(&negative),
            AbstractValue::of_types(ValueTypeSet::NUMBER),
        );
    }

    #[test]
    fn nan_is_stable_across_fixed_point_iterations() {
        let left = AbstractValue::from_constant(ConstantValue::Number(f64::NAN));
        let right = AbstractValue::from_constant(ConstantValue::Number(f64::NAN));

        assert_eq!(left.join(&right), left);
    }
}
