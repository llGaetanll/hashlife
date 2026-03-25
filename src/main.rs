mod action;
mod keymap;

use std::io::{self, Write};
use std::time::Duration;

use clap::{Parser, ValueEnum};
use crossterm::{cursor, event, execute, style, terminal};

use std::path::Path;

use hashlife::camera::{Camera, CameraBlock, CameraBraille};
use hashlife::rle_data::RleBuffer;
use hashlife::rle_file;
use hashlife::rule_set::RuleSet;
use hashlife::world::World;

use action::{Action, AppAction, CameraAction};

#[derive(Clone, ValueEnum)]
enum CameraArg {
    Braille,
    Block,
}

#[derive(Parser)]
struct Args {
    /// Path to an RLE file
    path: String,

    /// Camera rendering mode
    #[arg(long, default_value = "braille")]
    camera: CameraArg,
}

fn load_rle(path: &str) -> anyhow::Result<(World, RuleSet)> {
    let bytes = std::fs::read(path)?;
    let mut buf = RleBuffer::new();
    let header = rle_file::read_rle(&bytes, &mut buf)?;
    let set = header.set.clone();
    Ok((World::from_rle(header.set, buf), set))
}

fn make_camera(ty: &CameraArg, cols: u16, rows: u16) -> Box<dyn Camera> {
    // Reserve 1 row for the status bar
    let cam_rows = rows.saturating_sub(1).max(1);
    match ty {
        CameraArg::Braille => Box::new(CameraBraille::new(cols, cam_rows)),
        CameraArg::Block => Box::new(CameraBlock::new(cols, cam_rows)),
    }
}

struct App {
    playing: bool,
    tick: Duration,
    /// Last drag position for computing deltas, None when not dragging
    drag_from: Option<(u16, u16)>,
    iteration: i128,
    rule_set: RuleSet,
    file_name: String,
}

impl App {
    fn new(rule_set: RuleSet, file_name: String) -> Self {
        Self {
            playing: false,
            tick: Duration::from_millis(50),
            drag_from: None,
            iteration: 0,
            rule_set,
            file_name,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let (mut world, rule_set) = load_rle(&args.path)?;
    world.grow(2);
    let (cols, rows) = terminal::size()?;
    let mut cam = make_camera(&args.camera, cols, rows);

    let file_name = Path::new(&args.path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| args.path.clone());

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::Hide,
        event::EnableMouseCapture,
    )?;

    let mut app = App::new(rule_set, file_name);

    // Initial draw
    cam.reset();
    cam.draw(&world);
    render(&mut stdout, &app, cam.as_mut(), &world, rows)?;

    let result = run(&mut stdout, &mut app, cam.as_mut(), &mut world);

    execute!(
        stdout,
        event::DisableMouseCapture,
        terminal::LeaveAlternateScreen,
        cursor::Show,
    )?;
    terminal::disable_raw_mode()?;

    result
}

fn run(
    stdout: &mut io::Stdout,
    app: &mut App,
    cam: &mut dyn Camera,
    world: &mut World,
) -> anyhow::Result<()> {
    let mut rows = terminal::size()?.1;

    loop {
        // When playing, poll with a timeout so we can step automatically.
        // When paused, block until an event arrives.
        let has_event = if app.playing {
            event::poll(app.tick)?
        } else {
            // Block forever
            event::poll(Duration::from_secs(3600))?
        };

        if has_event {
            let event = event::read()?;
            let Some(action) = keymap::resolve(event) else {
                continue;
            };

            match action {
                Action::App(AppAction::Quit) => break,
                Action::App(AppAction::TogglePlay) => {
                    app.playing = !app.playing;
                }
                Action::App(AppAction::Step) => {
                    app.playing = false;
                    let k = if world.k() == 0 { world.depth - 3 } else { world.k() - 1 };
                    world.next();
                    app.iteration += 1i128 << k;
                }
                Action::App(AppAction::IncreaseK) => {
                    world.set_k(world.k() + 1);
                }
                Action::App(AppAction::DecreaseK) => {
                    if world.k() > 1 {
                        world.set_k(world.k() - 1);
                    }
                }

                Action::Camera(a) => match a {
                    CameraAction::MoveUp => cam.move_up(1),
                    CameraAction::MoveDown => cam.move_down(1),
                    CameraAction::MoveLeft => cam.move_left(1),
                    CameraAction::MoveRight => cam.move_right(1),
                    CameraAction::ZoomIn => cam.zoom_in(),
                    CameraAction::ZoomOut => cam.zoom_out(),
                    CameraAction::ZoomInAt { col, row } => cam.zoom_in_at(col, row),
                    CameraAction::ZoomOutAt { col, row } => cam.zoom_out_at(col, row),
                    CameraAction::Drag { col, row } => {
                        if let Some((prev_col, prev_row)) = app.drag_from {
                            let (ppc_x, ppc_y) = cam.pixels_per_char();
                            let dx = (col as i32 - prev_col as i32) * ppc_x as i32;
                            let dy = (row as i32 - prev_row as i32) * ppc_y as i32;
                            if dx > 0 { cam.move_left(dx as u64); }
                            if dx < 0 { cam.move_right((-dx) as u64); }
                            if dy > 0 { cam.move_up(dy as u64); }
                            if dy < 0 { cam.move_down((-dy) as u64); }
                        }
                        app.drag_from = Some((col, row));
                    }
                    CameraAction::DragEnd => {
                        app.drag_from = None;
                    }
                    CameraAction::ResetView => cam.reset_view(),
                    CameraAction::Resize { cols, rows: r } => {
                        rows = r;
                        cam.resize(cols, r.saturating_sub(1).max(1));
                    }
                },
            }
        } else if app.playing {
            // Tick expired — advance the simulation
            let k = if world.k() == 0 { world.depth - 3 } else { world.k() - 1 };
            world.next();
            app.iteration += 1i128 << k;
        }

        cam.reset();
        cam.draw(world);
        render(stdout, app, cam, world, rows)?;
    }

    Ok(())
}

fn render(
    stdout: &mut io::Stdout,
    app: &App,
    cam: &mut dyn Camera,
    world: &World,
    rows: u16,
) -> anyhow::Result<()> {
    let s = cam.render();

    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0),
    )?;

    for line in s.lines() {
        execute!(stdout, style::Print(line), cursor::MoveToNextLine(1))?;
    }

    // Status bar on the last row
    let (x, y) = cam.position();
    let cols = terminal::size()?.0 as usize;
    let left = format!("{}  {}  x: {}  y: {}  i: {}", app.rule_set, app.file_name, x, y, app.iteration);
    let right = format!("s: {}  d: {}  k: {}", cam.scale(), world.depth, world.k());
    let padding = cols.saturating_sub(left.len() + right.len());
    let status = format!("{}{:padding$}{}", left, "", right);

    execute!(
        stdout,
        cursor::MoveTo(0, rows - 1),
        style::Print(status),
    )?;

    stdout.flush()?;
    Ok(())
}
