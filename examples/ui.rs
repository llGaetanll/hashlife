use std::error::Error;
use std::io;
use std::thread;
use std::time;
use std::time::Duration;

use crossterm::cursor;
use crossterm::event;

use crossterm::event::Event as CtEvent;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::execute;
use crossterm::style;
use crossterm::terminal;
use hashlife::camera::Camera;
use hashlife::cell::Cell;
use hashlife::world::World;

const FRAMERATE: u32 = 120;
const FRAMETIME: time::Duration =
    time::Duration::from_millis(((1f64 / FRAMERATE as f64) * 1_000f64) as u64);

enum Event {
    ZoomIn,
    ZoomOut,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    CamResize { cols: u16, rows: u16 },
    ResetView,
    Exit,
}

const DUMMY_LEAF: Cell = Cell::leaf(
    0b0010_0001_0111_0000,
    0b0000_0100_0101_0110,
    0b0110_1010_0010_0000,
    0b0000_1110_1000_0100,
);

// See: https://conwaylife.com/wiki/Rulestring
const LIFE_RULES: &str = "b3s23";

fn setup_world(depth: u8) -> World {
    let mut world = World::new(0, LIFE_RULES).unwrap();

    world.buf.pop();
    world.buf.push(DUMMY_LEAF);

    let n = world.buf.len();

    for i in 0..depth {
        let i = n + i as usize - 1;
        world.buf.push(Cell::new(i, i, i, i));
    }

    world.root = world.buf.len() - 1;
    world.depth = depth;

    world
}

/// Returns true if app should exit
fn handle_event(event: CtEvent) -> io::Result<Option<Event>> {
    match event {
        CtEvent::Key(key_event) => match key_event {
            KeyEvent {
                code: KeyCode::Char('q'),
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Ok(Some(Event::Exit)),
            KeyEvent {
                code: KeyCode::Char('J'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Ok(Some(Event::ZoomOut)),
            KeyEvent {
                code: KeyCode::Char('K'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Ok(Some(Event::ZoomIn)),
            KeyEvent {
                code: KeyCode::Char('h'),
                ..
            } => Ok(Some(Event::MoveLeft)),
            KeyEvent {
                code: KeyCode::Char('j'),
                ..
            } => Ok(Some(Event::MoveDown)),
            KeyEvent {
                code: KeyCode::Char('k'),
                ..
            } => Ok(Some(Event::MoveUp)),
            KeyEvent {
                code: KeyCode::Char('l'),
                ..
            } => Ok(Some(Event::MoveRight)),
            KeyEvent {
                code: KeyCode::Char('0'),
                ..
            } => Ok(Some(Event::ResetView)),
            _ => Ok(None),
        },
        CtEvent::Resize(cols, rows) => Ok(Some(Event::CamResize { cols, rows })),
        _ => Ok(None),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();

    // Get the width and height of the terminal
    let (cols, rows) = terminal::size()?;

    let mut cam = Camera::new(cols, rows);
    let world = setup_world(6);

    loop {
        let t = time::SystemTime::now();

        // Poll event for as long as FRAMETIME
        let (dt, event) = if event::poll(FRAMETIME)? {
            let event = event::read()?;

            let event = handle_event(event)?;
            let dt = t.elapsed()?;

            (dt, event)
        } else {
            (Duration::ZERO, None)
        };

        match event {
            None => {}
            Some(Event::Exit) => break,
            Some(Event::ZoomIn) => cam.zoom_in(),
            Some(Event::ZoomOut) => cam.zoom_out(),
            Some(Event::MoveUp) => cam.move_up(),
            Some(Event::MoveDown) => cam.move_down(),
            Some(Event::MoveLeft) => cam.move_left(),
            Some(Event::MoveRight) => cam.move_right(),
            Some(Event::CamResize { cols, rows }) => {
                cam.resize(cols, rows);
            }
            Some(Event::ResetView) => {
                cam.reset_view();
            }
        }

        cam.reset();
        cam.draw(&world);
        let s = cam.render();

        execute!(
            stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0),
        )?;

        for line in s.lines() {
            execute!(
                stdout,
                style::Print(line),
                crossterm::cursor::MoveToNextLine(1)
            )?;
        }

        let time_left = FRAMETIME.saturating_sub(dt);
        thread::sleep(time_left);
    }

    terminal::disable_raw_mode()?;

    Ok(())
}
