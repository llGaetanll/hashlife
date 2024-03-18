use std::time::Duration;

use sdl2::event::Event;
use sdl2::event::WindowEvent;
use sdl2::keyboard::Keycode;
use sdl2::keyboard::Mod;
use sdl2::pixels::Color;

use draw::grid::GridData;
use draw::window::WindowData;

mod draw;
mod ext;
mod qt;
mod quadtree;

fn main() -> Result<(), String> {
    let mut window_state = WindowData::new(800, 600);
    let mut grid = GridData::new();

    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window("Hashlife demo", window_state.width, window_state.height)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;

    canvas.set_draw_color(Color::RGB(0, 255, 255));
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl_context.event_pump()?;

    'main: loop {

        grid.draw(&mut canvas, &window_state)?;

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,

                // window resize
                Event::Window {
                    win_event: WindowEvent::Resized(new_width, new_height),
                    ..
                } => {
                    window_state.width = new_width as u32;
                    window_state.height = new_height as u32;
                }

                // keybinds
                Event::KeyDown {
                    keycode: Some(keycode),
                    keymod,
                    ..
                } => {
                    match (keycode, keymod) {
                        (Keycode::Q, _) => break 'main,

                        (Keycode::G, _) => {
                            grid.toggle_gridlines();
                        }

                        // movements
                        (Keycode::H, Mod::NOMOD) => {
                            grid.shift_x(10);
                        }
                        (Keycode::J, Mod::NOMOD) => {
                            grid.shift_y(-10);
                        }
                        (Keycode::K, Mod::NOMOD) => {
                            grid.shift_y(10);
                        }
                        (Keycode::L, Mod::NOMOD) => {
                            grid.shift_x(-10);
                        }

                        // zoom
                        (Keycode::J, Mod::LSHIFTMOD | Mod::RSHIFTMOD) => {
                            grid.zoom_in();
                        }
                        (Keycode::K, Mod::LSHIFTMOD | Mod::RSHIFTMOD) => {
                            grid.zoom_out();
                        }

                        _ => {}
                    }
                }
                _ => {}
            }
        }

        canvas.present();

        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
