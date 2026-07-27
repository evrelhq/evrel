//! Structured JavaScript object-literal operations.

use crate::{FunctionId, RegionId};

use super::OperationEffects;

/// Property key retained by an object literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObjectLiteralKey {
    /// A statically known property name.
    Static(Box<str>),

    /// A source expression whose result the object operation converts to a
    /// property key.
    Computed { expression: RegionId },
}

/// The semantic form of an object-literal method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectMethodKind {
    Method,
    Getter,
    Setter,
}

/// One source-ordered component of an object literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObjectLiteralEntry {
    /// Define an enumerable, writable, configurable own data property.
    Property {
        key: ObjectLiteralKey,
        value: RegionId,
    },

    /// Define a concise method, getter, or setter with `[[HomeObject]]`.
    Method {
        kind: ObjectMethodKind,
        key: ObjectLiteralKey,
        function: FunctionId,
    },

    /// Copy enumerable own properties from an evaluated source value.
    Spread { expression: RegionId },

    /// Apply object-literal `__proto__` initializer semantics.
    Prototype { expression: RegionId },
}

impl ObjectLiteralEntry {
    pub(crate) fn visit_regions(&self, visit: &mut impl FnMut(RegionId)) {
        match self {
            Self::Property {
                key: ObjectLiteralKey::Static(_),
                value,
            } => visit(*value),

            Self::Property {
                key: ObjectLiteralKey::Computed { expression },
                value,
            } => {
                visit(*expression);
                visit(*value);
            }

            Self::Method {
                key: ObjectLiteralKey::Static(_),
                ..
            } => {}

            Self::Method {
                key: ObjectLiteralKey::Computed { expression },
                ..
            } => visit(*expression),

            Self::Spread { expression } | Self::Prototype { expression } => visit(*expression),
        }
    }

    fn referenced_function(&self) -> Option<FunctionId> {
        match self {
            Self::Method { function, .. } => Some(*function),
            _ => None,
        }
    }
}

/// Creates a complete JavaScript object literal.
///
/// Entry regions execute in source order. Computed-key regions execute before
/// their corresponding value regions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectLiteralOp {
    entries: Box<[ObjectLiteralEntry]>,
}

impl ObjectLiteralOp {
    /// Creates an object literal from source-ordered entries.
    pub fn new(entries: impl Into<Box<[ObjectLiteralEntry]>>) -> Self {
        Self {
            entries: entries.into(),
        }
    }

    /// Returns literal entries in source order.
    pub fn entries(&self) -> &[ObjectLiteralEntry] {
        &self.entries
    }

    /// Returns expression regions in semantic evaluation order.
    pub fn regions(&self) -> Vec<RegionId> {
        let mut regions = Vec::new();

        for entry in &self.entries {
            entry.visit_regions(&mut |region| regions.push(region));
        }

        regions
    }

    /// Returns deferred method and accessor functions.
    pub fn referenced_functions(&self) -> Vec<FunctionId> {
        self.entries
            .iter()
            .filter_map(ObjectLiteralEntry::referenced_function)
            .collect()
    }

    /// Returns the intrinsic effects of assembling evaluated entries.
    pub fn effects(&self) -> OperationEffects {
        if self.entries.iter().any(|entry| {
            matches!(
                entry,
                ObjectLiteralEntry::Spread { .. }
                    | ObjectLiteralEntry::Property {
                        key: ObjectLiteralKey::Computed { .. },
                        ..
                    }
                    | ObjectLiteralEntry::Method {
                        key: ObjectLiteralKey::Computed { .. },
                        ..
                    }
            )
        }) {
            OperationEffects::MAY_THROW
        } else {
            OperationEffects::NONE
        }
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use crate::{FunctionId, RegionId};

    use super::{ObjectLiteralEntry, ObjectLiteralKey, ObjectLiteralOp, ObjectMethodKind};

    #[test]
    fn preserves_object_entries_and_region_order() {
        let key = RegionId::from_index(1);
        let value = RegionId::from_index(2);
        let spread = RegionId::from_index(3);
        let method_key = RegionId::from_index(4);
        let method = FunctionId::from_index(1);
        let operation = ObjectLiteralOp::new([
            ObjectLiteralEntry::Property {
                key: ObjectLiteralKey::Computed { expression: key },
                value,
            },
            ObjectLiteralEntry::Spread { expression: spread },
            ObjectLiteralEntry::Method {
                kind: ObjectMethodKind::Getter,
                key: ObjectLiteralKey::Computed {
                    expression: method_key,
                },
                function: method,
            },
        ]);

        assert_eq!(operation.regions(), [key, value, spread, method_key]);
        assert_eq!(operation.referenced_functions(), [method]);
        assert_eq!(operation.operand_count(), 0);
        assert_eq!(operation.result_count(), 1);
        assert!(operation.effects().may_throw());
    }
}
