use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
    Frame,
};

use crate::database;
use crate::models::NotifiedPostRow;
use crate::tui::app::{App, Screen};

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

pub async fn load_logs(app: &mut App) -> Result<()> {
    // Load available subreddits for filter
    let subs = database::list_subscriptions(&app.pool).await?;
    app.logs_state.available_subreddits = subs.iter().map(|s| s.subreddit.clone()).collect();

    // Load posts based on filter
    let offset = app.logs_state.current_page * PAGE_SIZE;
    let posts = if let Some(ref subreddit) = app.logs_state.filter_subreddit {
        database::list_notified_posts_by_subreddit(&app.pool, subreddit, PAGE_SIZE, offset).await?
    } else {
        database::list_notified_posts(&app.pool, PAGE_SIZE, offset).await?
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

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    if app.logs_state.filter_mode {
        render_filter_mode(frame, app, area);
    } else {
        render_list_mode(frame, app, area);

        // Show delete confirmation dialog if needed
        if let Some(post_id) = app.logs_state.confirm_delete {
            render_confirm_delete(frame, area, post_id);
        }
    }
}

fn render_list_mode(frame: &mut Frame, app: &App, area: Rect) {
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

                let prefix = if i == app.logs_state.selected_post {
                    ">"
                } else {
                    " "
                };
                let style = if i == app.logs_state.selected_post {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    prefix.to_string(),
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
        "[f] Filter  ".into(),
        "[Esc] Back".into(),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[3]);
}

fn render_confirm_delete(frame: &mut Frame, area: Rect, post_id: i64) {
    let popup_area = centered_rect(50, 30, area);
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

fn render_filter_mode(frame: &mut Frame, app: &App, area: Rect) {
    // Render list mode in background
    render_list_mode(frame, app, area);

    // Render filter popup
    let popup_area = centered_rect(60, 60, area);

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
        let prefix = if is_selected { "> " } else { "  " };
        items.push(
            ListItem::new(format!("{}{}", prefix, sub)).style(if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            }),
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

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

pub async fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    if app.logs_state.confirm_delete.is_some() {
        handle_confirm_delete_mode(app, key).await
    } else if app.logs_state.filter_mode {
        handle_filter_mode(app, key).await
    } else {
        handle_list_mode(app, key).await
    }
}

async fn handle_list_mode(app: &mut App, key: KeyEvent) -> Result<()> {
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
        KeyCode::Esc => {
            app.current_screen = Screen::MainMenu;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_filter_mode(app: &mut App, key: KeyEvent) -> Result<()> {
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

async fn handle_confirm_delete_mode(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(post_id) = app.logs_state.confirm_delete {
                database::delete_notified_post(&app.pool, post_id).await?;
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
