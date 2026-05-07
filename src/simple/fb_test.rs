use crate::color::{Color, Palette};
use crate::fb::{Framebuffer, Mask};
use crate::simple::Console;

use super::{console_write, fb::FbConsole};

fn harness(console: &mut impl Console) {
    let colors = [
        Palette::White,
        Palette::LightCyan,
        Palette::LightMagenta,
        Palette::LightBlue,
        Palette::Yellow,
        Palette::LightGreen,
        Palette::LightRed,
        Palette::DarkGray,
        Palette::LightGray,
        Palette::Cyan,
        Palette::Magenta,
        Palette::Blue,
        Palette::Brown,
        Palette::Green,
        Palette::Red,
    ];

    console.clear().unwrap();
    for color in colors {
        console.state().set_fg(Color::Palette(color)).unwrap();
        console_write(console, b"The quick brown fox jumps over the lazy dog.\n").unwrap();
    }
}

#[test]
fn rgbx_8888_pad_0() {
    let width: usize = 20;
    let height: usize = 20;
    let bpp: usize = 32;
    let mut data: Vec<u8> = vec![0u8; width * (bpp / 8) * height];

    let fb = Framebuffer {
        ptr: data.as_mut_ptr(),
        width: width,
        height: height,
        pitch: width * (bpp / 8),
        bpp: bpp,
        red_mask: Mask::new(8, 0),
        green_mask: Mask::new(8, 8),
        blue_mask: Mask::new(8, 16),
    };

    let mut fb_console = FbConsole::new(fb, None).unwrap();

    harness(&mut fb_console);
}

mod sdl3_tests {
    use sdl3::event::Event;
    use sdl3::pixels::PixelFormat;
    use sdl3::surface::Surface;

    use crate::simple::Console;

    #[test]
    fn sdl3_test() {
        let sdl_context = sdl3::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();

        let window = video_subsystem
            .window("framebuffer test", 800, 600)
            .position_centered()
            .opengl()
            .build()
            .unwrap();

        let size = window.size();
        let mut surface = Surface::new(size.0, size.1, PixelFormat::RGBX32).unwrap();
        let rect = sdl3::rect::Rect::new(0, 0, size.0, size.1);

        surface
            .fill_rect(rect, sdl3::pixels::Color::RGB(255, 255, 255))
            .unwrap();

        let fb = unsafe {
            let data = surface.without_lock_mut().unwrap();

            super::Framebuffer {
                ptr: data.as_mut_ptr(),
                width: surface.width() as usize,
                height: surface.height() as usize,
                pitch: surface.pitch() as usize,
                bpp: 32,
                red_mask: super::Mask::new(8, 0),
                green_mask: super::Mask::new(8, 8),
                blue_mask: super::Mask::new(8, 16),
            }
        };

        let mut fb_console = super::FbConsole::new(fb, None).unwrap();

        super::harness(&mut fb_console);

        let mut canvas = window.into_canvas();
        let texture_creator = canvas.texture_creator();

        canvas.clear();
        canvas.present();

        let mut event_pump = sdl_context.event_pump().unwrap();
        let mut ticks = sdl3::timer::ticks();

        'event_loop: loop {
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => break 'event_loop,
                    _ => {}
                }
            }
            let now = sdl3::timer::ticks();
            if now - ticks >= 350 {
                ticks = now;
                fb_console.blink_cursor(None).unwrap();
            }
            let texture = surface.as_texture(&texture_creator).unwrap();
            canvas.copy(&texture, None, None).unwrap();
            canvas.present();
        }
    }
}
