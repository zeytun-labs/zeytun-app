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
#[cfg(target_os = "macos")]
const ICON_PX: f64 = 32.0;

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

    if process_path.is_empty() {
        return Err(crate::error::CommandError::Internal(
            "Cache miss and no process path provided for extraction".to_string(),
        ));
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
    }

    let png = extract_icon_png(process_path)?;

    // Write to a temp file + rename so a concurrent reader never sees a
    // half-written PNG.
    let tmp = dest.with_extension("png.tmp");
    std::fs::write(&tmp, &png).map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
    std::fs::rename(&tmp, &dest)
        .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;

    Ok(dest)
}

/// Replace path separators / NUL so a process name can't escape the cache dir.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | '\0' | ':' => '_',
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
        let target_size = NSSize::new(ICON_PX, ICON_PX);
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

#[cfg(not(target_os = "macos"))]
fn extract_icon_png(process_path: &str) -> Result<Vec<u8>, crate::error::CommandError> {
    let _ = process_path;
    Err(crate::error::CommandError::Internal(
        "Icon extraction is only supported on macOS".to_string(),
    ))
}
