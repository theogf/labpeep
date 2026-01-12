use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render(f: &mut Frame, area: Rect) {
    // Calculate the popup area (centered)
    let popup_area = centered_rect(60, 70, area);

    // Clear the background
    f.render_widget(Clear, popup_area);

    // Create the help content
    let help_text = vec![
        Line::from(vec![Span::styled(
            "Keyboard Controls",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("q", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" or "),
            Span::styled("Ctrl+C", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" - Quit the application"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("?", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" - Show/hide this help"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation:",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("←/→", Style::default().fg(Color::Cyan)),
            Span::raw(" or "),
            Span::styled("h/l", Style::default().fg(Color::Cyan)),
            Span::raw(" - Switch between MR tabs"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("↑/↓", Style::default().fg(Color::Cyan)),
            Span::raw(" or "),
            Span::styled("k/j", Style::default().fg(Color::Cyan)),
            Span::raw(" - Navigate jobs"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[/]", Style::default().fg(Color::Cyan)),
            Span::raw(" - Switch between pipelines"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Actions:",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Enter", Style::default().fg(Color::Cyan)),
            Span::raw(" - View selected job log"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("r", Style::default().fg(Color::Cyan)),
            Span::raw(" - Refresh all data"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("d", Style::default().fg(Color::Cyan)),
            Span::raw(" - Remove current MR from tracking"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("c", Style::default().fg(Color::Cyan)),
            Span::raw(" - Toggle between jobs and comments view"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("o", Style::default().fg(Color::Cyan)),
            Span::raw(" - Open current MR in browser"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Log Viewer:",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("q/Esc", Style::default().fg(Color::Cyan)),
            Span::raw(" - Close log viewer"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("↑/↓", Style::default().fg(Color::Cyan)),
            Span::raw(" or "),
            Span::styled("k/j", Style::default().fg(Color::Cyan)),
            Span::raw(" - Scroll log"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("PgUp/PgDn", Style::default().fg(Color::Cyan)),
            Span::raw(", "),
            Span::styled("Home/End", Style::default().fg(Color::Cyan)),
            Span::raw(" - Jump in log"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("t", Style::default().fg(Color::Cyan)),
            Span::raw(" - Toggle timestamp display (hidden/date/full)"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("/", Style::default().fg(Color::Cyan)),
            Span::raw(" - Start search"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("n/N", Style::default().fg(Color::Cyan)),
            Span::raw(" - Next/previous search result"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Status Indicators:",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("✓", Style::default().fg(Color::Green)),
            Span::raw(" - Success"),
            Span::raw("  "),
            Span::styled("✗", Style::default().fg(Color::Red)),
            Span::raw(" - Failed"),
            Span::raw("  "),
            Span::styled("⟳", Style::default().fg(Color::Yellow)),
            Span::raw(" - Running"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press Esc or ? to close",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(" Help ")
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    f.render_widget(paragraph, popup_area);
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
