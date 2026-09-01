//! Minimal graphical boot splash for the normal Luna boot path.
//!
//! The splash deliberately uses only UEFI GOP primitives so the bootloader can
//! draw it before Linux starts. Verbose boot never calls this module.

use uefi::boot::{self, open_protocol_exclusive};
use uefi::proto::console::gop::{BltOp, BltPixel, GraphicsOutput};

/// Draw the Luna splash on the active GOP framebuffer.
///
/// The drawing uses a handful of rectangle fills rather than per-pixel BLT
/// calls so the firmware performs bounded work even on a high-resolution mode.
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
    let radius = width.min(height) / 14;
    let thickness = (radius / 3).max(8);
    let outer = radius + thickness;

    let left = cx.saturating_sub(outer);
    let right = (cx + outer).min(width);
    let top = cy.saturating_sub(outer);
    let bottom = (cy + outer).min(height);
    let ring_width = right.saturating_sub(left);
    let ring_height = bottom.saturating_sub(top);

    // Stylized open ring. Six rectangles give a recognizable mark while
    // avoiding expensive per-pixel graphics operations in firmware.
    let _ = gop.blt(BltOp::VideoFill {
        color: accent,
        dest: (left, top),
        dims: (ring_width, thickness),
    });
    let _ = gop.blt(BltOp::VideoFill {
        color: accent,
        dest: (left, bottom.saturating_sub(thickness)),
        dims: (ring_width, thickness),
    });
    let _ = gop.blt(BltOp::VideoFill {
        color: accent,
        dest: (left, top),
        dims: (thickness, ring_height),
    });
    let _ = gop.blt(BltOp::VideoFill {
        color: accent,
        dest: (right.saturating_sub(thickness), top),
        dims: (thickness, ring_height / 2),
    });
    let _ = gop.blt(BltOp::VideoFill {
        color: accent,
        dest: (cx, cy.saturating_sub(thickness / 2)),
        dims: (outer, thickness),
    });
    let _ = gop.blt(BltOp::VideoFill {
        color: background,
        dest: (left + thickness, top + thickness),
        dims: (
            right.saturating_sub(left + thickness * 2),
            bottom.saturating_sub(top + thickness * 2),
        ),
    });
}
