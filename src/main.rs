use std::time::Duration;

use quadtree::QuadTree;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::pixels::Color;
use sdl2::rect::Point;

use crate::draw::WindowContext;

mod ext;
mod draw;
mod quadtree;
mod hashlife;

fn main() -> Result<(), String> {
    let width = 800;
    let height = 600;
    let mut ctx = WindowContext::new(width, height);
    let qt = QuadTree::new(5);

    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window("Hashlife demo", width, height)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    let mut event_pump = sdl_context.event_pump()?;

    'main: loop {
        // reset the frame buffer
        ctx.fb.fill(false);

        // draw tree
        qt.draw(&mut ctx);

        for x in 0..width {
            for y in 0..height {
                if ctx.fb[(y as usize) * (width as usize) + (x as usize)] {
                    canvas.set_draw_color(Color::BLACK);
                } else {
                    canvas.set_draw_color(Color::WHITE);
                }

                canvas.draw_point(Point::new(x as i32, y as i32))?;
            }
        }

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,

                // keybinds
                Event::KeyDown {
                    keycode: Some(keycode),
                    keymod,
                    ..
                } => {
                    #[allow(clippy::single_match)]
                    match (keycode, keymod) {
                        (Keycode::Q, _) => break 'main,

                        // zoom
                        (Keycode::J, Mod::LSHIFTMOD | Mod::RSHIFTMOD) => {
                            ctx.zoom_level += 1;
                        }
                        (Keycode::K, Mod::LSHIFTMOD | Mod::RSHIFTMOD) => {
                            ctx.zoom_level -= 1;
                        }

                        // movement
                        (Keycode::H, _) => {
                            ctx.x += 10
                        }
                        (Keycode::J, _) => {
                            ctx.y -= 10
                        }
                        (Keycode::K, _) => {
                            ctx.y += 10
                        }
                        (Keycode::L, _) => {
                            ctx.x -= 10
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
