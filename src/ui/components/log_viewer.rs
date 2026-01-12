use crate::app::{App, TimestampDisplayMode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use regex::Regex;

/// Parse and format log line based on timestamp display mode
fn process_log_line(line: &str, mode: &TimestampDisplayMode) -> String {
    // First, strip GitLab CI log prefixes (00E, 00O, section markers, etc.)
    let stripped_line = strip_gitlab_prefixes(line);

    // Regex to match ISO timestamps at the start of the line
    // Matches patterns like: 2024-01-15T10:30:45.123Z or 2024-01-15T10:30:45+00:00
    let re = Regex::new(r"^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?\s+").unwrap();

    match mode {
        TimestampDisplayMode::Hidden => {
            // Strip timestamp completely
            re.replace(&stripped_line, "").to_string()
        }
        TimestampDisplayMode::DateOnly => {
            // Show only the date part
            if let Some(caps) = re.captures(&stripped_line) {
                let date = &caps[1];
                let rest = &stripped_line[caps.get(0).unwrap().end()..];
                format!("{} {}", date, rest)
            } else {
                stripped_line
            }
        }
        TimestampDisplayMode::Full => {
            // Show date and time (but not milliseconds/timezone)
            if let Some(caps) = re.captures(&stripped_line) {
                let date = &caps[1];
                let time = &caps[2];
                let rest = &stripped_line[caps.get(0).unwrap().end()..];
                format!("{} {} {}", date, time, rest)
            } else {
                stripped_line
            }
        }
    }
}

/// Strip GitLab CI log prefixes like 00E, 00O, section markers, etc.
fn strip_gitlab_prefixes(line: &str) -> String {
    // GitLab uses special prefixes:
    // - \x00[0-9A-F]{2} (null byte + 2 hex chars) for control codes
    // - section_start:timestamp:name for collapsible sections
    // - section_end:timestamp:name for section endings

    let mut result = line;

    // Strip null byte prefixes like \x0000E, \x0000O, etc.
    // These show up as "00E", "00O" in the text
    if result.starts_with("\x00") && result.len() >= 3 {
        result = &result[3..]; // Skip null byte + 2 hex chars
    } else if result.starts_with("00") && result.len() >= 3 {
        // Sometimes they appear without the null byte
        let third_char = result.chars().nth(2);
        if matches!(third_char, Some('E') | Some('O') | Some('0'..='9') | Some('A'..='F') | Some('a'..='f')) {
            result = &result[3..];
        }
    }

    // Strip section markers
    if result.starts_with("section_start:") || result.starts_with("section_end:") {
        // These lines are typically used for collapsible sections, skip them entirely
        return String::new();
    }

    result.to_string()
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    // Calculate the log viewer area (90% width, 90% height, centered)
    let log_area = centered_rect(90, 90, area);

    // Clear the background to prevent rendering artifacts
    f.render_widget(Clear, log_area);

    let log_content = match &app.log_content {
        Some(content) => content,
        None => {
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Job Log")
                .style(Style::default().fg(Color::Gray));
            f.render_widget(block, log_area);
            return;
        }
    };

    let job_name = app
        .log_job_name
        .as_deref()
        .unwrap_or("Unknown Job");

    // Process timestamps and parse ANSI codes, converting to ratatui Lines
    let lines: Vec<Line> = log_content
        .lines()
        .map(|line| {
            // First, process the timestamp based on display mode
            let processed_line = process_log_line(line, &app.timestamp_mode);

            // Then parse ANSI escape sequences
            match ansi_to_tui::IntoText::into_text(&processed_line) {
                Ok(text) => {
                    // Convert ratatui Text to Line
                    if text.lines.is_empty() {
                        Line::from("")
                    } else {
                        text.lines[0].clone()
                    }
                }
                Err(_) => {
                    // If parsing fails, show raw text
                    Line::from(processed_line)
                }
            }
        })
        .collect();

    // Calculate visible range based on scroll offset
    let content_height = log_area.height.saturating_sub(2) as usize; // Account for borders
    let total_lines = lines.len();
    let max_offset = total_lines.saturating_sub(content_height);
    let scroll_offset = app.log_scroll_offset.min(max_offset);

    // Get visible lines
    let visible_lines: Vec<Line> = if total_lines > 0 {
        let start = scroll_offset;
        let end = (scroll_offset + content_height).min(total_lines);
        lines[start..end].to_vec()
    } else {
        vec![Line::from("(empty log)")]
    };

    let scroll_indicator = if total_lines > content_height {
        format!(
            " [{}/{}] ",
            scroll_offset + 1,
            max_offset + 1
        )
    } else {
        String::new()
    };

    let timestamp_indicator = match &app.timestamp_mode {
        TimestampDisplayMode::Hidden => "[Timestamps: Hidden]",
        TimestampDisplayMode::DateOnly => "[Timestamps: Date]",
        TimestampDisplayMode::Full => "[Timestamps: Full]",
    };

    // Build search indicator
    let search_indicator = if !app.search_results.is_empty() {
        format!(
            " [Match {}/{}]",
            app.current_search_result + 1,
            app.search_results.len()
        )
    } else if !app.search_query.is_empty() && !app.is_searching {
        " [No matches]".to_string()
    } else {
        String::new()
    };

    let title = format!(
        "Job Log: {}{}{}{} (q/Esc close, / search, n/N next/prev, t time)",
        job_name,
        if scroll_indicator.is_empty() { " " } else { &scroll_indicator },
        timestamp_indicator,
        search_indicator
    );

    // If searching, show search input bar at the bottom
    let (render_area, search_area) = if app.is_searching {
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(log_area);
        (chunks[0], Some(chunks[1]))
    } else {
        (log_area, None)
    };

    let paragraph = Paragraph::new(visible_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(Style::default()),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, render_area);

    // Render search input bar if in search mode
    if let Some(search_area) = search_area {
        let search_line = Line::from(vec![
            Span::raw("Search: "),
            Span::styled(
                &app.search_query,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "█",
                Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK),
            ),
        ]);

        let search_paragraph = Paragraph::new(search_line).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Enter to search, Esc to cancel ")
                .style(Style::default().fg(Color::Cyan)),
        );

        f.render_widget(search_paragraph, search_area);
    }
}
