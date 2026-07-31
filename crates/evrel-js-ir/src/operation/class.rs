//! Structured JavaScript class creation.

use crate::{BindingId, FunctionId, PrivateNameId, RegionId};

use super::OperationEffects;

/// A class element's property key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClassElementKey {
    Static(Box<str>),
    Computed(RegionId),
    Private(PrivateNameId),
}

/// The semantic form of a class method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassMethodKind {
    Constructor,
    Method,
    Getter,
    Setter,
}

/// Where a class method is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassMethodPlacement {
    Prototype,
    Static,
}

/// Where a class field is initialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassFieldPlacement {
    Instance,
    Static,
}

/// A method-like class element.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassMethod {
    kind: ClassMethodKind,
    placement: ClassMethodPlacement,
    key: ClassElementKey,
    function: FunctionId,
}

impl ClassMethod {
    pub fn new(
        kind: ClassMethodKind,
        placement: ClassMethodPlacement,
        key: ClassElementKey,
        function: FunctionId,
    ) -> Self {
        if kind == ClassMethodKind::Constructor {
            assert_eq!(
                placement,
                ClassMethodPlacement::Prototype,
                "a class constructor cannot be static"
            );
            assert!(
                matches!(&key, ClassElementKey::Static(name) if name.as_ref() == "constructor"),
                "a class constructor must have the canonical constructor key"
            );
        }

        Self {
            kind,
            placement,
            key,
            function,
        }
    }

    pub const fn kind(&self) -> ClassMethodKind {
        self.kind
    }

    pub const fn placement(&self) -> ClassMethodPlacement {
        self.placement
    }

    pub const fn key(&self) -> &ClassElementKey {
        &self.key
    }

    pub const fn function(&self) -> FunctionId {
        self.function
    }
}

/// A class field.
///
/// Its initializer is a separately owned function body. Instance initializers
/// are deferred until construction; static initializers run during class creation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassField {
    placement: ClassFieldPlacement,
    key: ClassElementKey,
    initializer: Option<FunctionId>,
}

impl ClassField {
    pub const fn new(
        placement: ClassFieldPlacement,
        key: ClassElementKey,
        initializer: Option<FunctionId>,
    ) -> Self {
        Self {
            placement,
            key,
            initializer,
        }
    }

    pub const fn placement(&self) -> ClassFieldPlacement {
        self.placement
    }

    pub const fn key(&self) -> &ClassElementKey {
        &self.key
    }

    pub const fn initializer(&self) -> Option<FunctionId> {
        self.initializer
    }
}

/// A class static-initialization block.
///
/// The referenced function owns the block's bindings and control-flow graph,
/// but is not callable. It executes synchronously as part of class creation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassStaticBlock {
    body: FunctionId,
}

impl ClassStaticBlock {
    pub const fn new(body: FunctionId) -> Self {
        Self { body }
    }

    pub const fn body(&self) -> FunctionId {
        self.body
    }
}

/// One source-ordered class body element.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClassElement {
    Method(ClassMethod),
    Field(ClassField),
    StaticBlock(ClassStaticBlock),
}

impl ClassElement {
    fn computed_key(&self) -> Option<RegionId> {
        let key = match self {
            Self::Method(method) => method.key(),
            Self::Field(field) => field.key(),
            Self::StaticBlock(_) => return None,
        };

        match key {
            ClassElementKey::Computed(region) => Some(*region),
            ClassElementKey::Static(_) | ClassElementKey::Private(_) => None,
        }
    }

    fn referenced_function(&self) -> Option<FunctionId> {
        match self {
            Self::Method(method) => Some(method.function()),
            Self::Field(field) => field.initializer(),
            Self::StaticBlock(block) => Some(block.body()),
        }
    }

    fn has_immediate_static_initialization(&self) -> bool {
        matches!(
            self,
            Self::Field(field)
                if field.placement() == ClassFieldPlacement::Static
                    && field.initializer().is_some()
        ) || matches!(self, Self::StaticBlock(_))
    }
}

/// Creates one JavaScript class value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateClassOp {
    self_binding: Option<BindingId>,
    super_class: Option<RegionId>,
    elements: Box<[ClassElement]>,
}

impl CreateClassOp {
    pub fn new(
        self_binding: Option<BindingId>,
        super_class: Option<RegionId>,
        elements: impl Into<Box<[ClassElement]>>,
    ) -> Self {
        Self {
            self_binding,
            super_class,
            elements: elements.into(),
        }
    }

    pub const fn self_binding(&self) -> Option<BindingId> {
        self.self_binding
    }

    pub const fn super_class(&self) -> Option<RegionId> {
        self.super_class
    }

    pub fn elements(&self) -> &[ClassElement] {
        &self.elements
    }

    /// Returns immediately evaluated regions in semantic order.
    pub fn regions(&self) -> Vec<RegionId> {
        self.super_class
            .into_iter()
            .chain(self.elements.iter().filter_map(ClassElement::computed_key))
            .collect()
    }

    /// Returns every module-owned class-element body in source order.
    pub fn referenced_functions(&self) -> Vec<FunctionId> {
        self.elements
            .iter()
            .filter_map(ClassElement::referenced_function)
            .collect()
    }

    pub fn effects(&self) -> OperationEffects {
        if self
            .elements
            .iter()
            .any(ClassElement::has_immediate_static_initialization)
        {
            OperationEffects::MAY_THROW_AND_OBSERVABLE
        } else {
            OperationEffects::MAY_THROW
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

    use super::{
        ClassElement, ClassElementKey, ClassField, ClassFieldPlacement, ClassMethod,
        ClassMethodKind, ClassMethodPlacement, ClassStaticBlock, CreateClassOp,
    };

    #[test]
    fn preserves_class_evaluation_and_referenced_body_order() {
        let super_class = RegionId::from_index(1);
        let computed_key = RegionId::from_index(2);
        let method_function = FunctionId::from_index(1);
        let field_initializer = FunctionId::from_index(2);
        let static_block = FunctionId::from_index(3);
        let operation = CreateClassOp::new(
            None,
            Some(super_class),
            [
                ClassElement::Method(ClassMethod::new(
                    ClassMethodKind::Method,
                    ClassMethodPlacement::Prototype,
                    ClassElementKey::Computed(computed_key),
                    method_function,
                )),
                ClassElement::Field(ClassField::new(
                    ClassFieldPlacement::Instance,
                    ClassElementKey::Static("value".into()),
                    Some(field_initializer),
                )),
                ClassElement::StaticBlock(ClassStaticBlock::new(static_block)),
            ],
        );

        assert_eq!(operation.regions(), [super_class, computed_key]);
        assert_eq!(
            operation.referenced_functions(),
            [method_function, field_initializer, static_block]
        );
        assert_eq!(operation.operand_count(), 0);
        assert_eq!(operation.result_count(), 1);
        assert!(operation.effects().may_throw());
        assert!(operation.effects().may_have_observable_effects());
    }

    #[test]
    fn class_without_immediate_static_initialization_is_not_intrinsically_observable() {
        let operation = CreateClassOp::new(
            None,
            None,
            [ClassElement::Field(ClassField::new(
                ClassFieldPlacement::Instance,
                ClassElementKey::Static("value".into()),
                Some(FunctionId::from_index(1)),
            ))],
        );

        assert!(operation.effects().may_throw());
        assert!(!operation.effects().may_have_observable_effects());
    }
}
