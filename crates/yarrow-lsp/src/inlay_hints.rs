//! `textDocument/inlayHint` from core `TypeIndex` probes (types / stack notes).

use tower_lsp_server::ls_types::{InlayHint, InlayHintKind, InlayHintLabel, Position, Range};
use yarrow_core::{Span, TypeProbe};

use crate::config::LspConfig;
use crate::position::{PositionEncoding, PositionMap};

/// Inlay hints for sites that intersect `range`, or empty when check fails / disabled.
pub fn inlay_hints(
    path: &str,
    text: &str,
    encoding: PositionEncoding,
    range: Range,
    config: &LspConfig,
) -> Vec<InlayHint> {
    if !config.inlay_hints_enable {
        return Vec::new();
    }

    let session = config.session(path);
    let Ok(checked) = session.check_source(text.to_string()) else {
        return Vec::new();
    };
    let map = PositionMap::from_file(&checked.file, encoding);
    let Some(range_span) = span_for_range(&map, range) else {
        return Vec::new();
    };

    let mut hints = Vec::new();
    for probe in checked.type_index.probes() {
        if !spans_overlap(probe.span, range_span) {
            continue;
        }
        let Some(hint) = hint_from_probe(&map, &probe) else {
            continue;
        };
        hints.push(hint);
    }
    hints.sort_by_key(|h| (h.position.line, h.position.character));
    hints
}

fn hint_from_probe(map: &PositionMap<'_>, probe: &TypeProbe) -> Option<InlayHint> {
    let label = label_for(probe)?;
    // Place after the identifier so the editor shows `name : i32` / stack note.
    let position = map.position(probe.span.hi);
    Some(InlayHint {
        position,
        label: InlayHintLabel::String(label),
        kind: Some(InlayHintKind::TYPE),
        text_edits: None,
        tooltip: None,
        padding_left: Some(true),
        padding_right: None,
        data: None,
    })
}

fn label_for(probe: &TypeProbe) -> Option<String> {
    if let Some(ty) = probe.ty.as_deref() {
        if ty.is_empty() {
            return None;
        }
        return Some(format!(": {ty}"));
    }
    let sig = probe.signature.as_deref()?;
    // Prefer the short stack-effect line when present.
    for line in sig.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("stack:") {
            let note = rest.trim();
            if !note.is_empty() {
                return Some(format!("stack: {note}"));
            }
        }
    }
    let first = sig.lines().next()?.trim();
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

fn span_for_range(map: &PositionMap<'_>, range: Range) -> Option<Span> {
    let lo = map.offset(range.start)?;
    let hi = map.offset(Position {
        line: range.end.line,
        character: range.end.character,
    })?;
    Some(Span::new(lo.min(hi), lo.max(hi)))
}

fn spans_overlap(site: Span, range: Span) -> bool {
    site.lo < range.hi && site.hi > range.lo
}
