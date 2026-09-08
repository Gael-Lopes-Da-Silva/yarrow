//! `textDocument/codeAction` for diagnostic explain (`yarrow explain` catalog).

use std::collections::HashSet;

use serde_json::json;
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionResponse, Command, Diagnostic,
    NumberOrString, Range,
};
use yarrow_core::{explain_code, format_explain};

/// Workspace command run when the user picks “Explain Exxx”.
pub const EXPLAIN_COMMAND: &str = "yarrow.explain";

/// Build explain code actions for diagnostics that intersect `range`.
///
/// No semantic rewrites: actions only surface [`format_explain`] text via
/// [`EXPLAIN_COMMAND`] (and `data.explain` for clients / resolve).
pub fn code_actions(
    diagnostics: &[Diagnostic],
    range: Range,
    only: Option<&[CodeActionKind]>,
) -> Option<CodeActionResponse> {
    if let Some(kinds) = only
        && !kinds.is_empty()
        && !kinds.iter().any(|k| k.as_str().is_empty())
    {
        // Explain is not a quick-fix / refactor; skip when the client filters those only.
        return None;
    }

    let mut actions = Vec::new();
    let mut seen = HashSet::new();

    for diag in diagnostics {
        if !ranges_overlap(diag.range, range) {
            continue;
        }
        let Some(NumberOrString::String(code)) = &diag.code else {
            continue;
        };
        if !seen.insert(code.clone()) {
            continue;
        }
        let Some(entry) = explain_code(code) else {
            continue;
        };
        let explain = format_explain(entry);
        actions.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: format!("Explain {}", entry.code),
            kind: Some(CodeActionKind::EMPTY),
            diagnostics: Some(vec![diag.clone()]),
            command: Some(Command {
                title: format!("Explain {}", entry.code),
                command: EXPLAIN_COMMAND.into(),
                arguments: Some(vec![json!(entry.code)]),
            }),
            data: Some(json!({
                "code": entry.code,
                "explain": explain,
            })),
            ..Default::default()
        }));
    }

    if actions.is_empty() {
        None
    } else {
        Some(actions)
    }
}

/// Resolve explain text for [`EXPLAIN_COMMAND`] arguments (code string).
pub fn explain_command_text(arguments: &[serde_json::Value]) -> Option<String> {
    let code = arguments.first()?.as_str()?;
    let entry = explain_code(code)?;
    Some(format_explain(entry))
}

fn ranges_overlap(a: Range, b: Range) -> bool {
    !(position_le(a.end, b.start) || position_le(b.end, a.start))
}

fn position_le(
    a: tower_lsp_server::ls_types::Position,
    b: tower_lsp_server::ls_types::Position,
) -> bool {
    a.line < b.line || (a.line == b.line && a.character <= b.character)
}
