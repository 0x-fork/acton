use super::TolkCompletionProviderContext;
use super::support::{ProviderGroup, add_snippet, provider_group};
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};

/// Completes common statement snippets in statement contexts.
///
/// Loop transfers require an enclosing loop body in the same function and cannot
/// occur inside `try`/`catch`. `catch` is offered after a compatible `try` statement.
pub(crate) struct StatementSnippetCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for StatementSnippetCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::General && context.syntax.is_statement()
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        for &(label, snippet) in STATEMENT_SNIPPETS {
            add_snippet(
                context.syntax,
                collector,
                label,
                snippet,
                CompletionCategory::Snippet,
            );
        }
        if allows_loop_transfer(context) {
            for (label, snippet) in [("break", "break;"), ("continue", "continue;")] {
                add_snippet(
                    context.syntax,
                    collector,
                    label,
                    if context.syntax.before_semicolon {
                        label
                    } else {
                        snippet
                    },
                    CompletionCategory::Snippet,
                );
            }
        }
        if follows_try_statement(context) {
            add_snippet(
                context.syntax,
                collector,
                "catch",
                "catch (${1:e}) {\n\t$0\n}",
                CompletionCategory::ContextElement,
            );
        }
        Some(())
    }
}

/// Checks lexical restrictions on loop transfers; compiler control-flow analysis
/// remains responsible for restrictions involving other statements in the loop.
fn allows_loop_transfer(context: &TolkCompletionProviderContext<'_>) -> bool {
    let Some(mut child) = context.syntax.cursor_node() else {
        return false;
    };
    let mut inside_loop = false;
    while let Some(parent) = child.parent() {
        match parent.kind() {
            "function_declaration"
            | "method_declaration"
            | "get_method_declaration"
            | "lambda_expression" => break,
            "try_catch_statement" => return false,
            "while_statement" | "do_while_statement" | "repeat_statement" => {
                if parent.child_by_field_name("body") != Some(child) {
                    return false;
                }
                inside_loop = true;
            }
            _ => {}
        }
        child = parent;
    }
    inside_loop
}

fn follows_try_statement(context: &TolkCompletionProviderContext<'_>) -> bool {
    context
        .syntax
        .cursor_node()
        .and_then(|node| node.parent())
        .and_then(|node| node.prev_named_sibling())
        .is_some_and(|node| {
            node.kind() == "try_statement"
                || node
                    .child(0)
                    .is_some_and(|first_child| first_child.kind() == "try")
        })
}

const STATEMENT_SNIPPETS: &[(&str, &str)] = &[
    ("val", "val ${1:name} = ${2:value};"),
    ("var", "var ${1:name} = ${2:value};"),
    ("valt", "val ${1:name}: ${2:int} = ${3:value};"),
    ("vart", "var ${1:name}: ${2:int} = ${3:value};"),
    ("if", "if (${1:condition}) {\n\t$0\n}"),
    ("ife", "if (${1:condition}) {\n\t$2\n} else {\n\t$0\n}"),
    ("while", "while (${1:condition}) {\n\t$0\n}"),
    ("do-while", "do {\n\t$0\n} while (${1:condition});"),
    ("repeat", "repeat(${1:count}) {\n\t$0\n}"),
    ("try", "try {\n\t$0\n}"),
    ("tryc", "try {\n\t$1\n} catch (${2:e}) {\n\t$0\n}"),
];
