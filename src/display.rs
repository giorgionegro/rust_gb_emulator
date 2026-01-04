use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;
use sdl2::EventPump;

pub struct Display {
    canvas: Canvas<Window>,
    event_pump: EventPump,
    scale: u32,
}

// Classic Game Boy DMG color palette
const GB_COLORS: [Color; 4] = [
    Color::RGB(155, 188, 15),   // Lightest
    Color::RGB(139, 172, 15),   // Light
    Color::RGB(48, 98, 48),     // Dark
    Color::RGB(15, 56, 15),     // Darkest
];

impl Display {
    pub fn new(title: &str, scale: u32) -> Result<Self, String> {
        let sdl_context = sdl2::init()?;
        let video_subsystem = sdl_context.video()?;

        let window = video_subsystem
            .window(title, 160 * scale, 144 * scale)
            .position_centered()
            .build()
            .map_err(|e| e.to_string())?;

        let mut canvas = window
            .into_canvas()
            .accelerated()
            .present_vsync()
            .build()
            .map_err(|e| e.to_string())?;

        canvas.set_scale(scale as f32, scale as f32)
            .map_err(|e| e.to_string())?;

        let event_pump = sdl_context.event_pump()?;

        Ok(Display {
            canvas,
            event_pump,
            scale,
        })
    }

    pub fn render(&mut self, framebuffer: &[u8; 160 * 144]) {
        self.canvas.set_draw_color(GB_COLORS[0]);
        self.canvas.clear();

        for y in 0..144 {
            for x in 0..160 {
                let pixel_index = y * 160 + x;
                let color_id = framebuffer[pixel_index] as usize;
                
                if color_id < 4 {
                    self.canvas.set_draw_color(GB_COLORS[color_id]);
                    self.canvas.draw_point((x as i32, y as i32)).ok();
                }
            }
        }

        self.canvas.present();
    }

    pub fn render_fast(&mut self, framebuffer: &[u8; 160 * 144]) {
        for y in 0..144 {
            for x in 0..160 {
                let pixel_index = y * 160 + x;
                let color_id = framebuffer[pixel_index] as usize;
                
                if color_id < 4 {
                    self.canvas.set_draw_color(GB_COLORS[color_id]);
                    let rect = Rect::new(x as i32, y as i32, 1, 1);
                    self.canvas.fill_rect(rect).ok();
                }
            }
        }

        self.canvas.present();
    }

    pub fn handle_events(&mut self) -> bool {
        use sdl2::event::Event;
        use sdl2::keyboard::Keycode;

        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => return false,
                _ => {}
            }
        }
        true
    }

    pub fn set_title(&mut self, title: &str) {
        self.canvas.window_mut().set_title(title).ok();
    }
}

