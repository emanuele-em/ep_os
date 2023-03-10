use bootloader_api::info::{PixelFormat, FrameBuffer};
use core::{fmt, ptr};
use font_constants::BACKUP_CHAR;
use noto_sans_mono_bitmap::{
    get_raster, get_raster_width, FontWeight, RasterHeight, RasterizedChar,
};
use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    pub static ref FRAMEBUFFERWRITER: Mutex<FrameBufferWriter> = Mutex::new(FrameBufferWriter::new());
}


const LINE_SPACING: usize = 2;
const LETTER_SPACING: usize = 0;

const BORDER_PADDING: usize = 1;

mod font_constants {
    use super::*;

    pub const CHAR_RASTER_HEIGHT: RasterHeight = RasterHeight::Size16;

    pub const CHAR_RASTER_WIDTH: usize = get_raster_width(FontWeight::Regular, CHAR_RASTER_HEIGHT);

    pub const BACKUP_CHAR: char = '�';

    pub const FONT_WEIGHT: FontWeight = FontWeight::Regular;
}

fn get_char_raster(c: char) -> RasterizedChar {
    fn get(c: char) -> Option<RasterizedChar> {
        get_raster(
            c,
            font_constants::FONT_WEIGHT,
            font_constants::CHAR_RASTER_HEIGHT,
        )
    }
    get(c).unwrap_or_else(|| get(BACKUP_CHAR).expect("Should get raster of backup char."))
}

pub struct FrameBufferWriter {
    framebuffer: Option<&'static mut FrameBuffer>,
    x_pos: usize,
    y_pos: usize,
}

impl FrameBufferWriter {
    pub fn new() -> Self {
        Self {
            framebuffer: None,
            x_pos: 0,
            y_pos: 0,
        }
    }

    pub fn init(&mut self, framebuffer: &'static mut FrameBuffer){
        self.framebuffer = Some(framebuffer);
        self.clear();
    }

    fn newline(&mut self) {
        self.y_pos += font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        self.carriage_return()
    }

    fn carriage_return(&mut self) {
        self.x_pos = BORDER_PADDING;
    }

    pub fn clear(&mut self) {
        self.x_pos = BORDER_PADDING;
        self.y_pos = BORDER_PADDING;
        if self.framebuffer.is_some() {
            self.framebuffer.as_mut().unwrap().buffer_mut().fill(0)
        }
    }

    fn width(&self) -> usize {
        self.framebuffer.as_ref().unwrap().info().width
    }

    fn height(&self) -> usize {
        self.framebuffer.as_ref().unwrap().info().height
    }

    /// Writes a single char to the framebuffer. Takes care of special control characters, such as
    /// newlines and carriage returns.
    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                let new_xpos = self.x_pos + font_constants::CHAR_RASTER_WIDTH;
                if new_xpos >= self.width() {
                    self.newline();
                }
                let new_ypos =
                    self.y_pos + font_constants::CHAR_RASTER_HEIGHT.val() + BORDER_PADDING;
                if new_ypos >= self.height() {
                    self.clear();
                }
                self.write_rendered_char(get_char_raster(c));
            }
        }
    }

    fn write_rendered_char(&mut self, rendered_char: RasterizedChar) {
        for (y, row) in rendered_char.raster().iter().enumerate() {
            for (x, byte) in row.iter().enumerate() {
                self.write_pixel(self.x_pos + x, self.y_pos + y, *byte);
            }
        }
        self.x_pos += rendered_char.width() + LETTER_SPACING;
    }

    fn write_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let pixel_offset = y * self.framebuffer.as_ref().unwrap().info().stride + x;
        let color = match self.framebuffer.as_ref().unwrap().info().pixel_format {
            PixelFormat::Rgb => [intensity, intensity, intensity / 2, 0],
            PixelFormat::Bgr => [intensity / 2, intensity, intensity, 0],
            PixelFormat::U8 => [if intensity > 200 { 0xf } else { 0 }, 0, 0, 0],
            other => {
                // set a supported (but invalid) pixel format before panicking to avoid a double
                // panic; it might not be readable though
                self.framebuffer.as_ref().unwrap().info().pixel_format = PixelFormat::Rgb;
                panic!("pixel format {:?} not supported in logger", other)
            }
        };
        let bytes_per_pixel = self.framebuffer.as_ref().unwrap().info().bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;
        self.framebuffer.as_mut().unwrap().buffer_mut()[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);
        let _ = unsafe { ptr::read_volatile(&self.framebuffer.as_mut().unwrap().buffer_mut()[byte_offset]) };
    }
}

unsafe impl Send for FrameBufferWriter {}
unsafe impl Sync for FrameBufferWriter {}

impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    FRAMEBUFFERWRITER.lock().write_fmt(args).unwrap();
}