//! JavaScript statement lowering.

mod block;
mod break_statement;
mod class;
mod continue_statement;
mod debugger;
mod do_while_statement;
mod export_default;
mod expression;
mod for_in_statement;
mod for_iteration;
mod for_of_statement;
mod for_statement;
mod if_statement;
mod labeled_statement;
mod return_statement;
mod switch_statement;
mod throw_statement;
mod try_statement;
mod variable;
mod while_statement;

use oxc_ast::ast::{Declaration, ImportOrExportKind, Statement};
use oxc_span::GetSpan;

use crate::{FrontendError, lower::FunctionLowerer};

/// Lowers statements in source order until the current block terminates.
pub(super) fn lower_statement_list(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statements: &[Statement<'_>],
) -> Result<(), FrontendError> {
    for statement in statements {
        if lowerer.current_block_is_terminated() {
            break;
        }

        lower_statement(lowerer, statement)?;
    }

    Ok(())
}

/// Lowers one JavaScript statement.
pub(super) fn lower_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &Statement<'_>,
) -> Result<(), FrontendError> {
    let span = statement.span();

    lowerer.with_span(span, |lowerer| {
        lower_statement_at_current_location(lowerer, statement)
    })
}

fn lower_statement_at_current_location(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &Statement<'_>,
) -> Result<(), FrontendError> {
    match statement {
        Statement::BreakStatement(statement) => {
            break_statement::lower_break_statement(lowerer, statement)
        }

        Statement::ClassDeclaration(class) if class.declare => Ok(()),

        Statement::ClassDeclaration(class) => class::lower_class_declaration(lowerer, class),

        Statement::ContinueStatement(statement) => {
            continue_statement::lower_continue_statement(lowerer, statement)
        }

        Statement::DebuggerStatement(statement) => {
            debugger::lower_debugger_statement(lowerer, statement);
            Ok(())
        }

        Statement::DoWhileStatement(statement) => {
            do_while_statement::lower_do_while_statement(lowerer, statement, Box::new([]))
        }

        Statement::EmptyStatement(_) => Ok(()),

        Statement::ExpressionStatement(statement) => {
            expression::lower_expression_statement(lowerer, statement)
        }

        Statement::ExportDefaultDeclaration(declaration) => {
            export_default::lower_export_default_declaration(lowerer, declaration)
        }

        // Static star exports are represented by module records.
        Statement::ExportAllDeclaration(_) => Ok(()),

        Statement::ExportNamedDeclaration(export) => match &export.declaration {
            Some(declaration) if is_ambient_declaration(declaration) => Ok(()),

            Some(Declaration::VariableDeclaration(declaration)) => {
                variable::lower_variable_declaration(lowerer, declaration)?;
                Ok(())
            }

            // Function declarations are emitted during declaration instantiation.
            Some(Declaration::FunctionDeclaration(_)) => Ok(()),

            Some(Declaration::ClassDeclaration(declaration)) => {
                class::lower_class_declaration(lowerer, declaration)
            }

            Some(
                Declaration::TSTypeAliasDeclaration(_) | Declaration::TSInterfaceDeclaration(_),
            ) => Ok(()),

            Some(_) => Err(FrontendError::UnsupportedStatement),

            // Local export lists are represented by module records.
            None => Ok(()),
        },

        Statement::FunctionDeclaration(_) => Ok(()),

        // Static imports are represented by module records.
        Statement::ImportDeclaration(_) => Ok(()),

        Statement::ForStatement(statement) => {
            for_statement::lower_for_statement(lowerer, statement, Box::new([]))
        }

        Statement::ForInStatement(statement) => {
            for_in_statement::lower_for_in_statement(lowerer, statement, Box::new([]))
        }

        Statement::ForOfStatement(statement) => {
            for_of_statement::lower_for_of_statement(lowerer, statement, Box::new([]))
        }

        Statement::BlockStatement(statement) => block::lower_block_statement(lowerer, statement),

        Statement::IfStatement(statement) => if_statement::lower_if_statement(lowerer, statement),

        Statement::LabeledStatement(statement) => {
            labeled_statement::lower_labeled_statement(lowerer, statement)
        }

        Statement::VariableDeclaration(declaration) if declaration.declare => Ok(()),

        Statement::VariableDeclaration(declaration) => {
            variable::lower_variable_declaration(lowerer, declaration)?;
            Ok(())
        }

        Statement::ReturnStatement(statement) => {
            return_statement::lower_return_statement(lowerer, statement)
        }

        Statement::SwitchStatement(statement) => {
            switch_statement::lower_switch_statement(lowerer, statement, Box::new([]))
        }

        Statement::ThrowStatement(statement) => {
            throw_statement::lower_throw_statement(lowerer, statement)
        }

        Statement::TSTypeAliasDeclaration(_) | Statement::TSInterfaceDeclaration(_) => Ok(()),

        Statement::TSEnumDeclaration(declaration) if declaration.declare => Ok(()),

        Statement::TSModuleDeclaration(declaration) if declaration.declare => Ok(()),

        Statement::TSGlobalDeclaration(declaration) if declaration.declare => Ok(()),

        Statement::TSImportEqualsDeclaration(declaration)
            if declaration.import_kind == ImportOrExportKind::Type =>
        {
            Ok(())
        }

        Statement::TryStatement(statement) => {
            try_statement::lower_try_statement(lowerer, statement)
        }

        Statement::WhileStatement(statement) => {
            while_statement::lower_while_statement(lowerer, statement, Box::new([]))
        }

        _ => Err(FrontendError::UnsupportedStatement),
    }
}

fn is_ambient_declaration(declaration: &Declaration<'_>) -> bool {
    match declaration {
        Declaration::VariableDeclaration(declaration) => declaration.declare,
        Declaration::FunctionDeclaration(declaration) => declaration.declare,
        Declaration::ClassDeclaration(declaration) => declaration.declare,
        Declaration::TSEnumDeclaration(declaration) => declaration.declare,
        Declaration::TSModuleDeclaration(declaration) => declaration.declare,
        Declaration::TSGlobalDeclaration(declaration) => declaration.declare,
        _ => false,
    }
}
