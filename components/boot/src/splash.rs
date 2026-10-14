//! Статический знак ранней загрузки UEFI. Анимацию после запуска ядра
//! рисует графическая граница luna-user-session; firmware не изображает прогресс.

use alloc::vec::Vec;
use uefi::boot::{self, open_protocol_exclusive};
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};

/// Точный статический кадр трёх каменных колец с пустым центром.
/// Файл воспроизводится примером render-alpha из нативной геометрии и исходной
/// каменной фактуры. Один BLT ограничивает работу прошивки независимо от режима.
pub fn show() {
    let Ok(handle) = boot::get_handle_for_protocol::<GraphicsOutput>() else {
        return;
    };
    let Ok(mut gop) = open_protocol_exclusive::<GraphicsOutput>(handle) else {
        return;
    };
    let (width, height) = gop.current_mode_info().resolution();
    let _ = gop.blt(BltOp::VideoFill {
        color: BltPixel::new(8, 15, 32),
        dest: (0, 0),
        dims: (width, height),
    });
    if width < 256 || height < 256 {
        return;
    }
    let pixels: Vec<_> = include_bytes!("../assets/rings-uefi.xrgb")
        .chunks_exact(4)
        .map(|p| BltPixel::new(p[2], p[1], p[0]))
        .collect();
    let _ = gop.blt(BltOp::BufferToVideo {
        buffer: &pixels,
        src: BltRegion::Full,
        dest: ((width - 256) / 2, (height - 256) / 2),
        dims: (256, 256),
    });
}
