//! Platform-specific tray presentation. macOS uses native text, not a bitmap font.

#[cfg(target_os = "macos")]
mod macos;

use tauri::AppHandle;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrafficLabels {
    pub download: String,
    pub upload: String,
    pub tooltip: String,
}

impl TrafficLabels {
    pub fn from_rates(up_bps: i64, down_bps: i64) -> Self {
        Self {
            download: format_speed(down_bps, true),
            upload: format_speed(up_bps, true),
            tooltip: format!(
                "↓ {} · ↑ {}",
                format_speed(down_bps, false),
                format_speed(up_bps, false)
            ),
        }
    }
}

fn format_speed(bytes_per_sec: i64, compact: bool) -> String {
    const UNITS: [&str; 7] = ["B", "K", "M", "G", "T", "P", "E"];
    if bytes_per_sec <= 0 {
        return if compact { "0K/s" } else { "0 KB/s" }.to_string();
    }
    let mut value = bytes_per_sec as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let round = |v: f64| {
        if v < 10.0 {
            (v * 10.0).round() / 10.0
        } else {
            v.round()
        }
    };
    // Promote before rounding produces a four-digit value like 1000K/s.
    if round(value) >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let rounded = round(value);
    let number = if rounded.fract() == 0.0 {
        format!("{}", rounded as u64)
    } else {
        format!("{rounded:.1}")
    };
    if compact {
        format!("{number}{}/s", UNITS[unit])
    } else if unit == 0 {
        format!("{number} B/s")
    } else {
        format!("{number} {}B/s", UNITS[unit])
    }
}

pub fn initial_icon(app: &AppHandle) -> tauri::Result<tauri::image::Image<'static>> {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        // Tauri sizes the native button from this image's aspect ratio (18pt tall).
        // Reserve 66pt for the logo and native text rows, with a single hit target.
        Ok(tauri::image::Image::new_owned(
            vec![0; 132 * 36 * 4],
            132,
            36,
        ))
    }
    #[cfg(not(target_os = "macos"))]
    app.default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))
}

pub fn initialize(app: &AppHandle) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    macos::install(app)?;
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    Ok(())
}

/// Called on the main thread; unchanged formatted values never reach AppKit.
pub fn update_traffic(labels: &TrafficLabels) {
    #[cfg(target_os = "macos")]
    macos::update(labels);
    #[cfg(not(target_os = "macos"))]
    let _ = labels;
}
