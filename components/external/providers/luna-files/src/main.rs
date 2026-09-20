use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Button, Image, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const APP_ID: &str = "dev.projectluna.Files";

fn human_size(size: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = size as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", size, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn current_home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/data/users/luna/home"))
}

fn entry_icon(path: &Path) -> &'static str {
    if path.is_dir() {
        "folder"
    } else {
        match path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
        {
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" => "image-x-generic",
            "mp3" | "wav" | "flac" | "ogg" => "audio-x-generic",
            "mp4" | "mkv" | "webm" | "mov" => "video-x-generic",
            "zip" | "tar" | "gz" | "xz" | "7z" | "zst" => "package-x-generic",
            "pdf" => "application-pdf",
            "rs" | "c" | "h" | "cpp" | "cc" | "toml" | "json" | "yaml" | "yml" | "sh" => {
                "text-x-script"
            }
            _ => "text-x-generic",
        }
    }
}

fn populate(list: &ListBox, path: &Path) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let mut entries = match fs::read_dir(path) {
        Ok(entries) => entries.flatten().collect::<Vec<_>>(),
        Err(error) => {
            let row = ListBoxRow::new();
            row.set_child(Some(&Label::new(Some(&format!(
                "Cannot open {}: {error}",
                path.display()
            )))));
            list.append(&row);
            return;
        }
    };
    entries.sort_by_key(|entry| (!entry.path().is_dir(), entry.file_name()));

    for entry in entries {
        let item_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let meta = fs::metadata(&item_path).ok();
        let suffix = meta.as_ref().map_or_else(String::new, |m| {
            if m.is_dir() {
                "Folder".to_owned()
            } else {
                human_size(m.len())
            }
        });
        let row = ListBoxRow::new();
        let row_box = Box::new(Orientation::Horizontal, 12);
        let icon = Image::from_icon_name(entry_icon(&item_path));
        icon.set_pixel_size(24);
        row_box.append(&icon);
        row_box.append(&Label::new(Some(&name)));
        let details = Label::new(Some(&suffix));
        details.set_hexpand(true);
        details.set_halign(gtk4::Align::End);
        row_box.append(&details);
        row.set_child(Some(&row_box));
        row.set_activatable(true);
        let open_path = item_path.clone();
        row.connect_activate(move |_| {
            if open_path.is_dir() {
                let _ = Command::new("luna-files")
                    .arg(open_path.as_os_str())
                    .spawn();
            } else {
                let _ = Command::new("xdg-open").arg(open_path.as_os_str()).spawn();
            }
        });
        list.append(&row);
    }
}

fn build_ui(app: &Application, start: PathBuf) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Luna Files")
        .default_width(1000)
        .default_height(700)
        .icon_name(APP_ID)
        .build();
    let root = Box::new(Orientation::Vertical, 8);
    let toolbar = Box::new(Orientation::Horizontal, 6);
    let home = Button::builder()
        .icon_name("go-home")
        .tooltip_text("Home")
        .build();
    let up = Button::builder()
        .icon_name("go-up")
        .tooltip_text("Up")
        .build();
    let title = Label::new(Some(&start.display().to_string()));
    title.set_hexpand(true);
    title.set_halign(gtk4::Align::Start);
    toolbar.append(&home);
    toolbar.append(&up);
    toolbar.append(&title);
    root.append(&toolbar);

    let list = ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::Single);
    let scroll = ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .hexpand(true)
        .build();
    root.append(&scroll);
    window.set_child(Some(&root));

    let current = std::rc::Rc::new(std::cell::RefCell::new(start));
    populate(&list, current.borrow().as_path());

    {
        let current = current.clone();
        let list = list.clone();
        let title = title.clone();
        home.connect_clicked(move |_| {
            *current.borrow_mut() = current_home();
            title.set_text(&current.borrow().display().to_string());
            populate(&list, current.borrow().as_path());
        });
    }
    {
        let current = current.clone();
        let list = list.clone();
        let title = title.clone();
        up.connect_clicked(move |_| {
            let parent = current.borrow().parent().map(Path::to_path_buf);
            if let Some(parent) = parent {
                *current.borrow_mut() = parent;
                title.set_text(&current.borrow().display().to_string());
                populate(&list, current.borrow().as_path());
            }
        });
    }
    window.present();
}

fn main() {
    if env::args().any(|arg| arg == "--reveal") {
        std::process::exit(0);
    }
    let start = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(current_home);
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build_ui(app, start.clone()));
    app.run();
}
