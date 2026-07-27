//! JavaScript meta-property lowering.

use evrel_ir::{MetaPropertyKind, MetaPropertyOp, OperationKind, ValueId};
use oxc_ast::ast::MetaProperty;

use crate::lower::FunctionLowerer;

/// Lowers an execution-context meta-property.
pub(super) fn lower_meta_property(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    property: &MetaProperty<'_>,
) -> ValueId {
    let kind = match (property.meta.name.as_str(), property.property.name.as_str()) {
        ("import", "meta") => MetaPropertyKind::ImportMeta,
        ("new", "target") => MetaPropertyKind::NewTarget,
        _ => unreachable!("Oxc produced an unknown JavaScript meta-property"),
    };

    lowerer.emit_value(OperationKind::MetaProperty(MetaPropertyOp::new(kind)), [])
}
