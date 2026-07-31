mod error;
mod lower;
mod module_attributes;
mod parse;

use evrel_js_ir::JsModuleIr;
use oxc_allocator::Allocator;
use oxc_span::SourceType;

pub use error::FrontendError;

/// Parses and lowers one source file using its filename.
pub fn lower_source_file(
    source_name: &str,
    source_text: &str,
) -> Result<JsModuleIr, FrontendError> {
    let source_type = source_type_from_name(source_name)?;

    lower_module_with_source_type(source_name, source_text, source_type)
}

fn source_type_from_name(source_name: &str) -> Result<SourceType, FrontendError> {
    SourceType::from_path(source_name).map_err(|error| FrontendError::UnknownSourceType {
        source_name: source_name.into(),
        reason: error.to_string().into(),
    })
}

fn lower_module_with_source_type(
    source_name: &str,
    source: &str,
    source_type: SourceType,
) -> Result<JsModuleIr, FrontendError> {
    let allocator = Allocator::new();
    let parsed = parse::parse_module(&allocator, source, source_type)?;

    lower::lower_module(&parsed, source_name, source)
}
