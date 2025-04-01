use std::io;

use crossterm::cursor;

use crossterm::event::Event as CrossTermEvent;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::execute;
use crossterm::style;

use crate::events::AppEvent;
use crate::events::Event;

/// Converts a crossterm event into a hashlife event
pub fn convert_event(event: CrossTermEvent) -> io::Result<Option<Event>> {
    let mut stdout = io::stdout();

    match event {
        CrossTermEvent::Key(key_event) => {
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
                } => Ok(Some(Event::AppEvent(AppEvent::Exit))),
                _ => Ok(None),
            }
        }
        CrossTermEvent::Resize(cols, rows) => {
            execute!(
                stdout,
                style::Print(format!("({cols}x{rows})")),
                cursor::MoveToNextLine(1)
            )?;

            Ok(None)
        }
        _ => Ok(None),
    }
}
