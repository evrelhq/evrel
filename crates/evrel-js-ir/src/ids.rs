//! Typed identifiers for entities stored in the IR.

use crate::arena::ArenaId;

/// Identifies one internal module within a program IR.
///
/// A `ModuleId` is only valid in the program that allocated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleId(u32);

impl ModuleId {
    /// Creates an ID from an arena index.
    pub(crate) fn from_index(index: usize) -> Self {
        let index =
            u32::try_from(index).expect("a program cannot contain more than u32::MAX modules");

        Self(index)
    }

    /// Returns the corresponding arena index.
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for ModuleId {
    fn from_index(index: usize) -> Self {
        ModuleId::from_index(index)
    }

    fn index(self) -> usize {
        ModuleId::index(self)
    }
}

/// Identifies one source file within a compiler source database.
///
/// A source file ID is only valid in the database that allocated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceFileId(u32);

impl SourceFileId {
    pub(crate) fn from_index(index: usize) -> Self {
        let index = u32::try_from(index)
            .expect("a source database cannot contain more than u32::MAX files");

        Self(index)
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for SourceFileId {
    fn from_index(index: usize) -> Self {
        SourceFileId::from_index(index)
    }

    fn index(self) -> usize {
        SourceFileId::index(self)
    }
}

/// Identifies one canonical compiler location.
///
/// A location ID is only valid in the source database that allocated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocationId(u32);

impl LocationId {
    /// Canonical location for IR without known source provenance.
    pub const UNKNOWN: Self = Self(0);

    pub(crate) fn from_index(index: usize) -> Self {
        let index = u32::try_from(index)
            .expect("a source database cannot contain more than u32::MAX locations");

        Self(index)
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for LocationId {
    fn from_index(index: usize) -> Self {
        LocationId::from_index(index)
    }

    fn index(self) -> usize {
        LocationId::index(self)
    }
}

/// Identifies a JavaScript binding within a module's IR.
///
/// A `BindingId` is only valid in the module that allocated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingId(u32);

impl BindingId {
    /// Creates an ID from an arena index.
    pub(crate) fn from_index(index: usize) -> Self {
        let index =
            u32::try_from(index).expect("a module cannot contain more than u32::MAX bindings");

        Self(index)
    }

    /// Returns the corresponding arena index.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for BindingId {
    fn from_index(index: usize) -> Self {
        BindingId::from_index(index)
    }

    fn index(self) -> usize {
        BindingId::index(self)
    }
}

/// Identifies one JavaScript binding within a whole program.
///
/// A [`BindingId`] is only unique within its owning module. Qualifying it with
/// a [`ModuleId`] makes it safe to use in program-wide analyses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramBindingId {
    module: ModuleId,
    binding: BindingId,
}

impl ProgramBindingId {
    /// Qualifies a module-local binding ID.
    pub const fn new(module: ModuleId, binding: BindingId) -> Self {
        Self { module, binding }
    }

    /// Returns the module that owns the binding.
    pub const fn module(self) -> ModuleId {
        self.module
    }

    /// Returns the module-local binding ID.
    pub const fn binding(self) -> BindingId {
        self.binding
    }
}

/// Identifies one lexically scoped JavaScript private name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrivateNameId(u32);

impl PrivateNameId {
    pub(crate) fn from_index(index: usize) -> Self {
        let index =
            u32::try_from(index).expect("a module cannot contain more than u32::MAX private names");

        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for PrivateNameId {
    fn from_index(index: usize) -> Self {
        PrivateNameId::from_index(index)
    }

    fn index(self) -> usize {
        PrivateNameId::index(self)
    }
}

/// Identifies one syntactic tagged-template site within a module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TemplateSiteId(u32);

impl TemplateSiteId {
    pub(crate) fn from_index(index: usize) -> Self {
        let index = u32::try_from(index)
            .expect("a module cannot contain more than u32::MAX tagged-template sites");

        Self(index)
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Identifies a function within a module's IR.
///
/// A `FunctionId` is only valid in the module that allocated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunctionId(u32);

impl FunctionId {
    /// Creates an ID from an arena index.
    pub(crate) fn from_index(index: usize) -> Self {
        let index =
            u32::try_from(index).expect("a module cannot contain more than u32::MAX functions");

        Self(index)
    }

    /// Returns the corresponding arena index.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for FunctionId {
    fn from_index(index: usize) -> Self {
        FunctionId::from_index(index)
    }

    fn index(self) -> usize {
        FunctionId::index(self)
    }
}

/// Identifies one function within a whole program.
///
/// A [`FunctionId`] is only unique within its owning module. Qualifying it
/// with a [`ModuleId`] makes it safe to use in program-wide analyses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramFunctionId {
    module: ModuleId,
    function: FunctionId,
}

impl ProgramFunctionId {
    /// Qualifies a module-local function ID.
    pub const fn new(module: ModuleId, function: FunctionId) -> Self {
        Self { module, function }
    }

    /// Returns the module that owns the function.
    pub const fn module(self) -> ModuleId {
        self.module
    }

    /// Returns the module-local function ID.
    pub const fn function(self) -> FunctionId {
        self.function
    }
}

/// Identifies a basic block within a function's IR.
///
/// A `BlockId` is only valid in the function that allocated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(u32);

impl BlockId {
    /// Creates an ID from an arena index.
    pub(crate) fn from_index(index: usize) -> Self {
        let index =
            u32::try_from(index).expect("a function cannot contain more than u32::MAX blocks");

        Self(index)
    }

    /// Returns the corresponding arena index.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for BlockId {
    fn from_index(index: usize) -> Self {
        BlockId::from_index(index)
    }

    fn index(self) -> usize {
        BlockId::index(self)
    }
}

/// Identifies an exception handler within a function.
///
/// An `ExceptionHandlerId` is only valid in the function that allocated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExceptionHandlerId(u32);

impl ExceptionHandlerId {
    /// Creates an ID from an arena index.
    pub(crate) fn from_index(index: usize) -> Self {
        let index = u32::try_from(index)
            .expect("a function cannot contain more than u32::MAX exception handlers");

        Self(index)
    }

    /// Returns the corresponding arena index.
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for ExceptionHandlerId {
    fn from_index(index: usize) -> Self {
        ExceptionHandlerId::from_index(index)
    }

    fn index(self) -> usize {
        ExceptionHandlerId::index(self)
    }
}

/// Identifies source-structured labeled-statement metadata within a function.
///
/// A `LabeledStatementId` is only valid in the function that allocated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LabeledStatementId(u32);

impl LabeledStatementId {
    /// Creates an ID from an arena index.
    pub(crate) fn from_index(index: usize) -> Self {
        let index = u32::try_from(index)
            .expect("a function cannot contain more than u32::MAX labeled statements");

        Self(index)
    }

    /// Returns the corresponding arena index.
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for LabeledStatementId {
    fn from_index(index: usize) -> Self {
        LabeledStatementId::from_index(index)
    }

    fn index(self) -> usize {
        LabeledStatementId::index(self)
    }
}

/// Identifies an inline executable region within a function.
///
/// A `RegionId` is only valid in the function that allocated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionId(u32);

impl RegionId {
    /// Creates an ID from an arena index.
    pub(crate) fn from_index(index: usize) -> Self {
        let index =
            u32::try_from(index).expect("a function cannot contain more than u32::MAX regions");

        Self(index)
    }

    /// Returns the corresponding arena index.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for RegionId {
    fn from_index(index: usize) -> Self {
        RegionId::from_index(index)
    }

    fn index(self) -> usize {
        RegionId::index(self)
    }
}

/// Identifies an operation within a function's IR.
///
/// An `OperationId` is only valid in the function that allocated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OperationId(u32);

impl OperationId {
    /// Creates an ID from an arena index.
    pub(crate) fn from_index(index: usize) -> Self {
        let index =
            u32::try_from(index).expect("a function cannot contain more than u32::MAX operations");

        Self(index)
    }

    /// Returns the corresponding arena index.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for OperationId {
    fn from_index(index: usize) -> Self {
        OperationId::from_index(index)
    }

    fn index(self) -> usize {
        OperationId::index(self)
    }
}

/// Identifies a value within a function's IR.
///
/// A `ValueId` is only valid in the function that allocated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueId(u32);

impl ValueId {
    /// Creates an ID from an arena index.
    pub(crate) fn from_index(index: usize) -> Self {
        let index =
            u32::try_from(index).expect("a function cannot contain more than u32::MAX values");

        Self(index)
    }

    /// Returns the corresponding arena index.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl ArenaId for ValueId {
    fn from_index(index: usize) -> Self {
        ValueId::from_index(index)
    }

    fn index(self) -> usize {
        ValueId::index(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BindingId, BlockId, ExceptionHandlerId, FunctionId, ModuleId, OperationId,
        ProgramBindingId, ProgramFunctionId, RegionId, TemplateSiteId, ValueId,
    };

    #[test]
    fn qualifies_binding_ids_by_module() {
        let binding = BindingId::from_index(0);
        let first_module = ModuleId::from_index(0);
        let second_module = ModuleId::from_index(1);

        let first = ProgramBindingId::new(first_module, binding);
        let second = ProgramBindingId::new(second_module, binding);

        assert_ne!(first, second);
        assert_eq!(first.module(), first_module);
        assert_eq!(first.binding(), binding);
    }

    #[test]
    fn qualifies_function_ids_by_module() {
        let function = FunctionId::from_index(0);
        let first_module = ModuleId::from_index(0);
        let second_module = ModuleId::from_index(1);

        let first = ProgramFunctionId::new(first_module, function);
        let second = ProgramFunctionId::new(second_module, function);

        assert_ne!(first, second);
        assert_eq!(first.module(), first_module);
        assert_eq!(first.function(), function);
    }

    #[test]
    fn converts_binding_ids_to_and_from_an_index() {
        let id = BindingId::from_index(42);

        assert_eq!(id.index(), 42);
    }

    #[test]
    fn converts_function_ids_to_and_from_an_index() {
        let id = FunctionId::from_index(42);

        assert_eq!(id.index(), 42);
    }

    #[test]
    fn converts_template_site_ids_to_and_from_an_index() {
        let id = TemplateSiteId::from_index(17);

        assert_eq!(id.index(), 17);
    }

    #[test]
    fn converts_block_ids_to_and_from_an_index() {
        let id = BlockId::from_index(42);

        assert_eq!(id.index(), 42);
    }

    #[test]
    fn converts_exception_handler_ids_to_and_from_an_index() {
        let id = ExceptionHandlerId::from_index(42);

        assert_eq!(id.index(), 42);
    }

    #[test]
    fn converts_region_ids_to_and_from_an_index() {
        let id = RegionId::from_index(42);

        assert_eq!(id.index(), 42);
    }

    #[test]
    fn converts_operation_ids_to_and_from_an_index() {
        let id = OperationId::from_index(42);

        assert_eq!(id.index(), 42);
    }

    #[test]
    fn converts_value_ids_to_and_from_an_index() {
        let id = ValueId::from_index(42);

        assert_eq!(id.index(), 42);
    }
}
