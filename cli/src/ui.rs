use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

pub use crate::LeaderboardEntry;

pub struct App {
    pub leaderboard: Vec<LeaderboardEntry>,
    pub scroll_offset: usize,
    pub should_quit: bool,
    pub submission_count: u64,
    pub miner_count: u64,
}

impl App {
    pub fn new() -> Self {
        Self {
            leaderboard: Vec::new(),
            scroll_offset: 0,
            should_quit: false,
            submission_count: 0,
            miner_count: 0,
        }
    }

    pub fn refresh(&mut self) {
        self.leaderboard = generate_mock_leaderboard();
        self.submission_count = 42;
        self.miner_count = self.leaderboard.len() as u64;
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn generate_mock_leaderboard() -> Vec<LeaderboardEntry> {
    vec![
        LeaderboardEntry {
            rank: 1,
            hotkey: "5GrwvaEF5zXb26Fz9rcQpDWS7hZkXhCuJDBu3qM9YxnU".to_string(),
            score: 0.9876,
            epoch: 127,
        },
        LeaderboardEntry {
            rank: 2,
            hotkey: "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694".to_string(),
            score: 0.9654,
            epoch: 127,
        },
        LeaderboardEntry {
            rank: 3,
            hotkey: "5FLSigC9HGRKVhB9NiE6M6hnHmdJq4SHhyf3o1p1eKPNm5".to_string(),
            score: 0.9432,
            epoch: 126,
        },
        LeaderboardEntry {
            rank: 4,
            hotkey: "5DAAnrj7VHTznn2AWBemMuyBwZWs9F1E6j43voXKk3mN".to_string(),
            score: 0.9210,
            epoch: 126,
        },
        LeaderboardEntry {
            rank: 5,
            hotkey: "5HGjWAeFDfFCWPsjFQfKCBnXW2SRcm2BXA3y3AGIUfHK9".to_string(),
            score: 0.8987,
            epoch: 125,
        },
        LeaderboardEntry {
            rank: 6,
            hotkey: "5CaCmg5sSYpS6tUNq4gMhLrM9J5YoU9YpiS9jHq9Y5GZ".to_string(),
            score: 0.8765,
            epoch: 125,
        },
        LeaderboardEntry {
            rank: 7,
            hotkey: "5CLRLgT7Z1kF3cYwPw3nH7MqXf9JqTLDBqG8Z4dLn6Y2c".to_string(),
            score: 0.8543,
            epoch: 124,
        },
        LeaderboardEntry {
            rank: 8,
            hotkey: "5Dxz1vB6eSf5pJWwy7vMhF3GqH9RjN2kL1mV8sBpX4T2n".to_string(),
            score: 0.8321,
            epoch: 124,
        },
        LeaderboardEntry {
            rank: 9,
            hotkey: "5F8YpB2jEo6tUjW6bTwTmN8zU5qH6iRsL4pE7zC9kX1m".to_string(),
            score: 0.8099,
            epoch: 123,
        },
        LeaderboardEntry {
            rank: 10,
            hotkey: "5GqZpK3iLrN7vYtS9fVcX8wB5jD6mE2hA4gR1pT3oU7k".to_string(),
            score: 0.7877,
            epoch: 123,
        },
    ]
}

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, chunks[0]);
    draw_leaderboard(frame, chunks[1], app);
    draw_footer(frame, chunks[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect) {
    let title = Line::from(vec![
        Span::styled(
            "Data Fabrication Challenge",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("━", Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled("Miner Leaderboard", Style::default().fg(Color::Yellow)),
    ]);

    let header = Paragraph::new(title).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(header, area);
}

fn draw_leaderboard(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec![
        Cell::from("Rank"),
        Cell::from("Hotkey"),
        Cell::from("Score"),
        Cell::from("Epoch"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let visible_rows = visible_row_count(area);
    let rows: Vec<Row> = app
        .leaderboard
        .iter()
        .skip(app.scroll_offset)
        .take(visible_rows)
        .map(|entry| {
            let rank_style = match entry.rank {
                1 => Style::default().fg(Color::Rgb(255, 215, 0)),
                2 => Style::default().fg(Color::LightYellow),
                3 => Style::default().fg(Color::Yellow),
                _ => Style::default().fg(Color::White),
            };

            let hotkey_display = truncate_str(&entry.hotkey, 16);
            let score_display = format!("{:.4}", entry.score);

            Row::new(vec![
                Cell::from(Span::styled(format!("#{}", entry.rank), rank_style)),
                Cell::from(hotkey_display),
                Cell::from(score_display),
                Cell::from(format!("#{}", entry.epoch)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(8),
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    let stats = format!(
        "Leaderboard ({} miners | {} submissions)",
        app.miner_count, app.submission_count
    );

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(stats)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(table, area);

    if app.leaderboard.is_empty() {
        draw_empty_message(frame, area, "No leaderboard data available");
    }
}

fn draw_footer(frame: &mut Frame, area: Rect, _app: &App) {
    let controls = Line::from(vec![
        Span::styled("[q]", Style::default().fg(Color::Yellow).bold()),
        Span::raw(" quit  "),
        Span::styled("[r]", Style::default().fg(Color::Yellow).bold()),
        Span::raw(" refresh  "),
        Span::styled("[↑/↓]", Style::default().fg(Color::Yellow).bold()),
        Span::raw(" scroll"),
    ]);

    let footer = Paragraph::new(controls).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Controls")
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(footer, area);
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

fn visible_row_count(area: Rect) -> usize {
    area.height.saturating_sub(4) as usize
}

fn draw_empty_message(frame: &mut Frame, area: Rect, message: &str) {
    let inner = centered_rect(60, 20, area);
    let text = Paragraph::new(message).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(text, inner);
}

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
