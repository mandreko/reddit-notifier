use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
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
            .map(|post| {
                // Format timestamp to be more readable
                let timestamp_short = post
                    .first_seen_at
                    .split('.')
                    .next()
                    .unwrap_or(&post.first_seen_at)
                    .replace('T', " ");

                Row::new(vec![
                    post.subreddit.clone(),
                    post.post_id.clone(),
                    timestamp_short,
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Min(15),
                Constraint::Min(10),
                Constraint::Min(20),
            ],
        )
        .header(
            Row::new(vec!["Subreddit", "Post ID", "First Seen"])
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
        "[←/→] Page  ".into(),
        "[f] Filter  ".into(),
        "[Esc] Back".into(),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[3]);
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
    if app.logs_state.filter_mode {
        handle_filter_mode(app, key).await
    } else {
        handle_list_mode(app, key).await
    }
}

async fn handle_list_mode(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Left => {
            app.logs_state.prev_page();
            load_logs(app).await?;
        }
        KeyCode::Right => {
            app.logs_state.next_page();
            load_logs(app).await?;
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
