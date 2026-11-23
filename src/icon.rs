use std::sync::OnceLock;

use image::{ImageFormat, RgbaImage};

const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");

fn source() -> &'static RgbaImage {
    static SOURCE: OnceLock<RgbaImage> = OnceLock::new();
    SOURCE.get_or_init(|| {
        image::load_from_memory_with_format(ICON_BYTES, ImageFormat::Png)
            .expect("bundled icon.png is valid")
            .into_rgba8()
    })
}

fn scaled(size: u32) -> RgbaImage {
    let source = source();
    if source.width() == size && source.height() == size {
        source.clone()
    } else {
        image::imageops::resize(source, size, size, image::imageops::FilterType::Lanczos3)
    }
}

/// Icon data for the native window (and eframe's window manager hints).
pub fn egui_icon() -> egui::IconData {
    let image = scaled(256);
    egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}

/// Icon pixmaps for the StatusNotifierItem tray icon.
pub fn tray_icons() -> Vec<ksni::Icon> {
    static ICONS: OnceLock<Vec<ksni::Icon>> = OnceLock::new();
    ICONS
        .get_or_init(|| {
            [16, 22, 32, 48]
                .into_iter()
                .map(|size| {
                    let image = scaled(size);
                    let mut data = image.into_raw();
                    // StatusNotifierItem wants ARGB32 in network byte order.
                    for pixel in data.as_chunks_mut::<4>().0 {
                        pixel.rotate_right(1);
                    }
                    ksni::Icon {
                        width: size as i32,
                        height: size as i32,
                        data,
                    }
                })
                .collect()
        })
        .clone()
}
