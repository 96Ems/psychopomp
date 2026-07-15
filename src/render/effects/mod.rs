mod focus_pulse;
mod prismatic_bloom;

use crate::dsl::{AnnotationEffect, AnnotationFrame};

#[derive(Clone, Copy)]
struct EffectTarget {
    center: [f32; 2],
}

pub(super) fn composite(
    pixels: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    code_origin: [f32; 2],
    line_height: f32,
    frame: AnnotationFrame,
) {
    let target = EffectTarget {
        center: [
            code_origin[0] + frame.target.center_x(),
            code_origin[1] + frame.target.line_y + line_height * 0.5,
        ],
    };
    let mut canvas = Canvas {
        pixels,
        width: canvas_width,
        height: canvas_height,
    };
    match frame.effect {
        AnnotationEffect::PrismaticBloom => {
            prismatic_bloom::composite(&mut canvas, target, frame.phase)
        }
        AnnotationEffect::FocusPulse => focus_pulse::composite(&mut canvas, target, frame.phase),
    }
}

struct Canvas<'a> {
    pixels: &'a mut [u8],
    width: u32,
    height: u32,
}

impl Canvas<'_> {
    fn blend(&mut self, x: i32, y: i32, color: [u8; 4], opacity: f32) {
        if !(0..self.width as i32).contains(&x) || !(0..self.height as i32).contains(&y) {
            return;
        }
        let index = (y as usize * self.width as usize + x as usize) * 4;
        super::blend_pixel(&mut self.pixels[index..index + 4], color, opacity);
    }

    fn soft_disc(&mut self, center: [f32; 2], radius: f32, color: [u8; 3], opacity: f32) {
        let extent = (radius + 1.0).ceil() as i32;
        for offset_y in -extent..=extent {
            for offset_x in -extent..=extent {
                let distance = ((offset_x * offset_x + offset_y * offset_y) as f32).sqrt();
                let alpha = (radius + 0.75 - distance).clamp(0.0, 1.0) * opacity;
                if alpha > 0.001 {
                    self.blend(
                        center[0].round() as i32 + offset_x,
                        center[1].round() as i32 + offset_y,
                        [color[0], color[1], color[2], 255],
                        alpha,
                    );
                }
            }
        }
    }
}
