//! Structured JSX syntax operations.

use crate::{JsString, RegionId};

use super::OperationEffects;

/// The syntactic name of a JSX element.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsxElementName {
    /// A lowercase or hyphenated intrinsic name, such as `div`.
    Intrinsic(Box<str>),

    /// A JavaScript reference, such as `Button`.
    ///
    /// The referenced value is the element operation's sole direct operand.
    Reference,

    /// A static member chain, such as `UI.Button.Primary`.
    Member {
        base: JsxMemberBase,
        properties: Box<[Box<str>]>,
    },

    /// A namespaced name, such as `svg:path`.
    Namespaced { namespace: Box<str>, name: Box<str> },

    /// The standalone JSX name `this`.
    This,
}

impl JsxElementName {
    fn validate(&self) {
        if let Self::Member { properties, .. } = self {
            assert!(
                !properties.is_empty(),
                "JSX member names require at least one property"
            );
        }
    }

    const fn operand_count(&self) -> usize {
        match self {
            Self::Reference
            | Self::Member {
                base: JsxMemberBase::Reference,
                ..
            } => 1,

            Self::Intrinsic(_)
            | Self::Member {
                base: JsxMemberBase::This,
                ..
            }
            | Self::Namespaced { .. }
            | Self::This => 0,
        }
    }
}

/// The base of a JSX member name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsxMemberBase {
    /// A JavaScript reference, such as `UI` in `UI.Button`.
    Reference,

    /// `this`, such as in `this.Button`.
    This,
}

/// A JSX attribute name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsxAttributeName {
    Identifier(Box<str>),

    Namespaced { namespace: Box<str>, name: Box<str> },
}

/// One source-ordered JSX attribute.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsxAttribute {
    Named {
        name: JsxAttributeName,
        value: Option<JsxAttributeValue>,
    },

    Spread {
        expression: RegionId,
    },
}

impl JsxAttribute {
    const fn expression(&self) -> Option<RegionId> {
        match self {
            Self::Named {
                value: Some(value), ..
            } => value.expression(),

            Self::Spread { expression } => Some(*expression),

            Self::Named { value: None, .. } => None,
        }
    }
}

/// The value of a named JSX attribute.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsxAttributeValue {
    String(JsString),

    Expression { expression: RegionId },

    Element { expression: RegionId },

    Fragment { expression: RegionId },
}

impl JsxAttributeValue {
    const fn expression(&self) -> Option<RegionId> {
        match self {
            Self::String(_) => None,

            Self::Expression { expression }
            | Self::Element { expression }
            | Self::Fragment { expression } => Some(*expression),
        }
    }
}

/// One source-ordered JSX child.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsxChild {
    Text(Box<str>),

    Expression { expression: RegionId },

    Spread { expression: RegionId },

    Element { expression: RegionId },

    Fragment { expression: RegionId },
}

impl JsxChild {
    const fn expression(&self) -> Option<RegionId> {
        match self {
            Self::Text(_) => None,

            Self::Expression { expression }
            | Self::Spread { expression }
            | Self::Element { expression }
            | Self::Fragment { expression } => Some(*expression),
        }
    }
}

/// Produces one JSX element while retaining its source-level structure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JsxElementOp {
    name: JsxElementName,
    attributes: Box<[JsxAttribute]>,
    children: Box<[JsxChild]>,
}

impl JsxElementOp {
    pub fn new(
        name: JsxElementName,
        attributes: impl Into<Box<[JsxAttribute]>>,
        children: impl Into<Box<[JsxChild]>>,
    ) -> Self {
        name.validate();

        Self {
            name,
            attributes: attributes.into(),
            children: children.into(),
        }
    }

    pub const fn name(&self) -> &JsxElementName {
        &self.name
    }

    pub fn attributes(&self) -> &[JsxAttribute] {
        &self.attributes
    }

    pub fn children(&self) -> &[JsxChild] {
        &self.children
    }

    /// Returns the operand containing the referenced tag base, if present.
    pub const fn reference_operand_index(&self) -> Option<usize> {
        if self.name.operand_count() == 1 {
            Some(0)
        } else {
            None
        }
    }

    /// Returns embedded expressions in JSX evaluation order.
    pub fn regions(&self) -> Vec<RegionId> {
        self.attributes
            .iter()
            .filter_map(JsxAttribute::expression)
            .chain(self.children.iter().filter_map(JsxChild::expression))
            .collect()
    }

    /// JSX runtime behavior is framework-defined and may execute arbitrary code.
    pub const fn effects(&self) -> OperationEffects {
        OperationEffects::MAY_THROW_AND_OBSERVABLE
    }

    pub(crate) const fn operand_count(&self) -> usize {
        self.name.operand_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

/// Produces one JSX fragment while retaining its children.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JsxFragmentOp {
    children: Box<[JsxChild]>,
}

impl JsxFragmentOp {
    pub fn new(children: impl Into<Box<[JsxChild]>>) -> Self {
        Self {
            children: children.into(),
        }
    }

    pub fn children(&self) -> &[JsxChild] {
        &self.children
    }

    pub fn regions(&self) -> Vec<RegionId> {
        self.children
            .iter()
            .filter_map(JsxChild::expression)
            .collect()
    }

    pub const fn effects(&self) -> OperationEffects {
        OperationEffects::MAY_THROW_AND_OBSERVABLE
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
    use crate::{JsString, RegionId};

    use super::{
        JsxAttribute, JsxAttributeName, JsxAttributeValue, JsxChild, JsxElementName, JsxElementOp,
        JsxFragmentOp, JsxMemberBase,
    };

    #[test]
    fn preserves_element_structure_and_expression_order() {
        let spread = RegionId::from_index(1);
        let attribute = RegionId::from_index(2);
        let child = RegionId::from_index(3);
        let element = JsxElementOp::new(
            JsxElementName::Member {
                base: JsxMemberBase::Reference,
                properties: vec!["Button".into()].into_boxed_slice(),
            },
            [
                JsxAttribute::Named {
                    name: JsxAttributeName::Identifier("label".into()),
                    value: Some(JsxAttributeValue::String(JsString::new("Save", false))),
                },
                JsxAttribute::Spread { expression: spread },
                JsxAttribute::Named {
                    name: JsxAttributeName::Identifier("value".into()),
                    value: Some(JsxAttributeValue::Expression {
                        expression: attribute,
                    }),
                },
            ],
            [JsxChild::Element { expression: child }],
        );

        assert_eq!(element.reference_operand_index(), Some(0));
        assert_eq!(element.operand_count(), 1);
        assert_eq!(element.result_count(), 1);
        assert_eq!(element.regions(), [spread, attribute, child]);
        assert!(element.effects().may_throw());
        assert!(element.effects().may_have_observable_effects());
    }

    #[test]
    fn preserves_fragment_child_order() {
        let first = RegionId::from_index(1);
        let second = RegionId::from_index(2);
        let fragment = JsxFragmentOp::new([
            JsxChild::Expression { expression: first },
            JsxChild::Text("between".into()),
            JsxChild::Fragment { expression: second },
        ]);

        assert_eq!(fragment.regions(), [first, second]);
        assert_eq!(fragment.operand_count(), 0);
        assert_eq!(fragment.result_count(), 1);
        assert!(fragment.effects().may_throw());
        assert!(fragment.effects().may_have_observable_effects());
    }
}
