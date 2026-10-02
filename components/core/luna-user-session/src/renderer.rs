//! Native Luna login renderer.
//!
//! The renderer consumes only a writable XRGB framebuffer and declarative
//! login state. It does not depend on GTK, Wayland, fontconfig or a desktop
//! compositor.

use std::fs;
use std::io::BufReader;
use std::path::Path;

use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};

const BG: u32 = 0x000B1020;
const PANEL: u32 = 0x001A284D;
const FIELD: u32 = 0x00111C33;
const BORDER: u32 = 0x002F4B7A;
const ACCENT: u32 = 0x003B82F6;
const CYAN: u32 = 0x007EE0FF;
const TEXT: u32 = 0x00F4F8FF;
const MUTED: u32 = 0x008EA9CF;
const ERROR: u32 = 0x00FF7B72;
const AVATAR: u32 = 0x001A284D;

const LOGIN_FONT_PATHS: &[&str] = &[
    "/resources/fonts/JetBrainsMonoNerdFontMono-Regular.ttf",
    "/usr/share/fonts/truetype/luna/JetBrainsMonoNerdFontMono-Regular.ttf",
];
const LOGIN_ART_PATHS: &[&str] = &[
    "/resources/images/luna.png",
    "/usr/share/luna/luna-login.png",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginPage {
    UserSelection,
    Password,
}

#[derive(Clone, Copy, Debug)]
pub struct LoginUserVisual<'a> {
    pub display_name: &'a str,
    pub username: &'a str,
}

#[derive(Clone, Copy, Debug)]
pub struct LoginVisualState<'a> {
    pub users: &'a [LoginUserVisual<'a>],
    pub selected_user: usize,
    pub page: LoginPage,
    pub password_len: usize,
    pub password_focused: bool,
    pub error: bool,
    pub cursor_visible: bool,
    pub pointer_x: i32,
    pub pointer_y: i32,
    pub hovered_user: Option<usize>,
}

#[derive(Debug)]
struct LoginBackground {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[derive(Debug)]
enum TextBackend {
    TrueType(fontdue::Font),
    BitmapFallback,
}

#[derive(Debug)]
pub struct LoginRenderer {
    text: TextBackend,
    background: Option<LoginBackground>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x
            && x < self.x.saturating_add(self.w)
            && y >= self.y
            && y < self.y.saturating_add(self.h)
    }
}

impl LoginRenderer {
    pub fn load_default() -> Self {
        for path in LOGIN_FONT_PATHS {
            if let Some(font) = load_font(Path::new(path)) {
                eprintln!("luna-user-session: login renderer font loaded from {path}");
                return Self {
                    text: TextBackend::TrueType(font),
                    background: load_background(),
                };
            }
        }

        eprintln!(
            "luna-user-session: login renderer font resource unavailable; using bitmap fallback"
        );
        Self {
            text: TextBackend::BitmapFallback,
            background: load_background(),
        }
    }

    pub fn draw_login_screen(
        &self,
        pixels: &mut [u8],
        pitch: usize,
        width: u32,
        height: u32,
        view: &LoginVisualState<'_>,
    ) {
        let layout = LoginLayout::for_viewport(width, height);
        self.draw_background(pixels, pitch, width, height);
        self.draw_login_panel(pixels, pitch, width, height, &layout);
        self.draw_brand(pixels, pitch, width, height, layout.brand_y, layout.compact);

        match view.page {
            LoginPage::UserSelection => {
                self.centered_text(
                    pixels,
                    pitch,
                    width,
                    height,
                    layout.title_y,
                    if layout.compact { 20.0 } else { 26.0 },
                    "SELECT USER",
                    MUTED,
                );
                for index in 0..view.users.len() {
                    if let Some(rect) = layout.user_card_rect(index, view.users.len()) {
                        let selected = index == view.selected_user;
                        let hovered = Some(index) == view.hovered_user;
                        self.draw_user_card(
                            pixels,
                            pitch,
                            width,
                            height,
                            rect,
                            &view.users[index],
                            selected,
                            hovered,
                            layout.compact,
                        );
                    }
                }
                self.centered_text(
                    pixels,
                    pitch,
                    width,
                    height,
                    layout.helper_y,
                    if layout.compact { 11.0 } else { 13.0 },
                    "CLICK OR ARROWS TO SELECT  ENTER TO CONTINUE",
                    MUTED,
                );
            }
            LoginPage::Password => {
                if let Some(user) = view.users.get(view.selected_user) {
                    draw_avatar(
                        pixels,
                        pitch,
                        width,
                        height,
                        layout.user_icon_x,
                        layout.user_icon_y,
                        if layout.compact { 22 } else { 28 },
                        user.display_name,
                        true,
                        false,
                        &self.text,
                    );
                    self.draw_text(
                        pixels,
                        pitch,
                        width,
                        height,
                        layout.user_name_x,
                        layout.user_name_y,
                        if layout.compact { 17.0 } else { 22.0 },
                        user.display_name,
                        TEXT,
                    );
                    self.draw_text(
                        pixels,
                        pitch,
                        width,
                        height,
                        layout.user_name_x,
                        layout.user_username_y,
                        if layout.compact { 11.0 } else { 13.0 },
                        user.username,
                        MUTED,
                    );
                }

                let password_value = if view.password_len == 0 {
                    String::new()
                } else {
                    "*".repeat(view.password_len)
                };
                self.draw_field(
                    pixels,
                    pitch,
                    width,
                    height,
                    layout.field_x,
                    layout.password_field_y,
                    layout.field_w,
                    layout.field_h,
                    &password_value,
                    view.password_focused,
                    view.cursor_visible && view.password_len > 0,
                    if layout.compact { 14.0 } else { 17.0 },
                );
                if view.password_len == 0 && view.password_focused {
                    self.draw_text(
                        pixels,
                        pitch,
                        width,
                        height,
                        layout.field_x + 18,
                        layout.password_field_y + if layout.compact { 13 } else { 18 },
                        if layout.compact { 14.0 } else { 17.0 },
                        "ENTER PASSWORD",
                        MUTED,
                    );
                }
                self.draw_login_button(
                    pixels,
                    pitch,
                    width,
                    height,
                    layout.button_x,
                    layout.button_y,
                    layout.button_size,
                    view.password_focused,
                );
                self.centered_text(
                    pixels,
                    pitch,
                    width,
                    height,
                    layout.helper_y,
                    if layout.compact { 11.0 } else { 13.0 },
                    if view.error {
                        "PASSWORD INCORRECT"
                    } else if view.users.len() > 1 {
                        "ESC TO CHOOSE ANOTHER USER"
                    } else {
                        "ESC TO CANCEL"
                    },
                    if view.error { ERROR } else { MUTED },
                );
            }
        }

        if view.pointer_x >= 0 && view.pointer_y >= 0 {
            self.draw_pointer(pixels, pitch, width, height, view.pointer_x, view.pointer_y);
        }
    }

    fn draw_login_panel(
        &self,
        pixels: &mut [u8],
        pitch: usize,
        width: u32,
        height: u32,
        layout: &LoginLayout,
    ) {
        fill_rect(
            pixels,
            pitch,
            width,
            height,
            layout.panel_x,
            layout.panel_y,
            layout.panel_w,
            layout.panel_h,
            0x00111C33,
        );
        stroke_rect(
            pixels,
            pitch,
            width,
            height,
            layout.panel_x,
            layout.panel_y,
            layout.panel_w,
            layout.panel_h,
            BORDER,
        );
        let inset = 1;
        stroke_rect(
            pixels,
            pitch,
            width,
            height,
            layout.panel_x + inset,
            layout.panel_y + inset,
            layout.panel_w.saturating_sub(inset * 2),
            layout.panel_h.saturating_sub(inset * 2),
            0x001C3358,
        );
        fill_rect(
            pixels,
            pitch,
            width,
            height,
            layout.panel_x,
            layout.panel_y,
            2,
            layout.panel_h,
            ACCENT,
        );
    }

    fn draw_background(&self, pixels: &mut [u8], pitch: usize, width: u32, height: u32) {
        fill_rect(pixels, pitch, width, height, 0, 0, width, height, BG);

        if let Some(image) = &self.background {
            let art_width = width.saturating_mul(if width < 900 { 58 } else { 52 }) / 100;
            let art_height = height;
            let x = 0;
            let y = 0;
            draw_image_cover(
                pixels, pitch, width, height, image, x, y, art_width, art_height, 178,
            );
        }

        // Blend the artwork into the concept palette so it reads as atmosphere,
        // not as a separate panel.
        let fade_start = width.saturating_mul(28) / 100;
        let fade_end = width.saturating_mul(66) / 100;
        for x in fade_start..fade_end {
            let span = fade_end.saturating_sub(fade_start).max(1);
            let alpha = ((x - fade_start) * 220 / span) as u8;
            blend_vertical_column(pixels, pitch, width, height, x, BG, alpha);
        }
        fill_rect(
            pixels,
            pitch,
            width,
            height,
            width.saturating_mul(64) / 100,
            0,
            width.saturating_sub(width.saturating_mul(64) / 100),
            height,
            BG,
        );
    }

    fn draw_brand(
        &self,
        pixels: &mut [u8],
        pitch: usize,
        width: u32,
        height: u32,
        y: u32,
        compact: bool,
    ) {
        let center_x = width
            .saturating_mul(55)
            .saturating_div(100)
            .saturating_add(width.saturating_mul(40).saturating_div(100) / 2);
        let logo_radius = if compact { 22 } else { 28 };
        draw_orbit_logo(pixels, pitch, width, height, center_x, y, logo_radius);
        self.centered_text_at(
            pixels,
            pitch,
            width,
            height,
            center_x,
            y + logo_radius + if compact { 22 } else { 28 },
            if compact { 16.0 } else { 20.0 },
            "L U N A   O S",
            TEXT,
        );
    }

    fn draw_login_button(
        &self,
        pixels: &mut [u8],
        pitch: usize,
        width: u32,
        height: u32,
        x: u32,
        y: u32,
        size: u32,
        active: bool,
    ) {
        let center_x = x + size / 2;
        let center_y = y + size / 2;
        let color = if active { ACCENT } else { BORDER };
        fill_circle(
            pixels,
            pitch,
            width,
            height,
            center_x,
            center_y,
            size / 2,
            color,
        );
        fill_circle(
            pixels,
            pitch,
            width,
            height,
            center_x,
            center_y,
            size.saturating_sub(6) / 2,
            0x00214A82,
        );
        let arm = size / 4;
        fill_rect(
            pixels,
            pitch,
            width,
            height,
            center_x.saturating_sub(arm / 2),
            center_y.saturating_sub(2),
            arm,
            4,
            TEXT,
        );
        fill_rect(
            pixels,
            pitch,
            width,
            height,
            center_x + arm.saturating_sub(2),
            center_y.saturating_sub(arm / 2),
            4,
            arm,
            TEXT,
        );
        fill_rect(
            pixels,
            pitch,
            width,
            height,
            center_x + arm,
            center_y.saturating_add(arm / 2).saturating_sub(2),
            4,
            arm / 2 + 2,
            TEXT,
        );
    }

    fn draw_user_card(
        &self,
        pixels: &mut [u8],
        pitch: usize,
        width: u32,
        height: u32,
        rect: Rect,
        user: &LoginUserVisual<'_>,
        selected: bool,
        hovered: bool,
        compact: bool,
    ) {
        let border = if selected || hovered { ACCENT } else { BORDER };
        let background = if selected { PANEL } else { FIELD };
        fill_rect(
            pixels, pitch, width, height, rect.x, rect.y, rect.w, rect.h, background,
        );
        stroke_rect(
            pixels, pitch, width, height, rect.x, rect.y, rect.w, rect.h, border,
        );

        let radius = if compact { 25 } else { 34 };
        draw_avatar(
            pixels,
            pitch,
            width,
            height,
            rect.x + rect.w / 2,
            rect.y + radius + 12,
            radius,
            user.display_name,
            selected,
            hovered,
            &self.text,
        );

        self.centered_text_at(
            pixels,
            pitch,
            width,
            height,
            rect.x + rect.w / 2,
            rect.y + radius * 2 + 30,
            if compact { 15.0 } else { 19.0 },
            user.display_name,
            TEXT,
        );
        self.centered_text_at(
            pixels,
            pitch,
            width,
            height,
            rect.x + rect.w / 2,
            rect.y + radius * 2 + if compact { 50 } else { 60 },
            if compact { 11.0 } else { 14.0 },
            user.username,
            MUTED,
        );
    }

    fn draw_field(
        &self,
        pixels: &mut [u8],
        pitch: usize,
        width: u32,
        height: u32,
        x: u32,
        y: u32,
        field_width: u32,
        field_height: u32,
        value: &str,
        active: bool,
        cursor_visible: bool,
        size: f32,
    ) {
        fill_rect(
            pixels,
            pitch,
            width,
            height,
            x,
            y,
            field_width,
            field_height,
            FIELD,
        );
        stroke_rect(
            pixels,
            pitch,
            width,
            height,
            x,
            y,
            field_width,
            field_height,
            if active { ACCENT } else { BORDER },
        );

        let text_y = y + field_height.saturating_sub(size.round() as u32) / 2;
        self.draw_text(
            pixels,
            pitch,
            width,
            height,
            x + 18,
            text_y,
            size,
            value,
            TEXT,
        );

        if active && cursor_visible {
            let cursor_x = x + 18 + self.text_width(value, size).round() as u32 + 3;
            if cursor_x < x.saturating_add(field_width) {
                fill_rect(
                    pixels,
                    pitch,
                    width,
                    height,
                    cursor_x,
                    y + 9,
                    3,
                    field_height.saturating_sub(18),
                    TEXT,
                );
            }
        }
    }

    fn centered_text(
        &self,
        pixels: &mut [u8],
        pitch: usize,
        width: u32,
        height: u32,
        y: u32,
        size: f32,
        value: &str,
        color: u32,
    ) {
        self.centered_text_at(
            pixels,
            pitch,
            width,
            height,
            width / 2,
            y,
            size,
            value,
            color,
        );
    }

    fn centered_text_at(
        &self,
        pixels: &mut [u8],
        pitch: usize,
        width: u32,
        height: u32,
        center_x: u32,
        y: u32,
        size: f32,
        value: &str,
        color: u32,
    ) {
        let text_width = self.text_width(value, size).round() as u32;
        let x = center_x.saturating_sub(text_width / 2);
        self.draw_text(pixels, pitch, width, height, x, y, size, value, color);
    }

    fn text_width(&self, value: &str, size: f32) -> f32 {
        match &self.text {
            TextBackend::TrueType(font) => {
                let mut width = 0.0;
                let mut previous = None;
                for ch in value.chars() {
                    if let Some(prev) = previous {
                        width += font.horizontal_kern(prev, ch, size).unwrap_or(0.0);
                    }
                    width += font.metrics(ch, size).advance_width;
                    previous = Some(ch);
                }
                width
            }
            TextBackend::BitmapFallback => bitmap_text_width(value, fallback_scale(size)) as f32,
        }
    }

    fn draw_text(
        &self,
        pixels: &mut [u8],
        pitch: usize,
        width: u32,
        height: u32,
        x: u32,
        y: u32,
        size: f32,
        value: &str,
        color: u32,
    ) {
        match &self.text {
            TextBackend::TrueType(font) => {
                let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
                layout.reset(&LayoutSettings {
                    x: x as f32,
                    y: y as f32,
                    ..LayoutSettings::default()
                });
                layout.append(
                    std::slice::from_ref(font),
                    &TextStyle {
                        text: value,
                        px: size,
                        font_index: 0,
                        user_data: (),
                    },
                );
                for glyph in layout.glyphs() {
                    let (metrics, bitmap) = font.rasterize_config(glyph.key);
                    let gx = glyph.x.floor() as i32;
                    let gy = glyph.y.floor() as i32;
                    for row in 0..metrics.height {
                        for column in 0..metrics.width {
                            let coverage = bitmap[row * metrics.width + column];
                            if coverage == 0 {
                                continue;
                            }
                            blend_pixel(
                                pixels,
                                pitch,
                                width,
                                height,
                                gx + column as i32,
                                gy + row as i32,
                                color,
                                coverage,
                            );
                        }
                    }
                }
            }
            TextBackend::BitmapFallback => {
                draw_bitmap_text(
                    pixels,
                    pitch,
                    width,
                    height,
                    x,
                    y,
                    fallback_scale(size),
                    value,
                    color,
                );
            }
        }
    }

    pub(crate) fn draw_pointer(
        &self,
        pixels: &mut [u8],
        pitch: usize,
        width: u32,
        height: u32,
        x: i32,
        y: i32,
    ) {
        let x = x.clamp(0, width.saturating_sub(1) as i32) as u32;
        let y = y.clamp(0, height.saturating_sub(1) as i32) as u32;
        fill_rect(pixels, pitch, width, height, x, y, 2, 18, TEXT);
        fill_rect(pixels, pitch, width, height, x, y, 12, 2, TEXT);
        fill_rect(pixels, pitch, width, height, x + 2, y + 14, 6, 2, TEXT);
        fill_rect(pixels, pitch, width, height, x + 9, y + 9, 3, 3, TEXT);
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LoginLayout {
    panel_x: u32,
    pub(crate) panel_y: u32,
    panel_w: u32,
    panel_h: u32,
    pub(crate) field_x: u32,
    pub(crate) field_w: u32,
    pub(crate) field_h: u32,
    pub(crate) title_y: u32,
    pub(crate) password_field_y: u32,
    pub(crate) helper_y: u32,
    pub(crate) brand_y: u32,
    pub(crate) user_icon_x: u32,
    pub(crate) user_icon_y: u32,
    pub(crate) user_name_x: u32,
    pub(crate) user_name_y: u32,
    pub(crate) user_username_y: u32,
    pub(crate) button_x: u32,
    pub(crate) button_y: u32,
    pub(crate) button_size: u32,
    compact: bool,
}

impl LoginLayout {
    pub(crate) fn for_viewport(width: u32, height: u32) -> Self {
        let compact = width < 900 || height < 600;
        let tiny = height < 400;
        let panel_w = width
            .saturating_mul(if compact { 88 } else { 40 })
            .saturating_div(100)
            .min(width.saturating_sub(32))
            .max(if compact { 320 } else { 420 })
            .min(width.saturating_sub(24));
        let panel_h = height
            .saturating_sub(if compact { 48 } else { 96 })
            .min(620);
        let panel_left = width
            .saturating_mul(if compact { 6 } else { 55 })
            .saturating_div(100)
            .min(width.saturating_sub(panel_w));
        let panel_top = (height.saturating_sub(panel_h)) / 2;
        let field_w = panel_w
            .saturating_sub(if compact { 40 } else { 72 })
            .min(360);
        let form_center_x = panel_left + panel_w / 2;
        let field_x = form_center_x.saturating_sub(field_w / 2);
        let field_h = if compact { 42 } else { 48 };
        let button_size = if compact { 44 } else { 52 };
        let button_x = form_center_x.saturating_sub(button_size / 2);
        let content_top = panel_top
            + if tiny {
                10
            } else if compact {
                22
            } else {
                34
            };
        let password_field_y = content_top
            + if tiny {
                94
            } else if compact {
                286
            } else {
                300
            };
        let button_y = password_field_y
            + field_h
            + if tiny {
                22
            } else if compact {
                34
            } else {
                38
            };
        let user_icon_y = content_top
            + if tiny {
                40
            } else if compact {
                150
            } else {
                164
            };
        let user_name_y = user_icon_y
            + if tiny {
                28
            } else if compact {
                34
            } else {
                42
            };
        let user_username_y = user_name_y
            + if tiny {
                20
            } else if compact {
                26
            } else {
                30
            };

        Self {
            panel_x: panel_left,
            panel_y: panel_top,
            panel_w,
            panel_h,
            field_x,
            field_w,
            field_h,
            title_y: panel_top + if compact { 62 } else { 72 },
            password_field_y,
            helper_y: button_y
                + button_size
                + if tiny {
                    8
                } else if compact {
                    22
                } else {
                    28
                },
            brand_y: panel_top + if compact { 18 } else { 28 },
            user_icon_x: field_x + if compact { 24 } else { 30 },
            user_icon_y,
            user_name_x: field_x + if compact { 58 } else { 70 },
            user_name_y,
            user_username_y,
            button_x,
            button_y,
            button_size,
            compact,
        }
    }

    fn user_card_rect(&self, index: usize, count: usize) -> Option<Rect> {
        if index >= count || count == 0 {
            return None;
        }
        let cols = user_grid_columns(count);
        let rows = count.div_ceil(cols);
        let gap = if self.compact { 10 } else { 14 };
        let card_w = self
            .field_w
            .saturating_sub(gap * (cols.saturating_sub(1) as u32))
            / cols as u32;
        let top = self.panel_y + if self.compact { 118 } else { 180 };
        let card_h = if rows > 1 { 104 } else { 112 };
        let row = index / cols;
        let col = index % cols;
        Some(Rect {
            x: self.field_x + col as u32 * (card_w + gap),
            y: top + row as u32 * (card_h + gap),
            w: card_w,
            h: card_h,
        })
    }

    pub(crate) fn password_rect(&self) -> Rect {
        Rect {
            x: self.field_x,
            y: self.password_field_y,
            w: self.field_w,
            h: self.field_h,
        }
    }

    pub(crate) fn button_rect(&self) -> Rect {
        Rect {
            x: self.button_x,
            y: self.button_y,
            w: self.button_size,
            h: self.button_size,
        }
    }
}

pub(crate) fn user_grid_columns(count: usize) -> usize {
    match count {
        0 | 1 => 1,
        2..=3 => count,
        _ => 3,
    }
}

pub(crate) fn hit_test_user(
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    count: usize,
) -> Option<usize> {
    let layout = LoginLayout::for_viewport(width, height);
    (0..count).find(|&index| {
        layout
            .user_card_rect(index, count)
            .is_some_and(|rect| rect.contains(x, y))
    })
}

pub(crate) fn hit_test_password(width: u32, height: u32, x: u32, y: u32) -> bool {
    LoginLayout::for_viewport(width, height)
        .password_rect()
        .contains(x, y)
}

pub(crate) fn hit_test_login_button(width: u32, height: u32, x: u32, y: u32) -> bool {
    LoginLayout::for_viewport(width, height)
        .button_rect()
        .contains(x, y)
}

fn load_font(path: &Path) -> Option<fontdue::Font> {
    let data = fs::read(path).ok()?;
    fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).ok()
}

fn load_background() -> Option<LoginBackground> {
    for path in LOGIN_ART_PATHS {
        let file = fs::File::open(path).ok()?;
        let mut decoder = png::Decoder::new(BufReader::new(file));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().ok()?;
        let mut bytes = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut bytes).ok()?;
        let bytes = &bytes[..info.buffer_size()];
        let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
        match info.color_type {
            png::ColorType::Rgb => {
                for pixel in bytes.chunks_exact(3) {
                    rgba.extend_from_slice(pixel);
                    rgba.push(255);
                }
            }
            png::ColorType::Rgba => rgba.extend_from_slice(bytes),
            png::ColorType::Grayscale => {
                for &value in bytes {
                    rgba.extend_from_slice(&[value, value, value, 255]);
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for pixel in bytes.chunks_exact(2) {
                    rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
                }
            }
            png::ColorType::Indexed => return None,
        }
        eprintln!(
            "luna-user-session: login renderer background loaded from {} ({}x{})",
            path, info.width, info.height
        );
        return Some(LoginBackground {
            width: info.width,
            height: info.height,
            rgba,
        });
    }
    eprintln!("luna-user-session: login renderer background unavailable");
    None
}

fn draw_image_cover(
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    image: &LoginBackground,
    x: u32,
    y: u32,
    draw_width: u32,
    draw_height: u32,
    alpha: u8,
) {
    if draw_width == 0 || draw_height == 0 || image.width == 0 || image.height == 0 {
        return;
    }
    let scale_x = draw_width as f32 / image.width as f32;
    let scale_y = draw_height as f32 / image.height as f32;
    let scale = scale_x.max(scale_y);
    let source_width = (draw_width as f32 / scale).max(1.0) as u32;
    let source_height = (draw_height as f32 / scale).max(1.0) as u32;
    let source_x = image.width.saturating_sub(source_width) / 2;
    let source_y = image.height.saturating_sub(source_height) / 2;
    for dy in 0..draw_height {
        let py = y.saturating_add(dy);
        if py >= height {
            continue;
        }
        let sy = source_y + ((dy as f32 / draw_height as f32) * source_height as f32) as u32;
        for dx in 0..draw_width {
            let px = x.saturating_add(dx);
            if px >= width {
                continue;
            }
            let sx = source_x + ((dx as f32 / draw_width as f32) * source_width as f32) as u32;
            let src =
                ((sy.min(image.height - 1) * image.width + sx.min(image.width - 1)) * 4) as usize;
            let color = (u32::from(image.rgba[src]) << 16)
                | (u32::from(image.rgba[src + 1]) << 8)
                | u32::from(image.rgba[src + 2]);
            blend_pixel(
                pixels,
                pitch,
                width,
                height,
                px as i32,
                py as i32,
                color,
                ((u16::from(image.rgba[src + 3]) * u16::from(alpha)) / 255) as u8,
            );
        }
    }
}

fn blend_vertical_column(
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    x: u32,
    color: u32,
    alpha: u8,
) {
    for y in 0..height {
        blend_pixel(
            pixels, pitch, width, height, x as i32, y as i32, color, alpha,
        );
    }
}

fn draw_orbit_logo(
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    center_x: u32,
    center_y: u32,
    radius: u32,
) {
    let rx = radius.saturating_mul(2);
    let ry = radius.saturating_mul(3) / 4;
    for &(angle, scale_x, scale_y) in &[
        (0.0f32, 1.0f32, 0.62f32),
        (0.95f32, 0.85f32, 0.72f32),
        (-0.95f32, 0.85f32, 0.72f32),
    ] {
        stroke_ellipse(
            pixels,
            pitch,
            width,
            height,
            center_x,
            center_y,
            (rx as f32 * scale_x) as u32,
            (ry as f32 / scale_y) as u32,
            angle,
            CYAN,
            2,
        );
    }
    fill_circle(
        pixels,
        pitch,
        width,
        height,
        center_x,
        center_y,
        radius / 4,
        0x005EA7FF,
    );
}

fn stroke_ellipse(
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    center_x: u32,
    center_y: u32,
    radius_x: u32,
    radius_y: u32,
    angle: f32,
    color: u32,
    thickness: u32,
) {
    if radius_x == 0 || radius_y == 0 {
        return;
    }
    let (sin_a, cos_a) = angle.sin_cos();
    let steps = 180usize;
    for step in 0..steps {
        let theta = step as f32 * std::f32::consts::TAU / steps as f32;
        let (sin_t, cos_t) = theta.sin_cos();
        let x = cos_t * radius_x as f32;
        let y = sin_t * radius_y as f32;
        let px = center_x as f32 + x * cos_a - y * sin_a;
        let py = center_y as f32 + x * sin_a + y * cos_a;
        let px = px.round() as i32;
        let py = py.round() as i32;
        for oy in 0..thickness {
            for ox in 0..thickness {
                blend_pixel(
                    pixels,
                    pitch,
                    width,
                    height,
                    px + ox as i32 - thickness as i32 / 2,
                    py + oy as i32 - thickness as i32 / 2,
                    color,
                    210,
                );
            }
        }
    }
}

fn draw_avatar(
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    center_x: u32,
    center_y: u32,
    radius: u32,
    display_name: &str,
    selected: bool,
    hovered: bool,
    text: &TextBackend,
) {
    let ring = if selected || hovered { ACCENT } else { BORDER };
    fill_circle(
        pixels,
        pitch,
        width,
        height,
        center_x,
        center_y,
        radius + 3,
        ring,
    );
    fill_circle(
        pixels, pitch, width, height, center_x, center_y, radius, AVATAR,
    );

    let initials = avatar_initials(display_name);
    draw_text_backend(
        text,
        pixels,
        pitch,
        width,
        height,
        center_x.saturating_sub(12),
        center_y.saturating_sub(11),
        22.0,
        &initials,
        TEXT,
    );
}

fn avatar_initials(value: &str) -> String {
    let mut chars = value
        .split_whitespace()
        .flat_map(|word| word.chars())
        .filter(|ch| ch.is_alphanumeric())
        .take(2)
        .collect::<Vec<_>>();
    if chars.is_empty() {
        chars.push('?');
    }
    chars.into_iter().collect()
}

fn draw_text_backend(
    text: &TextBackend,
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    size: f32,
    value: &str,
    color: u32,
) {
    match text {
        TextBackend::TrueType(font) => {
            let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
            layout.reset(&LayoutSettings {
                x: x as f32,
                y: y as f32,
                ..LayoutSettings::default()
            });
            layout.append(
                std::slice::from_ref(font),
                &TextStyle {
                    text: value,
                    px: size,
                    font_index: 0,
                    user_data: (),
                },
            );
            for glyph in layout.glyphs() {
                let (metrics, bitmap) = font.rasterize_config(glyph.key);
                let gx = glyph.x.floor() as i32;
                let gy = glyph.y.floor() as i32;
                for row in 0..metrics.height {
                    for column in 0..metrics.width {
                        let coverage = bitmap[row * metrics.width + column];
                        if coverage != 0 {
                            blend_pixel(
                                pixels,
                                pitch,
                                width,
                                height,
                                gx + column as i32,
                                gy + row as i32,
                                color,
                                coverage,
                            );
                        }
                    }
                }
            }
        }
        TextBackend::BitmapFallback => {
            draw_bitmap_text(
                pixels,
                pitch,
                width,
                height,
                x,
                y,
                fallback_scale(size),
                value,
                color,
            );
        }
    }
}

fn blend_pixel(
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    color: u32,
    coverage: u8,
) {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return;
    }
    let offset = y as usize * pitch + x as usize * 4;
    if offset + 4 > pixels.len() {
        return;
    }

    let src = color.to_ne_bytes();
    let dst = &mut pixels[offset..offset + 4];
    let alpha = u32::from(coverage);
    let inverse = 255 - alpha;
    for channel in 0..3 {
        dst[channel] = ((u32::from(src[channel]) * alpha + u32::from(dst[channel]) * inverse + 127)
            / 255) as u8;
    }
    dst[3] = 0;
}

fn fill_circle(
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    center_x: u32,
    center_y: u32,
    radius: u32,
    color: u32,
) {
    let r = radius as i32;
    for y in center_y.saturating_sub(radius)..=(center_y + radius).min(height.saturating_sub(1)) {
        for x in center_x.saturating_sub(radius)..=(center_x + radius).min(width.saturating_sub(1))
        {
            let dx = x as i32 - center_x as i32;
            let dy = y as i32 - center_y as i32;
            if dx * dx + dy * dy <= r * r {
                let offset = y as usize * pitch + x as usize * 4;
                if offset + 4 <= pixels.len() {
                    pixels[offset..offset + 4].copy_from_slice(&color.to_ne_bytes());
                }
            }
        }
    }
}

fn fill_rect(
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    color: u32,
) {
    let x0 = x.min(width);
    let x1 = x.saturating_add(w).min(width);
    let y0 = y.min(height);
    let y1 = y.saturating_add(h).min(height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let bytes = color.to_ne_bytes();
    for row in y0..y1 {
        let base = row as usize * pitch + x0 as usize * 4;
        let len = (x1 - x0) as usize;
        for pixel in pixels[base..base + len * 4].chunks_exact_mut(4) {
            pixel.copy_from_slice(&bytes);
        }
    }
}

fn stroke_rect(
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    color: u32,
) {
    fill_rect(pixels, pitch, width, height, x, y, w, 2, color);
    fill_rect(
        pixels,
        pitch,
        width,
        height,
        x,
        y.saturating_add(h).saturating_sub(2),
        w,
        2,
        color,
    );
    fill_rect(pixels, pitch, width, height, x, y, 2, h, color);
    fill_rect(
        pixels,
        pitch,
        width,
        height,
        x.saturating_add(w).saturating_sub(2),
        y,
        2,
        h,
        color,
    );
}

fn fallback_scale(size: f32) -> u32 {
    if size < 16.0 {
        2
    } else if size < 26.0 {
        3
    } else if size < 44.0 {
        5
    } else {
        7
    }
}

fn bitmap_text_width(value: &str, scale: u32) -> u32 {
    value.chars().count() as u32 * 6 * scale
}

fn draw_bitmap_text(
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    scale: u32,
    value: &str,
    color: u32,
) {
    let mut cursor_x = x;
    for ch in value.chars() {
        for (row, bits) in glyph(ch.to_ascii_uppercase()).iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) != 0 {
                    fill_rect(
                        pixels,
                        pitch,
                        width,
                        height,
                        cursor_x + column * scale,
                        y + row as u32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
        cursor_x += 6 * scale;
    }
}

fn glyph(ch: char) -> &'static [u8; 7] {
    match ch {
        'A' => &[0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => &[0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => &[0x0f, 0x10, 0x10, 0x10, 0x10, 0x10, 0x0f],
        'D' => &[0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        'E' => &[0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => &[0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => &[0x0f, 0x10, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => &[0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => &[0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x1f],
        'J' => &[0x07, 0x02, 0x02, 0x02, 0x12, 0x12, 0x0c],
        'K' => &[0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => &[0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => &[0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => &[0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => &[0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => &[0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => &[0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => &[0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => &[0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => &[0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => &[0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => &[0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => &[0x11, 0x11, 0x15, 0x15, 0x15, 0x0a, 0x0a],
        'X' => &[0x11, 0x0a, 0x04, 0x04, 0x0a, 0x11, 0x11],
        'Y' => &[0x11, 0x0a, 0x04, 0x04, 0x04, 0x04, 0x04],
        'Z' => &[0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        '0' => &[0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => &[0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => &[0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => &[0x1e, 0x01, 0x01, 0x0e, 0x01, 0x01, 0x1e],
        '4' => &[0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => &[0x1f, 0x10, 0x10, 0x1e, 0x01, 0x01, 0x1e],
        '6' => &[0x0e, 0x10, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => &[0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => &[0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => &[0x0e, 0x11, 0x11, 0x0f, 0x01, 0x01, 0x0e],
        ':' => &[0, 0x04, 0, 0, 0x04, 0, 0],
        '-' => &[0, 0, 0, 0x1f, 0, 0, 0],
        '_' => &[0, 0, 0, 0, 0, 0, 0x1f],
        '.' => &[0, 0, 0, 0, 0, 0x06, 0x06],
        '/' => &[0x01, 0x02, 0x04, 0x04, 0x08, 0x10, 0x10],
        '*' => &[0, 0x0a, 0x04, 0x1f, 0x04, 0x0a, 0],
        ' ' => &[0, 0, 0, 0, 0, 0, 0],
        _ => &[0x0e, 0x11, 0x01, 0x02, 0x04, 0, 0x04],
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BG, LoginLayout, LoginPage, LoginRenderer, LoginUserVisual, LoginVisualState, TextBackend,
        hit_test_password, hit_test_user, user_grid_columns,
    };

    #[test]
    fn compact_layout_stays_inside_small_viewport() {
        let layout = LoginLayout::for_viewport(320, 240);
        assert!(layout.panel_x + layout.panel_w <= 320);
        assert!(layout.panel_y + layout.panel_h <= 240);
        assert!(layout.field_x + layout.field_w <= 320);
        assert!(layout.password_field_y + layout.field_h <= 240);
    }

    #[test]
    fn grid_columns_and_hit_tests_match() {
        assert_eq!(user_grid_columns(1), 1);
        assert_eq!(user_grid_columns(2), 2);
        assert_eq!(user_grid_columns(4), 3);
        let layout = LoginLayout::for_viewport(1280, 800);
        let first = layout.user_card_rect(0, 4).expect("first user card");
        let second = layout.user_card_rect(1, 4).expect("second user card");
        assert_eq!(
            hit_test_user(1280, 800, first.x + first.w / 2, first.y + first.h / 2, 4),
            Some(0)
        );
        assert_eq!(
            hit_test_user(
                1280,
                800,
                second.x + second.w / 2,
                second.y + second.h / 2,
                4
            ),
            Some(1)
        );
        let password = layout.password_rect();
        assert!(hit_test_password(
            1280,
            800,
            password.x + password.w / 2,
            password.y + password.h / 2
        ));
        let button = layout.button_rect();
        assert!(button.contains(button.x + button.w / 2, button.y + button.h / 2));
    }

    #[test]
    fn renderer_draws_single_user_password_frame() {
        let renderer = LoginRenderer {
            text: TextBackend::BitmapFallback,
            background: None,
        };
        let users = [LoginUserVisual {
            display_name: "Luna User",
            username: "luna",
        }];
        let mut pixels = vec![0u8; 800 * 600 * 4];
        renderer.draw_login_screen(
            &mut pixels,
            800 * 4,
            800,
            600,
            &LoginVisualState {
                users: &users,
                selected_user: 0,
                page: LoginPage::Password,
                password_len: 0,
                password_focused: true,
                error: false,
                cursor_visible: true,
                pointer_x: 400,
                pointer_y: 300,
                hovered_user: None,
            },
        );
        assert!(pixels.chunks_exact(4).any(|p| p != BG.to_ne_bytes()));
        let password = LoginLayout::for_viewport(800, 600).password_rect();
        assert!(hit_test_password(
            800,
            600,
            password.x + password.w / 2,
            password.y + password.h / 2
        ));
    }

    #[test]
    fn renderer_draws_user_selection_frame() {
        let renderer = LoginRenderer {
            text: TextBackend::BitmapFallback,
            background: None,
        };
        let users = [LoginUserVisual {
            display_name: "Luna User",
            username: "luna",
        }];
        let mut pixels = vec![0u8; 800 * 600 * 4];
        renderer.draw_login_screen(
            &mut pixels,
            800 * 4,
            800,
            600,
            &LoginVisualState {
                users: &users,
                selected_user: 0,
                page: LoginPage::UserSelection,
                password_len: 0,
                password_focused: false,
                error: false,
                cursor_visible: true,
                pointer_x: 400,
                pointer_y: 300,
                hovered_user: Some(0),
            },
        );
        assert!(pixels.chunks_exact(4).any(|p| p != BG.to_ne_bytes()));
    }
}
