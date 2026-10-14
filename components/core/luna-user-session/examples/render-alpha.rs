//! Локальные образцы нативного renderer и воспроизводимый статический знак UEFI.
//! Эти файлы не являются доказательством загрузки: QEMU сохраняет снимки отдельно.
#[allow(dead_code)]
#[path = "../src/renderer.rs"]
mod renderer;
#[allow(dead_code)]
#[path = "../src/rings.rs"]
mod rings;
use std::{fs::File, io::BufWriter, path::Path};
fn png(path: &Path, pixels: &[u8], width: u32, height: u32) {
    let file = BufWriter::new(File::create(path).unwrap());
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let bytes = pixels
        .chunks_exact(4)
        .flat_map(|p| [p[2], p[1], p[0]])
        .collect::<Vec<_>>();
    writer.write_image_data(&bytes).unwrap();
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    let assets = root.join("docs/design/luna-alpha-20261013/assets");
    let font=root.join("target/luna-files-operations-20261012/inputs-final/provider-root/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf");
    let renderer =
        renderer::LoginRenderer::from_resources(&font, &assets.join("wallpaper-night-sky.png"));
    let mut rings = rings::Rings::load_from(&assets.join("stone-reference.png")).unwrap();
    let mut pixels = vec![0; 1280 * 800 * 4];
    renderer.draw_background(&mut pixels, 1280 * 4, 1280, 800);
    rings.draw(&mut pixels, 1280 * 4, 1280, 800, 2.4, true);
    renderer.draw_boot_label(
        &mut pixels,
        1280 * 4,
        1280,
        800,
        "Подключение устройств и томов",
    );
    png(&out.join("native-boot.png"), &pixels, 1280, 800);
    let users = [renderer::LoginUserVisual {
        display_name: "Luna User",
        username: "luna",
    }];
    let mut view = renderer::LoginVisualState {
        users: &users,
        selected_user: 0,
        page: renderer::LoginPage::Password,
        password_len: 0,
        password_focused: true,
        error: false,
        cursor_visible: false,
        pointer_x: 0,
        pointer_y: 0,
        hovered_user: None,
    };
    renderer.draw_login_screen(&mut pixels, 1280 * 4, 1280, 800, &view);
    png(&out.join("native-login.png"), &pixels, 1280, 800);
    view.error = true;
    renderer.draw_login_screen(&mut pixels, 1280 * 4, 1280, 800, &view);
    png(&out.join("native-login-error.png"), &pixels, 1280, 800);
    let mut icon = vec![0; 256 * 376 * 4];
    for p in icon.chunks_exact_mut(4) {
        p.copy_from_slice(&[32, 15, 8, 0]);
    }
    rings.draw(&mut icon, 256 * 4, 256, 376, 2.4, true);
    std::fs::write(out.join("rings-uefi.xrgb"), &icon[..256 * 256 * 4]).unwrap();
}
