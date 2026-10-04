use ab_glyph::{FontRef, PxScale};
use image::{imageops, Rgba, RgbaImage};
use imageproc::drawing::{draw_text_mut, text_size};
use std::sync::OnceLock;
use tauri::AppHandle;

static FONT_DATA: &[u8] = include_bytes!("../../resources/fonts/Geist.ttf");
static FONT: OnceLock<FontRef<'static>> = OnceLock::new();
/// Pre-resized icon cached on first call.
static RESIZED_ICON: OnceLock<RgbaImage> = OnceLock::new();

/// Render at 2x (Retina) resolution. macOS menu bar is ~22pt.
/// So we render at 44px height.
const TRAY_H: u32 = 44;
/// Scaled font size.
const FONT_PX: f32 = 30.0;
/// Width reserved for the text block.
const TEXT_W: u32 = 98;

fn get_font() -> &'static FontRef<'static> {
    FONT.get_or_init(|| FontRef::try_from_slice(FONT_DATA).expect("Error loading font"))
}

fn get_resized_icon() -> &'static RgbaImage {
    RESIZED_ICON.get_or_init(|| {
        let raw = include_bytes!("../../icons/tray.png");
        let full = image::load_from_memory(raw)
            .expect("Failed to load base icon")
            .into_rgba8();
        // Resize to match our Retina canvas height
        imageops::resize(&full, TRAY_H, TRAY_H, imageops::FilterType::Lanczos3)
    })
}

fn format_speed(bytes_per_sec: i64) -> String {
    const UNITS: [&str; 4] = ["B/s", "KB/s", "MB/s", "GB/s"];
    if bytes_per_sec <= 0 {
        return "0 KB/s".to_string();
    }
    let mut value = bytes_per_sec as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 || rounded >= 100.0 {
        format!("{} {}", rounded as i64, UNITS[unit])
    } else {
        format!("{:.1} {}", rounded, UNITS[unit])
    }
}

/// Helper to draw right-aligned text on the canvas.
fn draw_text_right_aligned(
    img: &mut RgbaImage,
    color: Rgba<u8>,
    right_edge_x: u32,
    y: i32,
    scale: PxScale,
    font: &FontRef<'_>,
    text: &str,
) {
    let (w, _h) = text_size(scale, font, text);

    let x = (right_edge_x as i32) - w as i32;
    draw_text_mut(img, color, x, y, scale, font, text);
}

pub fn update_tray_icon(app: &AppHandle, up_bps: i64, down_bps: i64) {
    let icon_img = get_resized_icon();
    let canvas_w = icon_img.width() + TEXT_W;
    let mut img = RgbaImage::new(canvas_w, TRAY_H);

    imageops::overlay(&mut img, icon_img, 0, 0);

    let up_str = format_speed(up_bps).to_string();
    let down_str = format_speed(down_bps).to_string();

    let font = get_font();
    let scale = PxScale::from(FONT_PX);
    let color = Rgba([255, 255, 255, 255]);

    // Padding from the right edge
    let right_edge = canvas_w;

    // Y coordinates adjusted for the 44px retina canvas.
    // Download on top, upload on the bottom line.
    draw_text_right_aligned(&mut img, color, right_edge, -5, scale, font, &down_str);
    draw_text_right_aligned(&mut img, color, right_edge, 18, scale, font, &up_str);

    let width = img.width();
    let height = img.height();
    let raw_pixels = img.into_raw();
    let icon = tauri::image::Image::new_owned(raw_pixels, width, height);

    if let Some(tray) = app.tray_by_id(crate::tray::TRAY_ID) {
        let tray_clone = tray.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = tray_clone.set_icon(Some(icon));
        });
    }
}
