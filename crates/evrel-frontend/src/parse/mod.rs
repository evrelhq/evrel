//! JavaScript parsing and semantic analysis through Oxc.

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::{Scoping, SemanticBuilder};
use oxc_span::SourceType;

use crate::FrontendError;

/// Short-lived frontend information produced by Oxc.
///
/// The AST borrows from the Oxc allocator. Lowering must convert all required
/// information into compiler-owned IR before that allocator is discarded.
#[derive(Debug)]
pub(crate) struct ParsedModule<'a> {
    program: &'a Program<'a>,
    scoping: Scoping,
}

impl<'a> ParsedModule<'a> {
    /// Returns the parsed program.
    pub(crate) const fn program(&self) -> &'a Program<'a> {
        self.program
    }

    /// Returns scopes, symbols, bindings, and references discovered by Oxc.
    pub(crate) const fn scoping(&self) -> &Scoping {
        &self.scoping
    }
}

/// Parses and semantically analyzes one JavaScript module.
pub(crate) fn parse_module<'a>(
    allocator: &'a Allocator,
    source: &'a str,
    source_type: SourceType,
) -> Result<ParsedModule<'a>, FrontendError> {
    let parsed = Parser::new(allocator, source, source_type)
        .with_options(ParseOptions {
            parse_regular_expression: true,
            ..ParseOptions::default()
        })
        .parse();

    if parsed.diagnostics.has_errors() {
        return Err(FrontendError::Parse {
            diagnostics: parsed
                .diagnostics
                .into_vec()
                .into_iter()
                .map(|diagnostic| diagnostic.to_string())
                .collect(),
        });
    }

    let program = allocator.alloc(parsed.program);
    let semantic = SemanticBuilder::new_compiler().build(program);

    if semantic.diagnostics.has_errors() {
        return Err(FrontendError::Semantic {
            diagnostics: semantic
                .diagnostics
                .into_vec()
                .into_iter()
                .map(|diagnostic| diagnostic.to_string())
                .collect(),
        });
    }

    Ok(ParsedModule {
        program,
        scoping: semantic.semantic.into_scoping(),
    })
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;
    use oxc_span::SourceType;

    use super::parse_module;
    use crate::FrontendError;

    #[test]
    fn parses_a_javascript_module() {
        let allocator = Allocator::new();
        let parsed = parse_module(
            &allocator,
            r#"import { value } from "dependency"; value;"#,
            SourceType::mjs(),
        )
        .unwrap();

        assert_eq!(parsed.program().body.len(), 2);
    }

    #[test]
    fn returns_parse_diagnostics() {
        let allocator = Allocator::new();

        let Err(FrontendError::Parse { diagnostics }) =
            parse_module(&allocator, "const =", SourceType::mjs())
        else {
            panic!("invalid JavaScript must return parse diagnostics");
        };

        assert!(!diagnostics.is_empty());
    }

    #[test]
    fn validates_regexp_literal_patterns() {
        let allocator = Allocator::new();

        assert!(matches!(
            parse_module(&allocator, r"/\p{ASCII=F}/u;", SourceType::mjs()),
            Err(FrontendError::Parse { .. })
        ));
    }

    #[test]
    fn distinguishes_global_and_local_references() {
        let allocator = Allocator::new();
        let parsed = parse_module(
            &allocator,
            "let value = 42; console.log(value);",
            SourceType::mjs(),
        )
        .unwrap();

        let unresolved = parsed.scoping().root_unresolved_references();

        assert!(unresolved.contains_key("console"));
        assert!(!unresolved.contains_key("value"));
    }
}
