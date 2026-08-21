//! Rustc-like multi-label diagnostic rendering.

use super::{Diagnostic, Label, Severity, SourceFile, Span};

/// Whether to emit ANSI colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChoice {
    Always,
    Never,
    /// Color when stderr is a TTY and `NO_COLOR` is unset.
    Auto,
}

impl ColorChoice {
    pub fn resolve(self) -> bool {
        match self {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                if std::env::var_os("NO_COLOR").is_some() {
                    return false;
                }
                // Best-effort: assume color if we cannot probe; drivers may
                // override with Always/Never.
                true
            }
        }
    }
}

struct Style {
    color: bool,
}

impl Style {
    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    fn error(&self, text: &str) -> String {
        self.paint("1;31", text) // bold red
    }

    fn warning(&self, text: &str) -> String {
        self.paint("1;33", text) // bold yellow
    }

    fn blue(&self, text: &str) -> String {
        self.paint("1;34", text) // bold blue
    }

    fn green(&self, text: &str) -> String {
        self.paint("1;32", text) // bold green
    }

    fn cyan(&self, text: &str) -> String {
        self.paint("1;36", text) // bold cyan
    }
}

/// Render `diag` against `file` in rustc layout.
pub fn render(diag: &Diagnostic, file: &SourceFile, color: ColorChoice) -> String {
    let style = Style {
        color: color.resolve(),
    };
    let mut out = String::new();

    let (sev_word, sev_paint): (&str, fn(&Style, &str) -> String) = match diag.severity {
        Severity::Error => ("error", Style::error),
        Severity::Warning => ("warning", Style::warning),
        Severity::Note => ("note", Style::blue),
        Severity::Help => ("help", Style::green),
    };
    let header = if diag.code.is_empty() {
        format!("{sev_word}: {}", diag.message)
    } else {
        format!("{sev_word}[{}]: {}", diag.code, diag.message)
    };
    out.push_str(&sev_paint(&style, &header));
    out.push('\n');

    let path = if diag.path.is_empty() {
        file.path.as_str()
    } else {
        diag.path.as_str()
    };

    let primary = diag
        .labels
        .iter()
        .find(|l| l.primary)
        .or(diag.labels.first());
    if let Some(label) = primary {
        let loc = file.span_start(label.span);
        out.push_str(&format!(
            "  {} {}:{}:{}\n",
            style.blue("-->"),
            path,
            loc.line,
            loc.column
        ));
    } else {
        out.push_str(&format!("  {} {path}\n", style.blue("-->")));
    }

    if diag.labels.is_empty() {
        append_notes_helps(&mut out, diag, &style);
        return out;
    }

    // Collect lines that need annotation.
    let mut line_set: Vec<usize> = diag
        .labels
        .iter()
        .flat_map(|l| {
            let start = file.span_start(l.span).line;
            let end = file.span_end(l.span).line.max(start);
            start..=end
        })
        .collect();
    line_set.sort_unstable();
    line_set.dedup();

    let gutter_width = line_set
        .last()
        .map(|n| n.to_string().len())
        .unwrap_or(1)
        .max(1);

    out.push_str(&format!(
        "   {} {}\n",
        " ".repeat(gutter_width),
        style.blue("|")
    ));

    for (idx, &line_no) in line_set.iter().enumerate() {
        if idx > 0 {
            let prev = line_set[idx - 1];
            if line_no > prev + 1 {
                out.push_str(&format!(
                    "   {} {}\n",
                    " ".repeat(gutter_width),
                    style.blue("...")
                ));
            }
        }

        let text = file.line_text(line_no);
        let line_num = format!("{line_no:>gutter_width$}");
        out.push_str(&format!(
            "   {} {} {}\n",
            style.blue(&line_num),
            style.blue("|"),
            text
        ));

        let labels_on_line: Vec<&Label> = diag
            .labels
            .iter()
            .filter(|l| {
                let s = file.span_start(l.span).line;
                let e = file.span_end(l.span).line.max(s);
                (s..=e).contains(&line_no)
            })
            .collect();

        // Skip blank lines for underlines; still show the source row above.
        if text.trim().is_empty() {
            continue;
        }

        if labels_on_line.is_empty() {
            continue;
        }

        // Underline row: place carets / dashes under each span on this line.
        let display_cols = display_column_count(text);
        let mut marks = vec![b' '; display_cols.max(1)];
        let mut messages: Vec<(usize, bool, &str)> = Vec::new();

        for label in &labels_on_line {
            let (col_start, col_end) = underline_cols(file, text, line_no, label.span);
            let start = col_start
                .saturating_sub(1)
                .min(marks.len().saturating_sub(1));
            let end = col_end.saturating_sub(1).max(start + 1).min(marks.len());
            let ch = if label.primary { b'^' } else { b'-' };
            for m in &mut marks[start..end] {
                // Primary carets win over secondary dashes on the same column.
                if label.primary || *m != b'^' {
                    *m = ch;
                }
            }
            if !label.message.is_empty() {
                messages.push((start, label.primary, label.message.as_str()));
            }
        }

        let underline = String::from_utf8(marks).unwrap_or_default();
        let painted = if labels_on_line.iter().any(|l| l.primary) {
            style.error(underline.trim_end())
        } else {
            style.blue(underline.trim_end())
        };
        out.push_str(&format!(
            "   {} {} {}\n",
            " ".repeat(gutter_width),
            style.blue("|"),
            painted
        ));

        // Message lines aligned under the leftmost label on this line.
        messages.sort_by_key(|(col, primary, _)| (*col, !primary));
        for (col, primary, msg) in messages {
            let pad = " ".repeat(col);
            let colored = if primary {
                style.error(msg)
            } else {
                style.blue(msg)
            };
            out.push_str(&format!(
                "   {} {} {pad}{colored}\n",
                " ".repeat(gutter_width),
                style.blue("|"),
            ));
        }
    }

    out.push_str(&format!(
        "   {} {}\n",
        " ".repeat(gutter_width),
        style.blue("|")
    ));
    append_notes_helps(&mut out, diag, &style);
    out
}

fn append_notes_helps(out: &mut String, diag: &Diagnostic, style: &Style) {
    for note in &diag.notes {
        out.push_str(&format!(
            "   {} {}: {}\n",
            style.blue("="),
            style.cyan("note"),
            note
        ));
    }
    for help in &diag.helps {
        out.push_str(&format!(
            "   {} {}: {}\n",
            style.blue("="),
            style.green("help"),
            help
        ));
    }
}

fn display_column_count(text: &str) -> usize {
    text.chars().count()
}

/// 1-based display columns covering `span` on `line_no`.
fn underline_cols(
    file: &SourceFile,
    line_text: &str,
    line_no: usize,
    span: Span,
) -> (usize, usize) {
    let start_loc = file.span_start(span);
    let end_loc = file.span_end(span);

    let col_start = if start_loc.line == line_no {
        start_loc.column
    } else {
        1
    };
    let col_end = if end_loc.line == line_no {
        if end_loc.column > col_start {
            end_loc.column
        } else {
            col_start + 1
        }
    } else {
        display_column_count(line_text) + 1
    };
    (col_start, col_end)
}
