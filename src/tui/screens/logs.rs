use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
    Frame,
};

use crate::models::database::NotifiedPostRow;
use crate::services::DatabaseService;
use crate::tui::app::{App, Screen};
use crate::tui::widgets::common;

const PAGE_SIZE: i64 = 50;

pub struct LogsState {
    pub posts: Vec<NotifiedPostRow>,
    pub current_page: i64,
    pub total_count: usize,
    pub filter_subreddit: Option<String>,
    pub available_subreddits: Vec<String>,
    pub filter_mode: bool,
    pub filter_selected: usize,
    pub selected_post: usize,
    pub confirm_delete: Option<i64>, // ID of post to delete
    pub truncate_mode: bool,
    pub truncate_days_input: String,
    pub truncate_result: Option<String>, // Result message after truncate
}

impl Default for LogsState {
    fn default() -> Self {
        Self::new()
    }
}

impl LogsState {
    pub fn new() -> Self {
        Self {
            posts: Vec::new(),
            current_page: 0,
            total_count: 0,
            filter_subreddit: None,
            available_subreddits: Vec::new(),
            filter_mode: false,
            filter_selected: 0,
            selected_post: 0,
            confirm_delete: None,
            truncate_mode: false,
            truncate_days_input: "7".to_string(), // Default to 7 days
            truncate_result: None,
        }
    }

    pub fn next_post(&mut self) {
        if !self.posts.is_empty() {
            self.selected_post = (self.selected_post + 1).min(self.posts.len() - 1);
        }
    }

    pub fn prev_post(&mut self) {
        if self.selected_post > 0 {
            self.selected_post -= 1;
        }
    }

    pub fn total_pages(&self) -> i64 {
        ((self.total_count as i64 + PAGE_SIZE - 1) / PAGE_SIZE).max(1)
    }

    pub fn next_page(&mut self) {
        if self.current_page < self.total_pages() - 1 {
            self.current_page += 1;
        }
    }

    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
        }
    }

    pub fn next_filter(&mut self) {
        if !self.available_subreddits.is_empty() {
            self.filter_selected = (self.filter_selected + 1) % (self.available_subreddits.len() + 1);
        }
    }

    pub fn prev_filter(&mut self) {
        if !self.available_subreddits.is_empty() {
            if self.filter_selected > 0 {
                self.filter_selected -= 1;
            } else {
                self.filter_selected = self.available_subreddits.len();
            }
        }
    }
}

pub async fn load_logs<D: DatabaseService>(app: &mut App<D>) -> Result<()> {
    // Load available subreddits for filter
    let subs = app.db.list_subscriptions().await?;
    app.logs_state.available_subreddits = subs.iter().map(|s| s.subreddit.clone()).collect();

    // Load posts based on filter
    let offset = app.logs_state.current_page * PAGE_SIZE;
    let posts = if let Some(ref subreddit) = app.logs_state.filter_subreddit {
        app.db.list_notified_posts_by_subreddit(subreddit, PAGE_SIZE, offset).await?
    } else {
        app.db.list_notified_posts(PAGE_SIZE, offset).await?
    };

    // Estimate total count (not exact but good enough for pagination)
    app.logs_state.total_count = if posts.len() < PAGE_SIZE as usize {
        (offset + posts.len() as i64) as usize
    } else {
        ((app.logs_state.current_page + 2) * PAGE_SIZE) as usize
    };

    app.logs_state.posts = posts;
    Ok(())
}

pub fn render<D: DatabaseService>(frame: &mut Frame, app: &App<D>) {
    let area = frame.area();

    if app.logs_state.filter_mode {
        render_filter_mode(frame, app, area);
    } else {
        render_list_mode(frame, app, area);

        // Show delete confirmation dialog if needed
        if let Some(post_id) = app.logs_state.confirm_delete {
            render_confirm_delete(frame, area, post_id);
        }

        // Show truncate dialog if needed
        if app.logs_state.truncate_mode {
            render_truncate_dialog(frame, app, area);
        }
    }
}

fn render_list_mode<D: DatabaseService>(frame: &mut Frame, app: &App<D>, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    // Title
    let title = Paragraph::new("Notification History")
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        );
    frame.render_widget(title, chunks[0]);

    // Filter display
    let filter_text = if let Some(ref sub) = app.logs_state.filter_subreddit {
        format!("Filter: {} (press 'f' to change)", sub)
    } else {
        "Filter: All Subreddits (press 'f' to filter)".to_string()
    };
    let filter = Paragraph::new(filter_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(filter, chunks[1]);

    // Table
    if app.logs_state.posts.is_empty() {
        let empty = Paragraph::new("No notification history yet.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(empty, chunks[2]);
    } else {
        let rows: Vec<Row> = app
            .logs_state
            .posts
            .iter()
            .enumerate()
            .map(|(i, post)| {
                // Format timestamp to be more readable
                let timestamp_short = post
                    .first_seen_at
                    .split('.')
                    .next()
                    .unwrap_or(&post.first_seen_at)
                    .replace('T', " ");

                let is_selected = i == app.logs_state.selected_post;
                let (prefix, style) = common::selection_style(is_selected);

                Row::new(vec![
                    prefix,
                    post.subreddit.clone(),
                    post.post_id.clone(),
                    timestamp_short,
                ])
                .style(style)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(2),      // Selection marker
                Constraint::Percentage(25), // Subreddit
                Constraint::Percentage(30), // Post ID
                Constraint::Percentage(45), // First Seen timestamp
            ],
        )
        .header(
            Row::new(vec!["", "Subreddit", "Post ID", "First Seen"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    "Page {} of {}",
                    app.logs_state.current_page + 1,
                    app.logs_state.total_pages()
                )),
        );

        frame.render_widget(table, chunks[2]);
    }

    // Help text
    let help = Paragraph::new(Line::from(vec![
        "[↑/↓] Navigate  ".into(),
        "[←/→] Page  ".into(),
        "[d] Delete  ".into(),
        "[t] Truncate  ".into(),
        "[f] Filter  ".into(),
        "[Esc] Back".into(),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[3]);
}

fn render_truncate_dialog<D: DatabaseService>(frame: &mut Frame, app: &App<D>, area: Rect) {
    let popup_area = common::centered_rect(60, 40, area);

    let result_text = if let Some(ref result) = app.logs_state.truncate_result {
        vec![
            Line::from(""),
            Line::from(result.clone()).alignment(Alignment::Center).style(Style::default().fg(Color::Green)),
            Line::from(""),
            Line::from("Press any key to close").alignment(Alignment::Center).style(Style::default().fg(Color::Gray)),
        ]
    } else {
        vec![
            Line::from(""),
            Line::from("Delete posts older than N days").alignment(Alignment::Center),
            Line::from("").alignment(Alignment::Center),
            Line::from(vec![
                Span::raw("Days to keep: "),
                Span::styled(
                    app.logs_state.truncate_days_input.as_str(),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                ),
                Span::styled("█", Style::default().fg(Color::Yellow)),
            ])
            .alignment(Alignment::Center),
            Line::from(""),
            Line::from("Note: Only notifies on posts within 24 hours").alignment(Alignment::Center).style(Style::default().fg(Color::Gray)),
            Line::from("so older records won't trigger duplicates").alignment(Alignment::Center).style(Style::default().fg(Color::Gray)),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Enter]", Style::default().fg(Color::Yellow)),
                Span::raw(" Truncate  "),
                Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
                Span::raw(" Cancel"),
            ])
            .alignment(Alignment::Center),
        ]
    };

    let popup = Paragraph::new(result_text)
        .block(
            Block::default()
                .title("Truncate Old Posts")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        );

    frame.render_widget(ratatui::widgets::Clear, popup_area);
    frame.render_widget(popup, popup_area);
}

fn render_confirm_delete(frame: &mut Frame, area: Rect, post_id: i64) {
    let popup_area = common::centered_rect(50, 30, area);
    let text = format!("Delete log entry #{}?", post_id);
    let popup = Paragraph::new(vec![
        Line::from(""),
        Line::from(text).alignment(Alignment::Center),
        Line::from("").alignment(Alignment::Center),
        Line::from(vec![
            Span::raw("["),
            Span::styled("y", Style::default().fg(Color::Yellow)),
            Span::raw("] Yes    ["),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw("] No"),
        ])
        .alignment(Alignment::Center),
    ])
    .block(
        Block::default()
            .title("Confirm Delete")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Red)),
    );

    frame.render_widget(ratatui::widgets::Clear, popup_area);
    frame.render_widget(popup, popup_area);
}

fn render_filter_mode<D: DatabaseService>(frame: &mut Frame, app: &App<D>, area: Rect) {
    // Render list mode in background
    render_list_mode(frame, app, area);

    // Render filter popup
    let popup_area = common::centered_rect(60, 60, area);

    let mut items = vec![ListItem::new(
        if app.logs_state.filter_selected == 0 {
            "> All Subreddits"
        } else {
            "  All Subreddits"
        },
    )
    .style(if app.logs_state.filter_selected == 0 {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    })];

    for (i, sub) in app.logs_state.available_subreddits.iter().enumerate() {
        let is_selected = app.logs_state.filter_selected == i + 1;
        let (prefix, style) = common::selection_style(is_selected);
        items.push(
            ListItem::new(format!("{}{}", prefix, sub)).style(style),
        );
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Select Subreddit Filter")
            .style(Style::default().bg(Color::Black)),
    );

    frame.render_widget(ratatui::widgets::Clear, popup_area);
    frame.render_widget(list, popup_area);
}

pub async fn handle_key<D: DatabaseService>(app: &mut App<D>, key: KeyEvent) -> Result<()> {
    if app.logs_state.truncate_mode {
        handle_truncate_mode(app, key).await
    } else if app.logs_state.confirm_delete.is_some() {
        handle_confirm_delete_mode(app, key).await
    } else if app.logs_state.filter_mode {
        handle_filter_mode(app, key).await
    } else {
        handle_list_mode(app, key).await
    }
}

async fn handle_list_mode<D: DatabaseService>(app: &mut App<D>, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Up => {
            app.logs_state.prev_post();
        }
        KeyCode::Down => {
            app.logs_state.next_post();
        }
        KeyCode::Left => {
            app.logs_state.prev_page();
            app.logs_state.selected_post = 0;
            load_logs(app).await?;
        }
        KeyCode::Right => {
            app.logs_state.next_page();
            app.logs_state.selected_post = 0;
            load_logs(app).await?;
        }
        KeyCode::Char('d') => {
            if !app.logs_state.posts.is_empty() {
                let post_id = app.logs_state.posts[app.logs_state.selected_post].id;
                app.logs_state.confirm_delete = Some(post_id);
            }
        }
        KeyCode::Char('f') => {
            app.logs_state.filter_mode = true;
        }
        KeyCode::Char('t') => {
            app.logs_state.truncate_mode = true;
            app.logs_state.truncate_result = None;
        }
        KeyCode::Esc => {
            app.current_screen = Screen::MainMenu;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_truncate_mode<D: DatabaseService>(app: &mut App<D>, key: KeyEvent) -> Result<()> {
    // If showing result, any key closes the dialog
    if app.logs_state.truncate_result.is_some() {
        app.logs_state.truncate_mode = false;
        app.logs_state.truncate_result = None;
        app.logs_state.current_page = 0;
        load_logs(app).await?;
        return Ok(());
    }

    match key.code {
        KeyCode::Char(c) if c.is_ascii_digit() => {
            // Allow max 3 digits (up to 999 days)
            if app.logs_state.truncate_days_input.len() < 3 {
                app.logs_state.truncate_days_input.push(c);
            }
        }
        KeyCode::Backspace => {
            app.logs_state.truncate_days_input.pop();
        }
        KeyCode::Enter => {
            // Parse and execute truncate
            if let Ok(days) = app.logs_state.truncate_days_input.parse::<i64>() {
                if days > 0 {
                    match app.db.cleanup_old_posts(days).await {
                        Ok(deleted) => {
                            let msg = format!("Deleted {} post(s) older than {} day(s)", deleted, days);
                            app.logs_state.truncate_result = Some(msg);
                        }
                        Err(e) => {
                            let msg = format!("Error: {}", e);
                            app.logs_state.truncate_result = Some(msg);
                        }
                    }
                } else {
                    app.logs_state.truncate_result = Some("Days must be greater than 0".to_string());
                }
            } else {
                app.logs_state.truncate_result = Some("Invalid number".to_string());
            }
        }
        KeyCode::Esc => {
            app.logs_state.truncate_mode = false;
            app.logs_state.truncate_days_input = "7".to_string(); // Reset to default
        }
        _ => {}
    }
    Ok(())
}

async fn handle_filter_mode<D: DatabaseService>(app: &mut App<D>, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Up => {
            app.logs_state.prev_filter();
        }
        KeyCode::Down => {
            app.logs_state.next_filter();
        }
        KeyCode::Enter => {
            // Apply filter
            if app.logs_state.filter_selected == 0 {
                app.logs_state.filter_subreddit = None;
            } else {
                let sub = app.logs_state.available_subreddits
                    [app.logs_state.filter_selected - 1]
                    .clone();
                app.logs_state.filter_subreddit = Some(sub);
            }
            app.logs_state.current_page = 0;
            app.logs_state.filter_mode = false;
            load_logs(app).await?;
        }
        KeyCode::Esc => {
            app.logs_state.filter_mode = false;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_confirm_delete_mode<D: DatabaseService>(app: &mut App<D>, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(post_id) = app.logs_state.confirm_delete {
                app.db.delete_notified_post(post_id).await?;
                app.logs_state.confirm_delete = None;
                app.logs_state.selected_post = 0;
                load_logs(app).await?;
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.logs_state.confirm_delete = None;
        }
        _ => {}
    }
    Ok(())
}
