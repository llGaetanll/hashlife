mod action;
mod keymap;

use std::io::{self, Write};

use clap::{Parser, ValueEnum};
use crossterm::{cursor, event, execute, style, terminal};

use hashlife::camera::{Camera, CameraBlock, CameraBraille};
use hashlife::rle_data::RleBuffer;
use hashlife::rle_file;
use hashlife::world::World;

use action::{Action, AppAction, CameraAction, WorldAction};

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

fn load_rle(path: &str) -> anyhow::Result<World> {
    let bytes = std::fs::read(path)?;
    let mut buf = RleBuffer::new();
    let header = rle_file::read_rle(&bytes, &mut buf)?;
    Ok(World::from_rle(header.set, buf))
}

fn make_camera(ty: &CameraArg, cols: u16, rows: u16) -> Box<dyn Camera> {
    // Reserve 1 row for the status bar
    let cam_rows = rows.saturating_sub(1).max(1);
    match ty {
        CameraArg::Braille => Box::new(CameraBraille::new(cols, cam_rows)),
        CameraArg::Block => Box::new(CameraBlock::new(cols, cam_rows)),
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut world = load_rle(&args.path)?;
    world.grow(2);
    let (cols, rows) = terminal::size()?;
    let mut cam = make_camera(&args.camera, cols, rows);

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::Hide,
        event::EnableMouseCapture,
    )?;

    // Initial draw
    cam.reset();
    cam.draw(&world);
    render(&mut stdout, cam.as_mut(), &world, rows)?;

    let result = run(&mut stdout, cam.as_mut(), &mut world);

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
    cam: &mut dyn Camera,
    world: &mut World,
) -> anyhow::Result<()> {
    let mut rows = terminal::size()?.1;

    loop {
        let event = event::read()?;
        let Some(action) = keymap::resolve(event) else {
            continue;
        };

        match action {
            Action::App(AppAction::Quit) => break,

            Action::Camera(a) => {
                match a {
                    CameraAction::MoveUp => cam.move_up(1),
                    CameraAction::MoveDown => cam.move_down(1),
                    CameraAction::MoveLeft => cam.move_left(1),
                    CameraAction::MoveRight => cam.move_right(1),
                    CameraAction::ZoomIn => cam.zoom_in(),
                    CameraAction::ZoomOut => cam.zoom_out(),
                    CameraAction::ZoomInAt { col, row } => cam.zoom_in_at(col, row),
                    CameraAction::ZoomOutAt { col, row } => cam.zoom_out_at(col, row),
                    CameraAction::ResetView => cam.reset_view(),
                    CameraAction::Resize { cols, rows: r } => {
                        rows = r;
                        cam.resize(cols, r.saturating_sub(1).max(1));
                    }
                }
            }

            Action::World(a) => {
                match a {
                    WorldAction::Step => world.next(),
                }
            }
        }

        cam.reset();
        cam.draw(world);
        render(stdout, cam, world, rows)?;
    }

    Ok(())
}

fn render(
    stdout: &mut io::Stdout,
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
    let left = format!("x: {}  y: {}", x, y);
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
