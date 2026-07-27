//! Conversion of source-level module attributes into Evrel data.

use evrel_ir::{ModuleAttribute, ModuleExportName};
use oxc_ast::ast::{ImportAttributeKey, WithClause};

pub(crate) fn lower_module_attributes(clause: Option<&WithClause<'_>>) -> Vec<ModuleAttribute> {
    clause
        .into_iter()
        .flat_map(|clause| &clause.with_entries)
        .map(|attribute| {
            let key = match &attribute.key {
                ImportAttributeKey::Identifier(identifier) => {
                    ModuleExportName::Identifier(identifier.name.as_str().into())
                }
                ImportAttributeKey::StringLiteral(string) => {
                    ModuleExportName::String(string.value.as_str().into())
                }
            };

            ModuleAttribute::new(key, attribute.value.value.as_str())
        })
        .collect()
}
