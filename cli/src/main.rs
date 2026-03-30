use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use data_cli::ui;
use data_cli::ui::App;

fn main() -> Result<()> {
    let mut terminal = ratatui::try_init()?;
    let result = run(&mut terminal);
    ratatui::try_restore()?;
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let mut app = App::new();
    app.refresh();

    let tick_rate = Duration::from_secs(10);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_default();

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => {
                            app.should_quit = true;
                        }
                        KeyCode::Char('r') => {
                            app.refresh();
                            last_tick = Instant::now();
                        }
                        KeyCode::Up => {
                            app.scroll_up();
                        }
                        KeyCode::Down => {
                            app.scroll_down();
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }

        if last_tick.elapsed() >= tick_rate {
            app.refresh();
            last_tick = Instant::now();
        }
    }

    Ok(())
}
