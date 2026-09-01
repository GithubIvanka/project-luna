//! Minimal graphical boot splash for the normal Luna boot path.
//!
//! The splash deliberately uses only UEFI GOP primitives so the bootloader can
//! draw it before Linux starts. Verbose boot never calls this module.

use uefi::boot::{self, open_protocol_exclusive};
use uefi::proto::console::gop::{BltOp, BltPixel, GraphicsOutput};

/// Draw the Luna splash on the active GOP framebuffer.
///
/// This is intentionally a very small drawing operation: a dark background,
/// the Luna ring mark and a wordmark. A failure to obtain GOP is non-fatal;
/// text output remains available to the boot menu/diagnostic path.
pub fn show() {
    let Ok(handle) = boot::get_handle_for_protocol::<GraphicsOutput>() else {
        return;
    };
    let Ok(mut gop) = open_protocol_exclusive::<GraphicsOutput>(handle) else {
        return;
    };

    let (width, height) = gop.current_mode_info().resolution();
    let background = BltPixel::new(8, 10, 14);
    let accent = BltPixel::new(150, 180, 255);

    let _ = gop.blt(BltOp::VideoFill {
        color: background,
        dest: (0, 0),
        dims: (width, height),
    });

    let cx = width / 2;
    let cy = height / 2;
    let ring = width.min(height) / 18;
    let inner = ring / 2;
    let outer = ring + inner;

    for y in cy.saturating_sub(outer)..(cy + outer).min(height) {
        for x in cx.saturating_sub(outer)..(cx + outer).min(width) {
            let dx = x as isize - cx as isize;
            let dy = y as isize - cy as isize;
            let radius2 = dx * dx + dy * dy;
            let outer2 = outer as isize * outer as isize;
            let inner2 = inner as isize * inner as isize;
            if radius2 <= outer2 && radius2 >= inner2 {
                let _ = gop.blt(BltOp::VideoFill {
                    color: accent,
                    dest: (x, y),
                    dims: (1, 1),
                });
            }
        }
    }
}
