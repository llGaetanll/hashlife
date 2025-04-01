use std::error::Error;
use std::io;
use std::thread;
use std::time;
use std::time::Duration;

use crossterm::cursor;
use crossterm::event;

use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::execute;
use crossterm::style;
use crossterm::terminal;

const FRAMERATE: u32 = 30;
const FRAMETIME: time::Duration =
    time::Duration::from_millis(((1f64 / FRAMERATE as f64) * 1_000f64) as u64);

/// Returns true if app should exit
fn handle_event(event: Event) -> io::Result<bool> {
    let mut stdout = io::stdout();

    match event {
        Event::Key(key_event) => {
            execute!(
                stdout,
                style::Print(format!("{:?}", key_event)),
                cursor::MoveToNextLine(1)
            )?;

            match key_event {
                KeyEvent {
                    code: KeyCode::Char('q'),
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => Ok(true),
                _ => Ok(false),
            }
        }
        Event::Resize(cols, rows) => {
            execute!(
                stdout,
                style::Print(format!("({cols}x{rows})")),
                cursor::MoveToNextLine(1)
            )?;

            Ok(false)
        }
        _ => Ok(false),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();

    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0),
    )?;

    loop {
        let t = time::SystemTime::now();

        // Poll event for as long as FRAMETIME
        let dt = if event::poll(FRAMETIME)? {
            let event = event::read()?;

            if handle_event(event)? {
                break;
            }

            t.elapsed()?
        } else {
            Duration::ZERO
        };

        let time_left = FRAMETIME - dt;
        thread::sleep(time_left);
    }

    terminal::disable_raw_mode()?;

    Ok(())
}
