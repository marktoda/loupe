use std::sync::LazyLock;

use ratatui::prelude::*;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Convert a Claude Code Edit tool input (old_string/new_string) into patch format
/// and render with syntax highlighting.
pub fn render_edit(input: &serde_json::Value, content_cols: usize) -> Vec<Line<'static>> {
    let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
    let old = input.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
    let new = input.get("new_string").and_then(|v| v.as_str()).unwrap_or("");

    let mut patch = format!("*** Update File: {file_path}\n@@\n");
    for line in old.lines() {
        patch.push('-');
        patch.push_str(line);
        patch.push('\n');
    }
    for line in new.lines() {
        patch.push('+');
        patch.push_str(line);
        patch.push('\n');
    }

    render_patch(&patch, content_cols)
}

/// Render a Codex apply_patch as diff-colored, syntax-highlighted lines.
pub fn render_patch(patch: &str, content_cols: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut current_file: Option<&str> = None;

    for line in patch.lines() {
        if let Some(path) = line.strip_prefix("*** Update File: ") {
            current_file = Some(path);
            lines.push(patch_meta_line(line));
        } else if line.starts_with("*** ") {
            // Begin Patch, End Patch, etc.
            lines.push(patch_meta_line(line));
        } else if line.starts_with("@@") {
            lines.push(hunk_header_line(line));
        } else {
            lines.push(diff_line(line, current_file, content_cols));
        }
    }
    lines
}

fn patch_meta_line(line: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw("         "),
        Span::styled(
            format!("│ {line}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn hunk_header_line(line: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw("         "),
        Span::styled(
            format!("│ {line}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::DIM),
        ),
    ])
}

fn diff_line(line: &str, current_file: Option<&str>, _content_cols: usize) -> Line<'static> {
    let (prefix_char, code, prefix_style) = if let Some(rest) = line.strip_prefix('+') {
        ("+", rest, Style::default().fg(Color::Green))
    } else if let Some(rest) = line.strip_prefix('-') {
        ("-", rest, Style::default().fg(Color::Red))
    } else if let Some(rest) = line.strip_prefix(' ') {
        (" ", rest, Style::default().add_modifier(Modifier::DIM))
    } else {
        // Bare context line (no prefix)
        return Line::from(vec![
            Span::raw("         "),
            Span::styled(
                format!("│ {line}"),
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]);
    };

    // Syntax-highlight the code portion
    let code_spans = syntax_highlight(code, current_file, prefix_char);

    let mut spans = Vec::with_capacity(code_spans.len() + 3);
    spans.push(Span::raw("         "));
    spans.push(Span::styled("│ ", Style::default().add_modifier(Modifier::DIM)));
    spans.push(Span::styled(prefix_char.to_string(), prefix_style));
    spans.extend(code_spans);
    Line::from(spans)
}

fn syntax_highlight(code: &str, current_file: Option<&str>, diff_prefix: &str) -> Vec<Span<'static>> {
    let ext = current_file
        .and_then(|p| p.rsplit('.').next())
        .unwrap_or("");

    let syntax = SYNTAX_SET
        .find_syntax_by_extension(ext)
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let theme = &THEME_SET.themes["base16-ocean.dark"];
    let mut highlighter = HighlightLines::new(syntax, theme);

    let Ok(regions) = highlighter.highlight_line(code, &SYNTAX_SET) else {
        // Fallback: return code with just the diff color
        let style = match diff_prefix {
            "+" => Style::default().fg(Color::Green),
            "-" => Style::default().fg(Color::Red),
            _ => Style::default().add_modifier(Modifier::DIM),
        };
        return vec![Span::styled(code.to_string(), style)];
    };

    regions
        .into_iter()
        .map(|(syntect_style, text)| {
            let mut style = syntect_to_ratatui(syntect_style);
            // Tint based on diff prefix
            match diff_prefix {
                "+" => {
                    // Blend toward green for added lines
                    if let Some(fg) = style.fg {
                        style = style.fg(tint_green(fg));
                    }
                }
                "-" => {
                    // Blend toward red for removed lines
                    if let Some(fg) = style.fg {
                        style = style.fg(tint_red(fg));
                    }
                }
                _ => {
                    // Dim context lines
                    style = style.add_modifier(Modifier::DIM);
                }

            }
            Span::styled(text.to_string(), style)
        })
        .collect()
}

fn syntect_to_ratatui(style: syntect::highlighting::Style) -> Style {
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    let mut s = Style::default().fg(fg);
    if style.font_style.contains(FontStyle::BOLD) {
        s = s.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        s = s.add_modifier(Modifier::ITALIC);
    }
    s
}

/// Tint an RGB color toward green (for added lines).
fn tint_green(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            // Boost green channel, slightly reduce others
            let r = (r as u16 * 7 / 10) as u8;
            let g = ((g as u16).saturating_add(60)).min(255) as u8;
            let b = (b as u16 * 7 / 10) as u8;
            Color::Rgb(r, g, b)
        }
        _ => Color::Green,
    }
}

/// Tint an RGB color toward red (for removed lines).
fn tint_red(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let r = ((r as u16).saturating_add(60)).min(255) as u8;
            let g = (g as u16 * 7 / 10) as u8;
            let b = (b as u16 * 7 / 10) as u8;
            Color::Rgb(r, g, b)
        }
        _ => Color::Red,
    }
}
