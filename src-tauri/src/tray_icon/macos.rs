//! AppKit controls retain system font rendering, Retina scaling and appearance.
//! All native controls in this module stay on the main thread.

use super::TrafficLabels;
use objc2::{
    define_class, msg_send, rc::Retained, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSAccessibility, NSColor, NSFont, NSFontWeightRegular, NSImage, NSImageScaling, NSImageView,
    NSTextAlignment, NSTextField, NSView,
};
use objc2_foundation::{NSData, NSPoint, NSRect, NSSize, NSString};
use std::cell::RefCell;
use tauri::AppHandle;

const WIDTH: f64 = 66.0;
const HEIGHT: f64 = 22.0;

struct TrafficViewIvars {
    download: Retained<NSTextField>,
    upload: Retained<NSTextField>,
}

define_class!(
    // SAFETY: NSView is the immediate superclass, every retained control is
    // main-thread-only, and the override matches NSView's hitTest: signature.
    #[unsafe(super(NSView))]
    #[name = "ZeytunTrayTrafficView"]
    #[thread_kind = MainThreadOnly]
    #[ivars = TrafficViewIvars]
    struct TrafficView;

    impl TrafficView {
        /// Decorative content must not intercept Tauri's native menu target.
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }
    }
);

impl TrafficView {
    fn new(mtm: MainThreadMarker) -> tauri::Result<Retained<Self>> {
        let download = label(mtm, "0K/s", 30.0, 11.0, 36.0, true);
        let upload = label(mtm, "0K/s", 30.0, 0.0, 36.0, true);
        let this = Self::alloc(mtm).set_ivars(TrafficViewIvars { download, upload });
        // SAFETY: freshly allocated NSView subclass with initialized Rust ivars;
        // invoke the designated superclass initializer on the main thread.
        let this: Retained<Self> = unsafe {
            msg_send![super(this), initWithFrame: NSRect::new(NSPoint::ZERO, NSSize::new(WIDTH, HEIGHT))]
        };
        this.addSubview(&this.ivars().download);
        this.addSubview(&this.ivars().upload);
        this.addSubview(&label(mtm, "↓", 22.0, 11.0, 12.0, false));
        this.addSubview(&label(mtm, "↑", 22.0, 0.0, 12.0, false));
        let data = NSData::with_bytes(include_bytes!("../../icons/tray.png"));
        let image = NSImage::initWithData(NSImage::alloc(), &data)
            .ok_or_else(|| tauri::Error::AssetNotFound("tray.png".into()))?;
        image.setTemplate(true);
        let logo = NSImageView::initWithFrame(
            NSImageView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 2.0), NSSize::new(18.0, 18.0)),
        );
        logo.setImage(Some(&image));
        logo.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
        logo.setContentTintColor(Some(&NSColor::labelColor()));
        this.addSubview(&logo);
        Ok(this)
    }

    fn update(&self, labels: &TrafficLabels) {
        if self.ivars().download.stringValue().to_string() != labels.download {
            self.ivars()
                .download
                .setStringValue(&NSString::from_str(&labels.download));
        }
        if self.ivars().upload.stringValue().to_string() != labels.upload {
            self.ivars()
                .upload
                .setStringValue(&NSString::from_str(&labels.upload));
        }
    }
}

fn label(
    mtm: MainThreadMarker,
    text: &str,
    x: f64,
    y: f64,
    width: f64,
    numeric: bool,
) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(width, 11.0)));
    // SAFETY: immutable AppKit font-weight constant, provided by the linked framework.
    let regular = unsafe { NSFontWeightRegular };
    let font = if numeric {
        NSFont::monospacedDigitSystemFontOfSize_weight(8.5, regular)
    } else {
        NSFont::systemFontOfSize_weight(9.0, regular)
    };
    label.setFont(Some(&font));
    label.setTextColor(Some(&NSColor::labelColor()));
    label.setAlignment(if numeric {
        NSTextAlignment::Right
    } else {
        NSTextAlignment::Left
    });
    label.setSelectable(false);
    label
}

thread_local! {
    static TRAFFIC_VIEW: RefCell<Option<Retained<TrafficView>>> = const { RefCell::new(None) };
}

pub(super) fn install(app: &AppHandle) -> tauri::Result<()> {
    let tray = app
        .tray_by_id(crate::tray::TRAY_ID)
        .ok_or_else(|| tauri::Error::AssetNotFound("tray icon".into()))?;
    tray.with_inner_tray_icon(|inner| -> tauri::Result<()> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| tauri::Error::AssetNotFound("AppKit main thread".into()))?;
        let item = inner
            .ns_status_item()
            .ok_or_else(|| tauri::Error::AssetNotFound("native status item".into()))?;
        let button = item
            .button(mtm)
            .ok_or_else(|| tauri::Error::AssetNotFound("native status button".into()))?;
        button.setAccessibilityLabel(Some(&NSString::from_str("Zeytun")));
        let view = TrafficView::new(mtm)?;
        let bounds = button.bounds();
        view.setFrameOrigin(NSPoint::new(
            (bounds.size.width - WIDTH) / 2.0,
            (bounds.size.height - HEIGHT) / 2.0,
        ));
        button.addSubview(&view);
        TRAFFIC_VIEW.with(|slot| *slot.borrow_mut() = Some(view));
        Ok(())
    })?
}

pub(super) fn update(labels: &TrafficLabels) {
    debug_assert!(MainThreadMarker::new().is_some());
    TRAFFIC_VIEW.with(|slot| {
        if let Some(view) = slot.borrow().as_ref() {
            view.update(labels);
        }
    });
}
