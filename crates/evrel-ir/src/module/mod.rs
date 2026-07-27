//! Module IR storage and construction.

mod builder;
mod export;
mod import;
mod module;
mod name;

pub use builder::ModuleBuilder;
pub use export::ModuleExport;
pub use import::ModuleImport;
pub use module::ModuleIr;
pub use name::{ModuleAttribute, ModuleExportName};
