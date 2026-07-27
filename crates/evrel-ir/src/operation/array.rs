//! Structured JavaScript array-literal operations.

use crate::RegionId;

use super::OperationEffects;

/// One source-ordered component of an array literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArrayLiteralElement {
    /// Evaluate one expression and append its value.
    Value { expression: RegionId },

    /// Evaluate one expression and append values from its iterator.
    Spread { expression: RegionId },

    /// Advance the array length without defining an element.
    Elision,
}

impl ArrayLiteralElement {
    /// Returns the expression region evaluated for this element, if any.
    pub const fn expression(self) -> Option<RegionId> {
        match self {
            Self::Value { expression } | Self::Spread { expression } => Some(expression),
            Self::Elision => None,
        }
    }
}

/// Creates a complete JavaScript array literal.
///
/// Element regions execute from left to right. Spread iteration completes
/// before evaluation advances to the next element region.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayLiteralOp {
    elements: Box<[ArrayLiteralElement]>,
}

impl ArrayLiteralOp {
    /// Creates an array literal from source-ordered elements.
    pub fn new(elements: impl Into<Box<[ArrayLiteralElement]>>) -> Self {
        Self {
            elements: elements.into(),
        }
    }

    /// Returns literal elements in source order.
    pub fn elements(&self) -> &[ArrayLiteralElement] {
        &self.elements
    }

    /// Returns expression regions in semantic evaluation order.
    pub fn regions(&self) -> Vec<RegionId> {
        self.elements
            .iter()
            .filter_map(|element| element.expression())
            .collect()
    }

    /// Returns the intrinsic effects of assembling evaluated elements.
    pub fn effects(&self) -> OperationEffects {
        if self
            .elements
            .iter()
            .any(|element| matches!(element, ArrayLiteralElement::Spread { .. }))
        {
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
    use crate::RegionId;

    use super::{ArrayLiteralElement, ArrayLiteralOp};

    #[test]
    fn preserves_array_elements_and_region_order() {
        let first = RegionId::from_index(1);
        let spread = RegionId::from_index(2);
        let operation = ArrayLiteralOp::new([
            ArrayLiteralElement::Value { expression: first },
            ArrayLiteralElement::Elision,
            ArrayLiteralElement::Spread { expression: spread },
        ]);

        assert_eq!(operation.regions(), [first, spread]);
        assert_eq!(operation.operand_count(), 0);
        assert_eq!(operation.result_count(), 1);
        assert!(operation.effects().may_throw());
    }
}
