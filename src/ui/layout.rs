use crate::app::{App, AppMode};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use super::components;

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // MR Tabs
            Constraint::Length(10), // Pipeline list
            Constraint::Min(10),    // Jobs table
        ])
        .split(f.area());

    components::mr_tabs::render(f, app, chunks[0]);
    components::pipeline_list::render(f, app, chunks[1]);

    // Toggle between jobs and comments view
    if app.is_viewing_comments() {
        components::comments_list::render(f, app, chunks[2]);
    } else {
        components::job_list::render(f, app, chunks[2]);
    }

    // Render help popup on top if in help mode
    if app.mode == AppMode::ShowingHelp {
        components::help::render(f, f.area());
    }

    // Render log viewer on top if in log viewing mode
    if app.mode == AppMode::ViewingLog {
        components::log_viewer::render(f, app, f.area());
    }
}
