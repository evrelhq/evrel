//! Compiler-owned JavaScript binding patterns.

use crate::{BindingId, PrivateNameId, RegionId};

/// An expression evaluated by a pattern at a spec-defined sequence point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PatternExpression {
    region: RegionId,
}

impl PatternExpression {
    /// Creates a deferred pattern expression backed by an inline region.
    pub const fn new(region: RegionId) -> Self {
        Self { region }
    }

    /// Returns the inline region that evaluates this expression.
    pub const fn region(self) -> RegionId {
        self.region
    }
}

/// A destination written by a destructuring assignment.
///
/// Expressions needed to construct property references remain deferred until
/// the pattern reaches this target.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssignmentTarget {
    Binding {
        binding: BindingId,
    },

    Global {
        name: Box<str>,
    },

    StaticProperty {
        object: PatternExpression,
        name: Box<str>,
    },

    ComputedProperty {
        object: PatternExpression,
        key: PatternExpression,
    },

    PrivateProperty {
        object: PatternExpression,
        private_name: PrivateNameId,
    },

    StaticSuperProperty {
        name: Box<str>,
    },

    ComputedSuperProperty {
        key: PatternExpression,
    },
}

impl AssignmentTarget {
    /// Returns deferred expression regions in evaluation order.
    pub fn regions(&self) -> Vec<RegionId> {
        match self {
            Self::Binding { .. } | Self::Global { .. } | Self::StaticSuperProperty { .. } => {
                Vec::new()
            }

            Self::StaticProperty { object, .. } | Self::PrivateProperty { object, .. } => {
                vec![object.region()]
            }

            Self::ComputedProperty { object, key } => {
                vec![object.region(), key.region()]
            }

            Self::ComputedSuperProperty { key } => {
                vec![key.region()]
            }
        }
    }

    fn visit_binding_ids(&self, visit: &mut impl FnMut(BindingId)) {
        if let Self::Binding { binding } = self {
            visit(*binding);
        }
    }
}

/// One property in an object assignment pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObjectAssignmentProperty {
    Static {
        name: Box<str>,
        target: AssignmentPattern,
    },

    Computed {
        key: PatternExpression,
        target: AssignmentPattern,
    },
}

impl ObjectAssignmentProperty {
    /// Creates a statically named object assignment property.
    pub fn static_property(name: impl Into<Box<str>>, target: AssignmentPattern) -> Self {
        Self::Static {
            name: name.into(),
            target,
        }
    }

    /// Creates an object assignment property with a deferred computed key.
    pub const fn computed_property(key: PatternExpression, target: AssignmentPattern) -> Self {
        Self::Computed { key, target }
    }

    /// Returns the pattern receiving this property's value.
    pub const fn target(&self) -> &AssignmentPattern {
        match self {
            Self::Static { target, .. } | Self::Computed { target, .. } => target,
        }
    }

    fn visit_regions(&self, visit: &mut impl FnMut(RegionId)) {
        match self {
            Self::Static { target, .. } => target.visit_regions(visit),

            Self::Computed { key, target } => {
                visit(key.region());
                target.visit_regions(visit);
            }
        }
    }

    fn visit_binding_ids(&self, visit: &mut impl FnMut(BindingId)) {
        self.target().visit_binding_ids(visit);
    }
}

/// A recursive JavaScript destructuring-assignment pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssignmentPattern {
    /// A single assignment destination.
    Target { target: AssignmentTarget },

    /// An array assignment pattern.
    Array {
        /// Pattern elements in source order.
        ///
        /// `None` represents an elision such as the first position in `[, x]`.
        elements: Box<[Option<AssignmentPattern>]>,

        /// The optional final pattern receiving remaining iterator values.
        rest: Option<Box<AssignmentPattern>>,
    },

    /// An object assignment pattern.
    Object {
        /// Properties evaluated in source order.
        properties: Box<[ObjectAssignmentProperty]>,

        /// The optional final pattern receiving remaining own properties.
        rest: Option<Box<AssignmentPattern>>,
    },

    /// A pattern whose initializer runs only when its input is `undefined`.
    Default {
        target: Box<AssignmentPattern>,
        initializer: PatternExpression,
    },
}

impl AssignmentPattern {
    /// Creates a pattern that writes one assignment target.
    pub const fn target(target: AssignmentTarget) -> Self {
        Self::Target { target }
    }

    /// Creates an array assignment pattern.
    pub fn array(elements: impl Into<Box<[Option<Self>]>>, rest: Option<Self>) -> Self {
        Self::Array {
            elements: elements.into(),
            rest: rest.map(Box::new),
        }
    }

    /// Creates an object assignment pattern.
    pub fn object(
        properties: impl Into<Box<[ObjectAssignmentProperty]>>,
        rest: Option<Self>,
    ) -> Self {
        Self::Object {
            properties: properties.into(),
            rest: rest.map(Box::new),
        }
    }

    /// Creates a pattern with a lazily evaluated default initializer.
    pub fn default(target: Self, initializer: PatternExpression) -> Self {
        Self::Default {
            target: Box::new(target),
            initializer,
        }
    }

    /// Returns the destination when this is a simple assignment target.
    pub const fn as_target(&self) -> Option<&AssignmentTarget> {
        match self {
            Self::Target { target } => Some(target),
            Self::Array { .. } | Self::Object { .. } | Self::Default { .. } => None,
        }
    }

    /// Returns deferred expression regions in semantic evaluation order.
    pub fn regions(&self) -> Vec<RegionId> {
        let mut regions = Vec::new();
        self.visit_regions(&mut |region| regions.push(region));

        regions
    }

    /// Returns assignment-target bindings in evaluation order.
    pub fn binding_ids(&self) -> Vec<BindingId> {
        let mut bindings = Vec::new();
        self.visit_binding_ids(&mut |binding| bindings.push(binding));

        bindings
    }

    pub(crate) fn visit_binding_ids(&self, visit: &mut impl FnMut(BindingId)) {
        match self {
            Self::Target { target } => target.visit_binding_ids(visit),

            Self::Array { elements, rest } => {
                for element in elements.iter().flatten() {
                    element.visit_binding_ids(visit);
                }

                if let Some(rest) = rest {
                    rest.visit_binding_ids(visit);
                }
            }

            Self::Object { properties, rest } => {
                for property in properties {
                    property.visit_binding_ids(visit);
                }

                if let Some(rest) = rest {
                    rest.visit_binding_ids(visit);
                }
            }

            Self::Default { target, .. } => target.visit_binding_ids(visit),
        }
    }

    fn visit_regions(&self, visit: &mut impl FnMut(RegionId)) {
        match self {
            Self::Target { target } => {
                for region in target.regions() {
                    visit(region);
                }
            }

            Self::Array { elements, rest } => {
                for element in elements.iter().flatten() {
                    element.visit_regions(visit);
                }

                if let Some(rest) = rest {
                    rest.visit_regions(visit);
                }
            }

            Self::Object { properties, rest } => {
                for property in properties {
                    property.visit_regions(visit);
                }

                if let Some(rest) = rest {
                    rest.visit_regions(visit);
                }
            }

            Self::Default {
                target,
                initializer,
            } => {
                if target.as_target().is_some() {
                    target.visit_regions(visit);
                    visit(initializer.region());
                } else {
                    visit(initializer.region());
                    target.visit_regions(visit);
                }
            }
        }
    }
}

/// One property in an object binding pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObjectBindingProperty {
    /// A property whose key is known during lowering.
    Static {
        name: Box<str>,
        target: BindingPattern,
    },

    /// A property whose key expression executes and is converted to a property
    /// key when this property is reached.
    Computed {
        key: PatternExpression,
        target: BindingPattern,
    },
}

impl ObjectBindingProperty {
    /// Creates a statically named object binding property.
    pub fn static_property(name: impl Into<Box<str>>, target: BindingPattern) -> Self {
        Self::Static {
            name: name.into(),
            target,
        }
    }

    /// Creates a computed object binding property.
    pub const fn computed_property(key: PatternExpression, target: BindingPattern) -> Self {
        Self::Computed { key, target }
    }

    /// Returns the pattern receiving this property's value.
    pub const fn target(&self) -> &BindingPattern {
        match self {
            Self::Static { target, .. } | Self::Computed { target, .. } => target,
        }
    }

    pub(crate) fn visit_regions(&self, visit: &mut impl FnMut(RegionId)) {
        match self {
            Self::Static { target, .. } => target.visit_regions(visit),
            Self::Computed { key, target } => {
                visit(key.region());
                target.visit_regions(visit);
            }
        }
    }
}

/// A target initialized by a declaration or function parameter.
///
/// Additional variants will represent array, object, default, and rest
/// patterns without retaining references to the Oxc AST.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BindingPattern {
    /// A single declared binding.
    Binding {
        /// The binding initialized by the pattern.
        binding: BindingId,
    },

    /// An array binding pattern.
    Array {
        /// Pattern elements in source order.
        ///
        /// `None` represents an elision such as the first position in `[, x]`.
        elements: Box<[Option<BindingPattern>]>,

        /// The optional final pattern that receives remaining iterator values.
        rest: Option<Box<BindingPattern>>,
    },

    /// An object binding pattern.
    Object {
        /// Properties evaluated in source order.
        properties: Box<[ObjectBindingProperty]>,

        /// The optional final pattern receiving remaining own properties.
        rest: Option<Box<BindingPattern>>,
    },

    /// A pattern whose initializer runs only when its input is `undefined`.
    Default {
        target: Box<BindingPattern>,
        initializer: PatternExpression,
    },
}

impl BindingPattern {
    /// Creates a pattern that initializes one binding.
    pub const fn binding(binding: BindingId) -> Self {
        Self::Binding { binding }
    }

    /// Creates an array binding pattern.
    pub fn array(elements: impl Into<Box<[Option<Self>]>>, rest: Option<Self>) -> Self {
        Self::Array {
            elements: elements.into(),
            rest: rest.map(Box::new),
        }
    }

    /// Creates an object binding pattern.
    pub fn object(properties: impl Into<Box<[ObjectBindingProperty]>>, rest: Option<Self>) -> Self {
        Self::Object {
            properties: properties.into(),
            rest: rest.map(Box::new),
        }
    }

    /// Creates a pattern with a lazily evaluated default initializer.
    pub fn default(target: Self, initializer: PatternExpression) -> Self {
        Self::Default {
            target: Box::new(target),
            initializer,
        }
    }

    /// Returns the binding when this is a simple binding pattern.
    pub const fn as_binding(&self) -> Option<BindingId> {
        match self {
            Self::Binding { binding } => Some(*binding),
            Self::Array { .. } | Self::Object { .. } | Self::Default { .. } => None,
        }
    }

    /// Returns bindings in ECMAScript `BoundNames` order.
    pub fn binding_ids(&self) -> Vec<BindingId> {
        let mut bindings = Vec::new();
        self.visit_binding_ids(&mut |binding| bindings.push(binding));

        bindings
    }

    pub(crate) fn visit_binding_ids(&self, visit: &mut impl FnMut(BindingId)) {
        match self {
            Self::Binding { binding } => visit(*binding),

            Self::Array { elements, rest } => {
                for element in elements.iter().flatten() {
                    element.visit_binding_ids(visit);
                }

                if let Some(rest) = rest {
                    rest.visit_binding_ids(visit);
                }
            }

            Self::Object { properties, rest } => {
                for property in properties {
                    property.target().visit_binding_ids(visit);
                }

                if let Some(rest) = rest {
                    rest.visit_binding_ids(visit);
                }
            }

            Self::Default { target, .. } => target.visit_binding_ids(visit),
        }
    }

    /// Returns deferred expression regions in semantic pattern order.
    pub fn regions(&self) -> Vec<RegionId> {
        let mut regions = Vec::new();
        self.visit_regions(&mut |region| regions.push(region));

        regions
    }

    pub(crate) fn visit_regions(&self, visit: &mut impl FnMut(RegionId)) {
        match self {
            Self::Binding { .. } => {}

            Self::Array { elements, rest } => {
                for element in elements.iter().flatten() {
                    element.visit_regions(visit);
                }

                if let Some(rest) = rest {
                    rest.visit_regions(visit);
                }
            }

            Self::Object { properties, rest } => {
                for property in properties {
                    property.visit_regions(visit);
                }

                if let Some(rest) = rest {
                    rest.visit_regions(visit);
                }
            }

            Self::Default {
                target,
                initializer,
            } => {
                visit(initializer.region());
                target.visit_regions(visit);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{BindingId, RegionId};

    use super::{
        AssignmentPattern, AssignmentTarget, BindingPattern, ObjectAssignmentProperty,
        ObjectBindingProperty, PatternExpression,
    };

    #[test]
    fn collects_assignment_target_regions_in_reference_order() {
        let object = RegionId::from_index(1);
        let key = RegionId::from_index(2);
        let target = AssignmentTarget::ComputedProperty {
            object: PatternExpression::new(object),
            key: PatternExpression::new(key),
        };

        assert_eq!(target.regions(), [object, key]);
        assert!(
            AssignmentTarget::Global {
                name: "target".into(),
            }
            .regions()
            .is_empty()
        );
    }

    #[test]
    fn evaluates_a_leaf_target_before_its_default_initializer() {
        let object = RegionId::from_index(1);
        let initializer = RegionId::from_index(2);
        let pattern = AssignmentPattern::default(
            AssignmentPattern::target(AssignmentTarget::StaticProperty {
                object: PatternExpression::new(object),
                name: "value".into(),
            }),
            PatternExpression::new(initializer),
        );

        assert_eq!(pattern.regions(), [object, initializer]);
    }

    #[test]
    fn evaluates_a_default_before_entering_its_nested_pattern() {
        let initializer = RegionId::from_index(1);
        let object = RegionId::from_index(2);
        let pattern = AssignmentPattern::default(
            AssignmentPattern::array(
                [Some(AssignmentPattern::target(
                    AssignmentTarget::StaticProperty {
                        object: PatternExpression::new(object),
                        name: "value".into(),
                    },
                ))],
                None,
            ),
            PatternExpression::new(initializer),
        );

        assert_eq!(pattern.regions(), [initializer, object]);
    }

    #[test]
    fn evaluates_an_object_key_before_its_assignment_target() {
        let key = RegionId::from_index(1);
        let object = RegionId::from_index(2);
        let pattern = AssignmentPattern::object(
            [ObjectAssignmentProperty::computed_property(
                PatternExpression::new(key),
                AssignmentPattern::target(AssignmentTarget::StaticProperty {
                    object: PatternExpression::new(object),
                    name: "value".into(),
                }),
            )],
            None,
        );

        assert_eq!(pattern.regions(), [key, object]);
    }

    #[test]
    fn collects_array_bindings_in_source_order() {
        let first = BindingId::from_index(1);
        let second = BindingId::from_index(2);

        let rest = BindingId::from_index(3);
        let pattern = BindingPattern::array(
            [
                Some(BindingPattern::binding(first)),
                None,
                Some(BindingPattern::binding(second)),
            ],
            Some(BindingPattern::binding(rest)),
        );

        assert_eq!(pattern.binding_ids(), [first, second, rest]);
    }

    #[test]
    fn collects_object_bindings_in_source_order() {
        let first = BindingId::from_index(1);
        let renamed = BindingId::from_index(2);
        let rest = BindingId::from_index(3);

        let pattern = BindingPattern::object(
            [
                ObjectBindingProperty::static_property("first", BindingPattern::binding(first)),
                ObjectBindingProperty::static_property("source", BindingPattern::binding(renamed)),
            ],
            Some(BindingPattern::binding(rest)),
        );

        assert_eq!(pattern.binding_ids(), [first, renamed, rest]);
    }

    #[test]
    fn collects_pattern_regions_in_evaluation_order() {
        let key = RegionId::from_index(1);
        let initializer = RegionId::from_index(2);
        let binding = BindingId::from_index(1);
        let pattern = BindingPattern::object(
            [ObjectBindingProperty::computed_property(
                PatternExpression::new(key),
                BindingPattern::default(
                    BindingPattern::binding(binding),
                    PatternExpression::new(initializer),
                ),
            )],
            None,
        );

        assert_eq!(pattern.regions(), [key, initializer]);
    }
}
