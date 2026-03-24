use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::prelude::*;

#[derive(Clone)]
struct StyledChunk {
    text: String,
    style: Style,
}

enum MdBlock {
    Paragraph(Vec<StyledChunk>),
    Heading { level: u8, content: Vec<StyledChunk> },
    CodeFence { language: Option<String>, text: String },
    BulletList(Vec<Vec<StyledChunk>>),
    OrderedList { start: u64, items: Vec<Vec<StyledChunk>> },
    Quote(Vec<StyledChunk>),
    ThematicBreak,
}

enum Container {
    Paragraph(Vec<StyledChunk>),
    Heading {
        level: u8,
        content: Vec<StyledChunk>,
    },
    Quote(Vec<StyledChunk>),
    List(ListState),
}

struct ListState {
    ordered: bool,
    start: u64,
    items: Vec<Vec<StyledChunk>>,
    current_item: Vec<StyledChunk>,
}

pub struct MarkdownRenderOptions<'a> {
    pub label: &'static str,
    pub label_style: Style,
    pub content_cols: usize,
    pub base_style: Style,
    pub search_query: &'a str,
    pub search_active: bool,
}

pub fn render_markdown_block(text: &str, opts: &MarkdownRenderOptions<'_>) -> Vec<Line<'static>> {
    let blocks = parse_markdown(text, opts.base_style);
    let mut lines = Vec::new();
    let mut first_visual_line = true;

    for block in blocks {
        let block_lines = render_block(&block, opts);
        for line in block_lines {
            let prefix = if first_visual_line {
                Span::styled(opts.label, opts.label_style)
            } else {
                Span::raw("         ")
            };
            let mut spans = Vec::with_capacity(line.spans.len() + 1);
            spans.push(prefix);
            spans.extend(line.spans);
            lines.push(Line::from(spans));
            first_visual_line = false;
        }
        lines.push(Line::default());
    }

    if lines.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            opts.label,
            opts.label_style,
        )]));
        lines.push(Line::default());
    }

    lines
}

pub fn soft_wrap_plain(text: &str, max_cols: usize) -> Vec<&str> {
    if max_cols == 0 || text.is_empty() {
        return vec![text];
    }
    let mut result = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        match remaining.char_indices().nth(max_cols) {
            None => {
                result.push(remaining);
                break;
            }
            Some((byte_end, _)) => {
                result.push(&remaining[..byte_end]);
                remaining = &remaining[byte_end..];
            }
        }
    }
    result
}

fn parse_markdown(text: &str, base_style: Style) -> Vec<MdBlock> {
    let parser = Parser::new_ext(text, Options::all());
    let mut blocks = Vec::new();
    let mut containers: Vec<Container> = Vec::new();
    let mut style_stack = vec![base_style];
    let mut code_fence: Option<(Option<String>, String)> = None;

    for event in parser {
        if let Some((_, code)) = code_fence.as_mut() {
            match event {
                Event::End(TagEnd::CodeBlock) => {
                    let (language, text) = code_fence.take().unwrap();
                    push_block(&mut blocks, &mut containers, MdBlock::CodeFence { language, text });
                }
                Event::Text(t) | Event::Code(t) => code.push_str(&t),
                Event::SoftBreak | Event::HardBreak => code.push('\n'),
                _ => {}
            }
            continue;
        }

        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => containers.push(Container::Paragraph(Vec::new())),
                Tag::Heading { level, .. } => containers.push(Container::Heading {
                    level: heading_level_to_u8(level),
                    content: Vec::new(),
                }),
                Tag::BlockQuote(_) => containers.push(Container::Quote(Vec::new())),
                Tag::CodeBlock(kind) => {
                    let language = match kind {
                        CodeBlockKind::Indented => None,
                        CodeBlockKind::Fenced(lang) => {
                            let trimmed = lang.trim();
                            if trimmed.is_empty() {
                                None
                            } else {
                                Some(trimmed.to_string())
                            }
                        }
                    };
                    code_fence = Some((language, String::new()));
                }
                Tag::List(start) => containers.push(Container::List(ListState {
                    ordered: start.is_some(),
                    start: start.unwrap_or(1),
                    items: Vec::new(),
                    current_item: Vec::new(),
                })),
                Tag::Item => {}
                Tag::Emphasis => style_stack.push(current_style(&style_stack).add_modifier(Modifier::ITALIC)),
                Tag::Strong => style_stack.push(current_style(&style_stack).add_modifier(Modifier::BOLD)),
                Tag::Link { .. } => {
                    style_stack.push(
                        current_style(&style_stack)
                            .fg(Color::Blue)
                            .add_modifier(Modifier::UNDERLINED),
                    );
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Paragraph => {
                    if let Some(Container::Paragraph(content)) = containers.pop() {
                        push_block(&mut blocks, &mut containers, MdBlock::Paragraph(content));
                    }
                }
                TagEnd::Heading(_) => {
                    if let Some(Container::Heading { level, content }) = containers.pop() {
                        push_block(&mut blocks, &mut containers, MdBlock::Heading { level, content });
                    }
                }
                TagEnd::BlockQuote(_) => {
                    if let Some(Container::Quote(content)) = containers.pop() {
                        push_block(&mut blocks, &mut containers, MdBlock::Quote(content));
                    }
                }
                TagEnd::List(_) => {
                    if let Some(Container::List(list)) = containers.pop() {
                        let block = if list.ordered {
                            MdBlock::OrderedList {
                                start: list.start,
                                items: list.items,
                            }
                        } else {
                            MdBlock::BulletList(list.items)
                        };
                        push_block(&mut blocks, &mut containers, block);
                    }
                }
                TagEnd::Item => {
                    if let Some(Container::List(list)) = containers.last_mut() {
                        if !list.current_item.is_empty() {
                            list.items.push(std::mem::take(&mut list.current_item));
                        }
                    }
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Link => {
                    if style_stack.len() > 1 {
                        style_stack.pop();
                    }
                }
                _ => {}
            },
            Event::Text(t) => push_inline(&mut containers, styled_chunk(t.to_string(), current_style(&style_stack))),
            Event::Code(t) => push_inline(
                &mut containers,
                styled_chunk(
                    t.to_string(),
                    current_style(&style_stack)
                        .bg(Color::DarkGray)
                        .fg(Color::Yellow),
                ),
            ),
            Event::SoftBreak => push_inline(&mut containers, styled_chunk(" ".to_string(), current_style(&style_stack))),
            Event::HardBreak => push_inline(&mut containers, styled_chunk("\n".to_string(), current_style(&style_stack))),
            Event::Rule => blocks.push(MdBlock::ThematicBreak),
            Event::TaskListMarker(checked) => {
                let marker = if checked { "[x] " } else { "[ ] " };
                push_inline(&mut containers, styled_chunk(marker.to_string(), current_style(&style_stack)));
            }
            _ => {}
        }
    }

    blocks
}

fn push_block(blocks: &mut Vec<MdBlock>, containers: &mut [Container], block: MdBlock) {
    if let Some(parent) = containers.last_mut() {
        match parent {
            Container::Quote(content) => {
                let text = match block {
                    MdBlock::Paragraph(chunks) | MdBlock::Quote(chunks) => chunks,
                    MdBlock::Heading { content, .. } => content,
                    _ => {
                        blocks.push(block);
                        return;
                    }
                };
                append_chunks(content, text);
                content.push(styled_chunk("\n".to_string(), Style::default()));
            }
            Container::List(list) => match block {
                MdBlock::Paragraph(chunks) | MdBlock::Quote(chunks) => {
                    append_chunks(&mut list.current_item, chunks);
                }
                MdBlock::Heading { content, .. } => append_chunks(&mut list.current_item, content),
                _ => blocks.push(block),
            },
            _ => blocks.push(block),
        }
    } else {
        blocks.push(block);
    }
}

fn push_inline(containers: &mut [Container], chunk: StyledChunk) {
    if let Some(container) = containers.last_mut() {
        match container {
            Container::Paragraph(content) => content.push(chunk),
            Container::Heading { content, .. } => content.push(chunk),
            Container::Quote(content) => content.push(chunk),
            Container::List(list) => list.current_item.push(chunk),
        }
    }
}

fn append_chunks(target: &mut Vec<StyledChunk>, mut source: Vec<StyledChunk>) {
    target.append(&mut source);
}

fn current_style(stack: &[Style]) -> Style {
    stack.last().copied().unwrap_or_default()
}

fn styled_chunk(text: String, style: Style) -> StyledChunk {
    StyledChunk { text, style }
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn render_block(block: &MdBlock, opts: &MarkdownRenderOptions<'_>) -> Vec<Line<'static>> {
    match block {
        MdBlock::Paragraph(content) => wrap_chunks(content, opts.content_cols, "", opts),
        MdBlock::Heading { level, content } => {
            let style = match level {
                1 => opts.base_style.fg(Color::Cyan).add_modifier(Modifier::BOLD),
                2 | 3 => opts.base_style.fg(Color::Blue).add_modifier(Modifier::BOLD),
                _ => opts.base_style.add_modifier(Modifier::BOLD),
            };
            let mut styled = content.clone();
            for chunk in &mut styled {
                chunk.style = style.patch(chunk.style);
            }
            wrap_chunks(&styled, opts.content_cols, "", opts)
        }
        MdBlock::CodeFence { language, text } => render_code_fence(language.as_deref(), text, opts),
        MdBlock::BulletList(items) => render_list(items, None, opts),
        MdBlock::OrderedList { start, items } => render_list(items, Some(*start), opts),
        MdBlock::Quote(content) => {
            let quote_opts = MarkdownRenderOptions {
                label: opts.label,
                label_style: opts.label_style,
                content_cols: opts.content_cols.saturating_sub(2),
                base_style: opts.base_style.add_modifier(Modifier::DIM),
                search_query: opts.search_query,
                search_active: opts.search_active,
            };
            let inner = wrap_chunks(content, quote_opts.content_cols, "", &quote_opts);
            inner
                .into_iter()
                .map(|line| {
                    let mut spans = vec![
                        Span::styled("│ ", Style::default().add_modifier(Modifier::DIM)),
                    ];
                    spans.extend(line.spans);
                    Line::from(spans)
                })
                .collect()
        }
        MdBlock::ThematicBreak => vec![Line::from(vec![Span::styled(
            "─".repeat(opts.content_cols.max(3)),
            Style::default().add_modifier(Modifier::DIM),
        )])],
    }
}

fn render_list(
    items: &[Vec<StyledChunk>],
    ordered_start: Option<u64>,
    opts: &MarkdownRenderOptions<'_>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        let marker = if let Some(start) = ordered_start {
            format!("{}. ", start + idx as u64)
        } else {
            "• ".to_string()
        };
        let wrapped = wrap_chunks(
            item,
            opts.content_cols.saturating_sub(marker.chars().count()),
            "",
            opts,
        );
        for (line_idx, line) in wrapped.into_iter().enumerate() {
            let mut spans = Vec::new();
            if line_idx == 0 {
                spans.push(Span::styled(
                    marker.clone(),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::raw(" ".repeat(marker.chars().count())));
            }
            spans.extend(line.spans);
            lines.push(Line::from(spans));
        }
    }
    lines
}

fn render_code_fence(
    language: Option<&str>,
    text: &str,
    opts: &MarkdownRenderOptions<'_>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let title = language.unwrap_or("text");
    lines.push(Line::from(vec![Span::styled(
        format!("┌─ {title} ─"),
        Style::default().add_modifier(Modifier::DIM),
    )]));
    for source_line in text.lines() {
        for chunk in soft_wrap_plain(source_line, opts.content_cols.saturating_sub(2)) {
            lines.push(Line::from(vec![
                Span::styled("│ ", Style::default().add_modifier(Modifier::DIM)),
                Span::styled(chunk.to_string(), Style::default().fg(Color::Green)),
            ]));
        }
    }
    if text.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "│",
            Style::default().add_modifier(Modifier::DIM),
        )]));
    }
    lines.push(Line::from(vec![Span::styled(
        "└──────────",
        Style::default().add_modifier(Modifier::DIM),
    )]));
    lines
}

fn wrap_chunks(
    chunks: &[StyledChunk],
    width: usize,
    _prefix: &str,
    opts: &MarkdownRenderOptions<'_>,
) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::default()];
    }

    let mut result = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut remaining = width;

    for chunk in chunks {
        let text = if opts.search_active && !opts.search_query.is_empty() {
            split_for_highlight(&chunk.text, opts.search_query, chunk.style)
        } else {
            vec![Span::styled(chunk.text.clone(), chunk.style)]
        };

        for span in text {
            let mut segment = span.content.to_string();
            while !segment.is_empty() {
                let take = take_prefix_by_width(&segment, remaining);
                let taken = segment[..take].to_string();
                current.push(Span::styled(taken, span.style));
                segment = segment[take..].to_string();
                remaining = remaining.saturating_sub(char_width(&current.last().unwrap().content));
                if remaining == 0 && !segment.is_empty() {
                    result.push(Line::from(std::mem::take(&mut current)));
                    remaining = width;
                }
            }
        }

        if chunk.text.contains('\n') {
            result.push(Line::from(std::mem::take(&mut current)));
            remaining = width;
        }
    }

    if !current.is_empty() {
        result.push(Line::from(current));
    }
    if result.is_empty() {
        result.push(Line::default());
    }
    result
}

fn split_for_highlight(text: &str, query: &str, base_style: Style) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }
    let query_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in text_lower.match_indices(&query_lower as &str) {
        let end = start + query_lower.len();
        if !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
            || !text.is_char_boundary(last_end)
        {
            continue;
        }
        if start > last_end {
            spans.push(Span::styled(text[last_end..start].to_string(), base_style));
        }
        spans.push(Span::styled(
            text[start..end].to_string(),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        last_end = end;
    }
    if last_end < text.len() && text.is_char_boundary(last_end) {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }
    spans
}

fn take_prefix_by_width(text: &str, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    match text.char_indices().nth(width) {
        None => text.len(),
        Some((idx, _)) => idx,
    }
}

fn char_width(text: &str) -> usize {
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> MarkdownRenderOptions<'static> {
        MarkdownRenderOptions {
            label: "ASSIST   ",
            label_style: Style::default().fg(Color::Cyan),
            content_cols: 40,
            base_style: Style::default(),
            search_query: "",
            search_active: false,
        }
    }

    #[test]
    fn renders_headings_and_paragraphs() {
        let lines = render_markdown_block("# Overview\n\nHello `world`", &opts());
        let rendered: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.iter().any(|l| l.contains("Overview")));
        assert!(rendered.iter().any(|l| l.contains("Hello world")));
    }

    #[test]
    fn renders_bullet_lists() {
        let lines = render_markdown_block("- one\n- two", &opts());
        let rendered: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.iter().any(|l| l.contains("• one")));
        assert!(rendered.iter().any(|l| l.contains("• two")));
    }

    #[test]
    fn renders_code_fences() {
        let lines = render_markdown_block("```rust\nfn main() {}\n```", &opts());
        let rendered: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.iter().any(|l| l.contains("rust")));
        assert!(rendered.iter().any(|l| l.contains("fn main() {}")));
    }
}
