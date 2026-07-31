//! Module IR storage and construction.

mod builder;
mod editor;
mod export;
mod import;
mod module;
mod name;

pub use builder::ModuleBuilder;
pub use editor::ModuleEditor;
pub use export::ModuleExport;
pub use import::ModuleImport;
pub use module::JsModuleIr;
pub use name::{ModuleAttribute, ModuleExportName};
