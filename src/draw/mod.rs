use crate::quadtree::Node;
use crate::quadtree::QuadTree;

pub mod window;

pub use self::window::WindowContext;

impl Node {
    pub fn draw(&self, ctx: &mut WindowContext, x: i32, y: i32, depth: u32) {
        // too small to draw
        if ctx.zoom_level as i64 > depth as i64 {
            return;
        }

        // apparent side length of the node
        let sl = 1 << (depth as i64 - ctx.zoom_level as i64);

        // if the square extends too far to the right or to the left
        if x + ctx.width as i32 <= 0 || y + ctx.height as i32 <= 0 || x >= sl || y >= sl {
            return;
        }

        if self.is_empty() {
            return;
        }

        if depth > 0 && sl > 1 {
            let sw = sl >> 1;

            if let Some(child) = &self.sw {
                child.draw(ctx, x, y, depth - 1);
            }

            if let Some(child) = &self.se {
                child.draw(ctx, x - sw, y, depth - 1);
            }

            if let Some(child) = &self.nw {
                child.draw(ctx, x, y - sw, depth - 1);
            }

            if let Some(child) = &self.ne {
                child.draw(ctx, x - sw, y - sw, depth - 1);
            }

            return;
        }

        let at = |x: usize, y: usize| -> usize { y * ctx.width as usize + x };

        // draw a single pixel
        if sl == 1 && x <= 0 && y <= 0 {
            ctx.fb[at(-x as usize, -y as usize)] = true;
            return;
        }

        // draw a square
        if self.is_leaf() {
            // FIXME: can be out of range (see above)

            for dx in 0..sl {
                for dy in 0..sl {
                    let x = -(x + dx);
                    let y = -(y + dy);
                    ctx.fb[at(x as usize, y as usize)] = true;
                }
            }
        }
    }
}

impl QuadTree {
    pub fn draw(&self, ctx: &mut WindowContext) {
        let (x, y) = (ctx.x, ctx.y);
        self.root.draw(ctx, x, y, self.level);
    }
}
