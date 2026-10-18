// src-tauri/src/adapters/process_icon.rs
//
// Persistent, disk-based process-icon cache.
//
// The macOS system icon for a process is extracted once, resized to a small
// raster (to avoid the multi-hundred-KB full-resolution icon), and written to
//
//     <app_cache_dir>/icons/<process_name>.png
//
// On subsequent requests we short-circuit: if the PNG already exists we skip
// the (expensive) AppKit extraction entirely and just hand back the path. The
// frontend loads the file via Tauri's asset protocol (`convertFileSrc`), so no
// base64 ever crosses the IPC boundary or sits in the WebView heap.

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
pub fn set_dock_icon(app_handle: &tauri::AppHandle, style: &str) -> Result<(), String> {
    use objc2::msg_send;
    use objc2_app_kit::NSApplication;

    use std::path::PathBuf;
    use tauri::Manager;

    // style can be "default", "clear-dark", "clear-light", "dark", "tinted-dark"
    let filename = format!("{}.png", style);

    // In dev mode, tauri often resolves `resource_dir()` differently.
    // Try to resolve it via `app_handle.path().resolve(...)` directly to the `static` or `icons` directory.
    let path = app_handle
        .path()
        .resolve(
            format!("icons/dock/{}", filename),
            tauri::path::BaseDirectory::Resource,
        )
        .unwrap_or_else(|_| PathBuf::from(""));

    if !path.exists() {
        return Err(format!("Icon file not found: {:?}", path));
    }

    let _ = app_handle.run_on_main_thread(move || {
        unsafe {
            let ns_string_class = objc2::runtime::AnyClass::get(c"NSString").unwrap();

            let path_str = path.to_string_lossy().to_string();
            let ns_path: *mut objc2::runtime::AnyObject = msg_send![ns_string_class, alloc];
            let ns_path: *mut objc2::runtime::AnyObject = msg_send![
                ns_path,
                initWithBytes: path_str.as_ptr(),
                length: path_str.len(),
                encoding: 4 // NSUTF8StringEncoding
            ];
            let ns_path_id = objc2::rc::Retained::from_raw(ns_path).unwrap();

            let ns_image_class = objc2::runtime::AnyClass::get(c"NSImage").unwrap();

            let image: *mut objc2::runtime::AnyObject = msg_send![ns_image_class, alloc];
            let image: *mut objc2::runtime::AnyObject =
                msg_send![image, initWithContentsOfFile: &*ns_path_id];

            if !image.is_null() {
                let mtm = objc2::MainThreadMarker::new().unwrap();
                let app = NSApplication::sharedApplication(mtm);
                let _: () = msg_send![&app, setApplicationIconImage: image];
            } else {
                println!("Failed to create NSImage from path: {:?}", path_str);
            }
        }
    });

    Ok(())
}
#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon(_app_handle: &tauri::AppHandle, _style: &str) -> Result<(), String> {
    Ok(())
}

/// Subdirectory (under the OS app-cache dir) that holds the cached icon PNGs.
pub const ICON_CACHE_SUBDIR: &str = "icons";

/// Inspector renders icons at 24px; cap the raster at 32x32 so each PNG stays
/// ~2KB instead of the full 512/1024px system icon.
const ICON_PX: u32 = 32;

/// Absolute path of the cache file for a given process name (no extraction).
pub fn icon_cache_path(cache_dir: &Path, process_name: &str) -> PathBuf {
    cache_dir
        .join(ICON_CACHE_SUBDIR)
        .join(format!("{}.png", sanitize(process_name)))
}

/// Ensure the icon for `process_name` (extracted from `process_path`) exists in
/// the on-disk cache, and return its absolute path.
///
/// Work performed:
///   1. If `<cache>/icons/<name>.png` already exists -> return it, no extraction.
///   2. Otherwise extract the macOS icon, resize, and persist it, then return.
pub fn ensure_process_icon(
    cache_dir: &Path,
    process_name: &str,
    process_path: &str,
) -> Result<PathBuf, crate::error::CommandError> {
    let dest = icon_cache_path(cache_dir, process_name);

    // Step 2 (Performance): check-before-extract. A hit skips all AppKit work.
    if dest.exists() {
        return Ok(dest);
    }

    let mut effective_path = process_path.trim_matches('"').to_string();
    if effective_path.is_empty() || !Path::new(&effective_path).exists() {
        let (_, candidate) =
            crate::adapters::process_resolve::resolve_process("PROCESS-NAME", process_name);
        if !candidate.is_empty() && Path::new(&candidate).exists() {
            effective_path = candidate;
        }
    }

    if effective_path.is_empty() {
        return Err(crate::error::CommandError::Internal(
            "Cache miss and no process path provided for extraction".to_string(),
        ));
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
    }

    let png = extract_icon_png(&effective_path)?;

    // Write to a temp file + rename so a concurrent reader never sees a
    // half-written PNG.
    let tmp = dest.with_extension("png.tmp");
    std::fs::write(&tmp, &png).map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
    std::fs::rename(&tmp, &dest)
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;

    Ok(dest)
}

/// Replace path separators / NUL / Windows-invalid characters so a process
/// name can't escape the cache dir or create an illegal filename.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Extract the macOS system icon for `process_path`, resized to `ICON_PX`, as
/// raw PNG bytes. macOS-only; other platforms return an error.
#[cfg(target_os = "macos")]
fn extract_icon_png(process_path: &str) -> Result<Vec<u8>, crate::error::CommandError> {
    use objc2::msg_send;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    unsafe {
        let mut path_to_use = process_path.to_string();
        if let Some(app_idx) = process_path.find(".app") {
            path_to_use = process_path[..app_idx + 4].to_string();
        }

        let workspace = NSWorkspace::sharedWorkspace();

        let ns_string_class = objc2::runtime::AnyClass::get(c"NSString")
            .ok_or_else(|| "NSString class not found".to_string())?;
        let ns_path: *mut objc2::runtime::AnyObject = msg_send![ns_string_class, alloc];
        let ns_path: *mut objc2::runtime::AnyObject = msg_send![
            ns_path,
            initWithBytes: path_to_use.as_ptr(),
            length: path_to_use.len(),
            encoding: 4 // NSUTF8StringEncoding
        ];
        let ns_path_id = objc2::rc::Retained::from_raw(ns_path)
            .ok_or_else(|| "Failed to wrap NSString".to_string())?;

        let image: *mut objc2::runtime::AnyObject =
            msg_send![&workspace, iconForFile: &*ns_path_id];
        if image.is_null() {
            return Err(crate::error::CommandError::Internal(
                "Failed to get icon image".to_string(),
            ));
        }

        // Redraw the (possibly 1024px) system icon into a small 32x32 bitmap so
        // the PNG stays ~2KB. Uses an explicit CoreGraphics bitmap context
        // (NSBitmapImageRep + NSGraphicsContext) so it is safe on the tokio
        // worker thread this command runs on -- NSImage lockFocus is
        // main-thread-only and must not be used here.
        let target_size = NSSize::new(ICON_PX as f64, ICON_PX as f64);
        let px = ICON_PX as isize;

        let bitmap_class = objc2::runtime::AnyClass::get(c"NSBitmapImageRep")
            .ok_or_else(|| "NSBitmapImageRep class not found".to_string())?;

        // colorSpaceName: NSDeviceRGBColorSpace
        let cs_name = "NSDeviceRGBColorSpace";
        let ns_cs: *mut objc2::runtime::AnyObject = msg_send![ns_string_class, alloc];
        let ns_cs: *mut objc2::runtime::AnyObject = msg_send![
            ns_cs,
            initWithBytes: cs_name.as_ptr(),
            length: cs_name.len(),
            encoding: 4usize // NSUTF8StringEncoding
        ];
        let ns_cs = objc2::rc::Retained::from_raw(ns_cs)
            .ok_or_else(|| "Failed to wrap colorSpace NSString".to_string())?;

        let rep: *mut objc2::runtime::AnyObject = msg_send![bitmap_class, alloc];
        let rep: *mut objc2::runtime::AnyObject = msg_send![
            rep,
            initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>(),
            pixelsWide: px,
            pixelsHigh: px,
            bitsPerSample: 8isize,
            samplesPerPixel: 4isize,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: &*ns_cs,
            bytesPerRow: 0isize,
            bitsPerPixel: 0isize
        ];
        let rep = objc2::rc::Retained::from_raw(rep)
            .ok_or_else(|| "Failed to alloc NSBitmapImageRep".to_string())?;
        let _: () = msg_send![&*rep, setSize: target_size];

        let gc_class = objc2::runtime::AnyClass::get(c"NSGraphicsContext")
            .ok_or_else(|| "NSGraphicsContext class not found".to_string())?;
        let ctx: *mut objc2::runtime::AnyObject =
            msg_send![gc_class, graphicsContextWithBitmapImageRep: &*rep];
        if ctx.is_null() {
            return Err(crate::error::CommandError::Internal(
                "Failed to create graphics context".to_string(),
            ));
        }

        let _: () = msg_send![gc_class, saveGraphicsState];
        let _: () = msg_send![gc_class, setCurrentContext: ctx];
        let dst = NSRect::new(NSPoint::new(0.0, 0.0), target_size);
        let zero = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
        // drawInRect:fromRect:operation:fraction: -- op 1 = NSCompositingOperationCopy.
        let _: () = msg_send![
            image,
            drawInRect: dst,
            fromRect: zero,
            operation: 1usize,
            fraction: 1.0f64
        ];
        let _: () = msg_send![gc_class, restoreGraphicsState];

        let png_data: *mut objc2::runtime::AnyObject = msg_send![
            &*rep,
            representationUsingType: 4usize, // NSBitmapImageFileTypePNG
            properties: std::ptr::null_mut::<objc2::runtime::AnyObject>()
        ];
        if png_data.is_null() {
            return Err(crate::error::CommandError::Internal(
                "Failed to get PNG representation".to_string(),
            ));
        }

        let bytes: *const std::ffi::c_void = msg_send![png_data, bytes];
        let length: usize = msg_send![png_data, length];
        let slice = std::slice::from_raw_parts(bytes as *const u8, length);
        Ok(slice.to_vec())
    }
}

#[cfg(target_os = "windows")]
fn extract_icon_png(process_path: &str) -> Result<Vec<u8>, crate::error::CommandError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::UI::Shell::{
        ExtractIconExW, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON,
    };
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON};

    let clean_path = process_path.trim_matches('"');
    let path_wide: Vec<u16> = std::ffi::OsStr::new(clean_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut hicon = HICON::default();
    let mut hicon_large = HICON::default();
    let mut hicon_small = HICON::default();

    unsafe {
        let count = ExtractIconExW(
            PCWSTR(path_wide.as_ptr()),
            0,
            Some(&mut hicon_large),
            Some(&mut hicon_small),
            1,
        );
        if count == 1 && !hicon_large.is_invalid() {
            hicon = hicon_large;
            if !hicon_small.is_invalid() {
                let _ = DestroyIcon(hicon_small);
            }
        } else if count == 1 && !hicon_small.is_invalid() {
            hicon = hicon_small;
            if !hicon_large.is_invalid() {
                let _ = DestroyIcon(hicon_large);
            }
        } else {
            if !hicon_large.is_invalid() {
                let _ = DestroyIcon(hicon_large);
            }
            if !hicon_small.is_invalid() {
                let _ = DestroyIcon(hicon_small);
            }
            let mut shfi = SHFILEINFOW::default();
            let res = SHGetFileInfoW(
                PCWSTR(path_wide.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut shfi),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );
            if res != 0 && !shfi.hIcon.is_invalid() {
                hicon = shfi.hIcon;
            }
        }
    }

    if hicon.is_invalid() {
        return Err(crate::error::CommandError::Internal(format!(
            "Failed to extract icon for `{clean_path}`"
        )));
    }

    struct IconGuard(HICON);
    impl Drop for IconGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyIcon(self.0);
            }
        }
    }
    let _icon_guard = IconGuard(hicon);

    unsafe { hicon_to_png(hicon) }
}

#[cfg(target_os = "windows")]
unsafe fn hicon_to_png(
    hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
) -> Result<Vec<u8>, crate::error::CommandError> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW,
        BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC, HBITMAP, HGDIOBJ,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    let mut icon_info = ICONINFO::default();
    GetIconInfo(hicon, &mut icon_info)
        .map_err(|e| crate::error::CommandError::Internal(format!("GetIconInfo failed: {e}")))?;

    struct GdiGuard {
        color: HBITMAP,
        mask: HBITMAP,
    }
    impl Drop for GdiGuard {
        fn drop(&mut self) {
            unsafe {
                if !self.color.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(self.color.0));
                }
                if !self.mask.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(self.mask.0));
                }
            }
        }
    }
    let _guard = GdiGuard {
        color: icon_info.hbmColor,
        mask: icon_info.hbmMask,
    };

    let hdc = CreateCompatibleDC(None);
    if hdc.is_invalid() {
        return Err(crate::error::CommandError::Internal("CreateCompatibleDC failed".into()));
    }
    struct DcGuard(HDC);
    impl Drop for DcGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteDC(self.0);
            }
        }
    }
    let _dc_guard = DcGuard(hdc);

    let has_color = !icon_info.hbmColor.is_invalid();
    let target_bmp = if has_color {
        icon_info.hbmColor
    } else {
        icon_info.hbmMask
    };

    let mut bm = BITMAP::default();
    if GetObjectW(
        HGDIOBJ(target_bmp.0),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut _ as *mut _),
    ) == 0
    {
        return Err(crate::error::CommandError::Internal("GetObjectW failed".into()));
    }

    let width = bm.bmWidth;
    let height = if has_color {
        bm.bmHeight
    } else {
        bm.bmHeight / 2
    };
    if width <= 0 || height <= 0 {
        return Err(crate::error::CommandError::Internal("Invalid bitmap dimensions".into()));
    }

    let mut bi = BITMAPINFO::default();
    bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = width;
    bi.bmiHeader.biHeight = -height;
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    bi.bmiHeader.biCompression = BI_RGB.0;

    let mut bgra_buf = vec![0u8; (width * height * 4) as usize];
    let scanlines = GetDIBits(
        hdc,
        target_bmp,
        0,
        height as u32,
        Some(bgra_buf.as_mut_ptr() as *mut _),
        &mut bi,
        DIB_RGB_COLORS,
    );
    if scanlines == 0 {
        return Err(crate::error::CommandError::Internal("GetDIBits failed".into()));
    }

    let mut rgba_buf = bgra_buf;
    let mut has_alpha = false;
    for chunk in rgba_buf.chunks_exact_mut(4) {
        let b = chunk[0];
        let r = chunk[2];
        let a = chunk[3];
        chunk[0] = r;
        chunk[2] = b;
        if a > 0 {
            has_alpha = true;
        }
    }

    if !has_alpha && !icon_info.hbmMask.is_invalid() {
        let mut mask_bi = BITMAPINFO::default();
        mask_bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        mask_bi.bmiHeader.biWidth = width;
        mask_bi.bmiHeader.biHeight = -height;
        mask_bi.bmiHeader.biPlanes = 1;
        mask_bi.bmiHeader.biBitCount = 1;
        mask_bi.bmiHeader.biCompression = BI_RGB.0;

        let mask_row_bytes = ((width + 31) / 32 * 4) as usize;
        let mut mask_buf = vec![0u8; mask_row_bytes * height as usize];
        if GetDIBits(
            hdc,
            icon_info.hbmMask,
            0,
            height as u32,
            Some(mask_buf.as_mut_ptr() as *mut _),
            &mut mask_bi,
            DIB_RGB_COLORS,
        ) != 0
        {
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let byte_idx = y * mask_row_bytes + (x / 8);
                    let bit_idx = 7 - (x % 8);
                    let mask_bit = (mask_buf[byte_idx] >> bit_idx) & 1;
                    let pixel_idx = (y * width as usize + x) * 4;
                    rgba_buf[pixel_idx + 3] = if mask_bit == 0 { 255 } else { 0 };
                }
            }
        } else {
            for chunk in rgba_buf.chunks_exact_mut(4) {
                chunk[3] = 255;
            }
        }
    }

    let img = image::RgbaImage::from_raw(width as u32, height as u32, rgba_buf)
        .ok_or_else(|| crate::error::CommandError::Internal("Failed to create RgbaImage".into()))?;

    let resized = if width != ICON_PX as i32 || height != ICON_PX as i32 {
        image::imageops::resize(&img, ICON_PX, ICON_PX, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    resized
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| crate::error::CommandError::Internal(format!("PNG encode failed: {e}")))?;

    Ok(png_bytes)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn extract_icon_png(process_path: &str) -> Result<Vec<u8>, crate::error::CommandError> {
    let _ = process_path;
    Err(crate::error::CommandError::Internal(
        "Icon extraction is only supported on macOS and Windows".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_extract_windows_icon() {
        let sysroot = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        let cmd_path = format!(r"{sysroot}\System32\cmd.exe");
        if Path::new(&cmd_path).exists() {
            let png_bytes = extract_icon_png(&cmd_path).expect("failed to extract cmd.exe icon");
            assert!(!png_bytes.is_empty());
            assert_eq!(&png_bytes[0..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        }
    }
}
