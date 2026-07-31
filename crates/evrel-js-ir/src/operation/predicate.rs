//! JavaScript semantic predicate operations.

use crate::PrivateNameId;

/// Tests whether an object contains a particular private name.
///
/// This can throw when the operand is not an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HasPrivateNameOp {
    private_name: PrivateNameId,
}

impl HasPrivateNameOp {
    pub const fn new(private_name: PrivateNameId) -> Self {
        Self { private_name }
    }

    pub const fn private_name(&self) -> PrivateNameId {
        self.private_name
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

/// Tests whether a value is exactly JavaScript `null` or `undefined`.
///
/// This follows nullish-coalescing semantics. It must not use loose equality,
/// because special host values such as `document.all` are loosely equal to
/// `null` without being nullish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IsNullishOp;

impl IsNullishOp {
    /// Creates a nullish predicate operation.
    pub const fn new() -> Self {
        Self
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

impl Default for IsNullishOp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::{JsModuleIr, ModuleBuilder};

    use super::{HasPrivateNameOp, IsNullishOp};

    #[test]
    fn defines_the_private_name_predicate_shape() {
        let mut module = JsModuleIr::new();
        let private_name = ModuleBuilder::new(&mut module).create_private_name("value");
        let operation = HasPrivateNameOp::new(private_name);

        assert_eq!(operation.private_name(), private_name);
        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 1);
    }

    #[test]
    fn defines_the_nullish_predicate_shape() {
        let operation = IsNullishOp::new();

        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 1);
    }
}
