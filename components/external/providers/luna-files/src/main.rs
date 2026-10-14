//! Интерфейс GTK работает в главном потоке, файловые запросы — в отдельном.
//! Поколение навигации защищает от запоздавших ответов после смены каталога.
//! Закрытие окна дожидается отмены записи и очистки созданных временных объектов.

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Button, Entry, Image, Label, ListBox, ListBoxRow,
    Orientation, ScrolledWindow,
};
use luna_app_runtime::file_handoff::{self, FileHandlersResponse, OpenFileRequest};
use luna_app_runtime::protocol::RunResponse;
use luna_files::{FileBackend, FileEntry, FileOperation, OperationControl, YaziBackend};
use std::{
    cell::{Cell, RefCell},
    env,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    rc::Rc,
};

const APP_ID: &str = "org.luna.files";

type Reply<T> = async_channel::Sender<io::Result<T>>;
enum Request {
    Handlers(Reply<FileHandlersResponse>),
    OpenFile(OpenFileRequest, Reply<RunResponse>),
    List(PathBuf, Reply<Vec<FileEntry>>),
    Operate(
        FileOperation,
        PathBuf,
        Option<PathBuf>,
        PathBuf,
        OperationControl,
        Reply<PathBuf>,
    ),
}

#[derive(Clone)]
struct Backend(async_channel::Sender<Request>);
impl Backend {
    fn start() -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        // Один рабочий поток последовательно обрабатывает ограниченную очередь:
        // файловые операции не блокируют GTK и не создают неограниченное число задач.
        let (sender, receiver) = async_channel::bounded(8);
        std::thread::Builder::new()
            .name("luna-files-yazi".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv_blocking() {
                    match request {
                        Request::Handlers(reply) => {
                            let _ = reply.send_blocking(file_handoff::file_handlers());
                        }
                        Request::OpenFile(request, reply) => {
                            let _ = reply.send_blocking(file_handoff::open_file(&request));
                        }
                        Request::List(path, reply) => {
                            let result = runtime.block_on(YaziBackend.read_directory(&path));
                            let _ = reply.send_blocking(result);
                        }
                        Request::Operate(operation, source, target, home, control, reply) => {
                            let result = runtime.block_on(YaziBackend.operate(
                                operation,
                                &source,
                                target.as_deref(),
                                &home,
                                &control,
                            ));
                            let _ = reply.send_blocking(result);
                        }
                    }
                }
            })?;
        Ok(Self(sender))
    }

    async fn list(&self, path: PathBuf) -> io::Result<Vec<FileEntry>> {
        let (sender, receiver) = async_channel::bounded(1);
        self.0
            .send(Request::List(path, sender))
            .await
            .map_err(io::Error::other)?;
        receiver.recv().await.map_err(io::Error::other)?
    }
    async fn handlers(&self) -> io::Result<FileHandlersResponse> {
        let (sender, receiver) = async_channel::bounded(1);
        self.0
            .send(Request::Handlers(sender))
            .await
            .map_err(io::Error::other)?;
        receiver.recv().await.map_err(io::Error::other)?
    }
    async fn open_file(&self, request: OpenFileRequest) -> io::Result<RunResponse> {
        let (sender, receiver) = async_channel::bounded(1);
        self.0
            .send(Request::OpenFile(request, sender))
            .await
            .map_err(io::Error::other)?;
        receiver.recv().await.map_err(io::Error::other)?
    }
    async fn operate(
        &self,
        operation: FileOperation,
        source: PathBuf,
        target: Option<PathBuf>,
        control: OperationControl,
    ) -> io::Result<PathBuf> {
        let (sender, receiver) = async_channel::bounded(1);
        self.0
            .send(Request::Operate(
                operation,
                source,
                target,
                current_home(),
                control,
                sender,
            ))
            .await
            .map_err(io::Error::other)?;
        receiver.recv().await.map_err(io::Error::other)?
    }
}

fn human_size(size: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = size as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{size} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn location_path(current: &Path, text: &str) -> PathBuf {
    if text == current.display().to_string() {
        current.to_owned()
    } else {
        PathBuf::from(text)
    }
}

fn next_selection(current: Option<i32>, delta: i32, count: i32) -> Option<i32> {
    (count > 0).then(|| (current.unwrap_or(-1) + delta).clamp(0, count - 1))
}

fn current_home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn entry_icon(item: &FileEntry) -> &'static str {
    if item.is_directory() {
        return "folder";
    }
    match item
        .path()
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

#[derive(Clone)]
struct Browser {
    backend: Backend,
    current: Rc<RefCell<PathBuf>>,
    generation: Rc<Cell<u64>>,
    entries: Rc<RefCell<Vec<FileEntry>>>,
    list: gtk4::glib::WeakRef<ListBox>,
    location: gtk4::glib::WeakRef<Entry>,
    status: gtk4::glib::WeakRef<Label>,
    busy: Rc<RefCell<Option<OperationControl>>>,
    clipboard: Rc<RefCell<Option<(PathBuf, bool)>>>,
    open_chooser: Rc<Cell<bool>>,
    cancel: gtk4::glib::WeakRef<Button>,
    application: gtk4::glib::WeakRef<Application>,
}

impl Browser {
    fn navigate(&self, path: PathBuf, reveal: Option<PathBuf>) {
        if self.busy.borrow().is_some() {
            self.message("Wait for the operation or cancel it first.");
            return;
        }
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        if let Some(status) = self.status.upgrade() {
            status.set_text("Loading…");
        }
        // При ошибке сохраняем последний открытый каталог; новый переход важнее старого ответа.
        let browser = self.clone();
        gtk4::glib::spawn_future_local(async move {
            let result = browser.backend.list(path.clone()).await;
            if browser.generation.get() != generation {
                return;
            }
            let (Some(list), Some(location), Some(status)) = (
                browser.list.upgrade(),
                browser.location.upgrade(),
                browser.status.upgrade(),
            ) else {
                return;
            };
            match result {
                Err(error) => {
                    status.set_text(&format!("Cannot open {}: {error}", path.display()));
                    location.set_text(&browser.current.borrow().display().to_string());
                }
                Ok(entries) => {
                    *browser.current.borrow_mut() = path.clone();
                    location.set_text(&path.display().to_string());
                    while let Some(child) = list.first_child() {
                        list.remove(&child);
                    }
                    for item in &entries {
                        let row = ListBoxRow::new();
                        let row_box = Box::new(Orientation::Horizontal, 12);
                        let icon = Image::from_icon_name(entry_icon(item));
                        icon.set_pixel_size(24);
                        row_box.append(&icon);
                        let name = item
                            .path()
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy();
                        let label = Label::new(Some(&name));
                        label.set_halign(gtk4::Align::Start);
                        row_box.append(&label);
                        let kind = if item.is_directory() {
                            "Folder".into()
                        } else {
                            human_size(item.size())
                        };
                        let detail = if item.is_symlink() {
                            format!("{kind} · Link")
                        } else {
                            kind
                        };
                        let details = Label::new(Some(&detail));
                        details.set_hexpand(true);
                        details.set_halign(gtk4::Align::End);
                        row_box.append(&details);
                        row.set_child(Some(&row_box));
                        list.append(&row);
                        if reveal.as_ref().is_some_and(|path| path == item.path()) {
                            list.select_row(Some(&row));
                        }
                    }
                    status.set_text(&format!("{} items", entries.len()));
                    *browser.entries.borrow_mut() = entries;
                    if list.selected_row().is_none() {
                        list.select_row(list.row_at_index(0).as_ref());
                    }
                    list.grab_focus();
                }
            }
        });
    }
    fn message(&self, text: &str) {
        if let Some(status) = self.status.upgrade() {
            status.set_text(text);
        }
    }
    fn selected(&self) -> Option<PathBuf> {
        let row = self.list.upgrade()?.selected_row()?;
        self.entries
            .borrow()
            .get(usize::try_from(row.index()).ok()?)
            .map(|item| item.path().to_owned())
    }
    fn open_selected(&self, window: &ApplicationWindow) {
        if self.busy.borrow().is_some() {
            self.message("Дождитесь завершения файловой операции.");
            return;
        }
        let Some(path) = self.selected() else {
            self.message("Выберите файл.");
            return;
        };
        if self
            .entries
            .borrow()
            .iter()
            .any(|entry| entry.path() == path && (entry.is_directory() || entry.is_symlink()))
        {
            self.message("Пока можно открывать только обычные файлы домашнего каталога.");
            return;
        }
        // Повторные нажатия не создают второй диалог, в том числе пока
        // рабочий поток ещё запрашивает каталог обработчиков.
        if self.open_chooser.replace(true) {
            self.message("Диалог открытия уже открыт.");
            return;
        }
        self.message("Получаю список разрешённых приложений…");
        let browser = self.clone();
        let weak = window.downgrade();
        gtk4::glib::spawn_future_local(async move {
            let response = match browser.backend.handlers().await {
                Ok(response) if !response.handlers.is_empty() => response,
                Ok(_) => {
                    browser.open_chooser.set(false);
                    browser.message(
                        "Нет установленного приложения с разрешённым режимом открытия файла.",
                    );
                    return;
                }
                Err(error) => {
                    browser.open_chooser.set(false);
                    browser.message(&format!("Не удалось получить приложения: {error}"));
                    return;
                }
            };
            let Some(window) = weak.upgrade() else {
                browser.open_chooser.set(false);
                return;
            };
            let labels = response
                .handlers
                .iter()
                .map(|handler| {
                    format!(
                        "{} ({})",
                        handler.application.name, handler.application.version
                    )
                })
                .collect::<Vec<_>>();
            let references = labels.iter().map(String::as_str).collect::<Vec<_>>();
            let choice = gtk4::DropDown::from_strings(&references);
            let dialog = gtk4::Dialog::builder()
                .title("Открыть с помощью…")
                .transient_for(&window)
                .modal(true)
                .build();
            dialog.add_css_class("luna-files");
            dialog.add_button("Отмена", gtk4::ResponseType::Cancel);
            let accept = dialog.add_button("Открыть", gtk4::ResponseType::Accept);
            dialog.set_default_response(gtk4::ResponseType::Accept);
            let label = Label::new(Some(&format!(
                "{}\nОткрытие только для чтения",
                path.display()
            )));
            label.set_wrap(true);
            dialog.content_area().append(&label);
            dialog.content_area().append(&choice);
            let choice = choice.downgrade();
            let b = browser.clone();
            // Сохраняем исходные байты выбранного пути, а не его отображаемую строку.
            dialog.connect_response(move |dialog, action| {
                b.open_chooser.set(false);
                if action == gtk4::ResponseType::Accept
                    && let Some(choice) = choice.upgrade()
                    && let Some(handler) = response.handlers.get(choice.selected() as usize)
                {
                    let request = OpenFileRequest {
                        format: 1,
                        operation: "open-file".into(),
                        application: handler.application.application.clone(),
                        version: handler.application.version.clone(),
                        content_identity: handler.application.content_identity.clone(),
                        mime_type: handler.mime_types[0].clone(),
                        path_hex: file_handoff::encode_file_path(&path),
                    };
                    let b = b.clone();
                    gtk4::glib::spawn_future_local(async move {
                        match b.backend.open_file(request).await {
                            Ok(_) => {
                                b.message("Файл открыт в отдельном приложении только для чтения.")
                            }
                            Err(error) => b.message(&format!("Не удалось открыть файл: {error}")),
                        }
                    });
                }
                finish_dialog(dialog);
            });
            dialog.present();
            // Enter подтверждает открытие; список обработчиков доступен через Tab.
            // Иначе GTK отдаёт начальный фокус выпадающему списку, который
            // перехватывает Enter вместо активации кнопки по умолчанию.
            accept.grab_focus();
        });
    }

    fn remember(&self, cut: bool) {
        if self.busy.borrow().is_some() {
            return;
        }
        if let Some(path) = self.selected() {
            self.message(&format!(
                "{} {} — choose a folder and Paste",
                if cut { "Move" } else { "Copy" },
                path.display()
            ));
            *self.clipboard.borrow_mut() = Some((path, cut));
        } else {
            self.message("Select an item first.");
        }
    }
    fn paste(&self) {
        let Some((source, cut)) = self.clipboard.borrow().clone() else {
            self.message("Copy or Cut an item first.");
            return;
        };
        let Some(name) = source.file_name() else {
            return;
        };
        let target = self.current.borrow().join(name);
        self.mutate(
            if cut {
                FileOperation::Move
            } else {
                FileOperation::Copy
            },
            source,
            Some(target),
        );
    }
    fn mutate(&self, operation: FileOperation, source: PathBuf, target: Option<PathBuf>) {
        if self.busy.borrow().is_some() {
            self.message("Another operation is still running.");
            return;
        }
        // До изменения содержимого делаем ожидающие ответы о каталоге устаревшими.
        self.generation.set(self.generation.get().wrapping_add(1));
        let control = OperationControl::default();
        *self.busy.borrow_mut() = Some(control.clone());
        if let Some(cancel) = self.cancel.upgrade() {
            cancel.set_sensitive(true);
        }
        self.message("Working…");
        let progress_browser = self.clone();
        let progress_control = control.clone();
        let timer =
            gtk4::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                if progress_browser.busy.borrow().is_none() {
                    return gtk4::glib::ControlFlow::Break;
                }
                let progress = progress_control.progress();
                progress_browser.message(&format!(
                    "Working… {} objects, {} copied",
                    progress.items,
                    human_size(progress.bytes)
                ));
                gtk4::glib::ControlFlow::Continue
            });
        let browser = self.clone();
        // Закрытие последнего окна отменяет работу. Главный цикл живёт,
        // пока запись не остановится и её временные объекты не будут очищены.
        let hold = self.application.upgrade().map(|app| app.hold());
        gtk4::glib::spawn_future_local(async move {
            let _hold = hold;
            let result = browser
                .backend
                .operate(operation, source.clone(), target, control)
                .await;
            timer.remove();
            *browser.busy.borrow_mut() = None;
            if let Some(cancel) = browser.cancel.upgrade() {
                cancel.set_sensitive(false);
            }
            match result {
                Ok(destination) => {
                    if operation == FileOperation::Move
                        && browser
                            .clipboard
                            .borrow()
                            .as_ref()
                            .is_some_and(|(path, cut)| *cut && path == &source)
                    {
                        *browser.clipboard.borrow_mut() = None;
                    }
                    let current = browser.current.borrow().clone();
                    if browser.list.upgrade().is_some() {
                        browser.navigate(current, Some(destination));
                    }
                }
                Err(error) => browser.message(&format!("Operation failed: {error}")),
            }
        });
    }
}

fn finish_dialog(dialog: &gtk4::Dialog) {
    // В GTK 4 destroy снимает внутреннюю ссылку GTK, но оставшиеся ссылки
    // могут удерживать окно. Сначала явно убираем модальное окно с экрана;
    // обработчики хранят слабые ссылки на поля, чтобы не удерживать иерархию.
    dialog.set_visible(false);
    dialog.destroy();
}

fn new_folder(window: &ApplicationWindow, browser: &Browser) {
    if browser.busy.borrow().is_some() {
        browser.message("Another operation is still running.");
        return;
    }
    let dialog = gtk4::Dialog::builder()
        .title("New Folder")
        .transient_for(window)
        .modal(true)
        .build();
    dialog.add_css_class("luna-files");
    dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
    dialog.add_button("Create", gtk4::ResponseType::Accept);
    let name = Entry::builder()
        .placeholder_text("Folder name")
        .activates_default(true)
        .build();
    dialog.set_default_response(gtk4::ResponseType::Accept);
    dialog.content_area().append(&name);
    let browser = browser.clone();
    // Диалог использует каталог на момент открытия, даже если пользователь уже перешёл в другой.
    let parent = browser.current.borrow().clone();
    let name = name.downgrade();
    dialog.connect_response(move |dialog, response| {
        if response == gtk4::ResponseType::Accept && let Some(name) = name.upgrade() {
            let browser = browser.clone();
            let parent = parent.clone();
            let name = OsString::from(name.text().as_str());
            let mut parts = Path::new(&name).components();
            if !matches!(parts.next(), Some(std::path::Component::Normal(part)) if part == name)
                || parts.next().is_some()
                || name.as_encoded_bytes().contains(&0)
            {
                browser.message("Enter one folder name, without '/' or '..'.");
            } else {
                browser.mutate(FileOperation::CreateDirectory, parent.join(name), None);
            }
        }
        finish_dialog(dialog);
    });
    dialog.present();
}

fn rename_item(window: &ApplicationWindow, browser: &Browser) {
    if browser.busy.borrow().is_some() {
        return;
    }
    let Some(source) = browser.selected() else {
        browser.message("Select an item first.");
        return;
    };
    let Some(original) = source.file_name().map(OsString::from) else {
        return;
    };
    let dialog = gtk4::Dialog::builder()
        .title("Rename")
        .transient_for(window)
        .modal(true)
        .build();
    dialog.add_css_class("luna-files");
    dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
    dialog.add_button("Rename", gtk4::ResponseType::Accept);
    dialog.set_default_response(gtk4::ResponseType::Accept);
    let name = Entry::builder()
        .activates_default(true)
        .text(original.to_string_lossy())
        .build();
    dialog.content_area().append(&name);
    let browser = browser.clone();
    let response_name = name.downgrade();
    dialog.connect_response(move |dialog, response| {
        if response == gtk4::ResponseType::Accept && let Some(response_name) = response_name.upgrade() {
            let text = response_name.text();
            // Неизменённое отображение имени не в UTF-8 сохраняет его исходные байты.
            let new = if text.as_str() == original.to_string_lossy() {
                original.clone()
            } else {
                OsString::from(text.as_str())
            };
            let mut components = Path::new(&new).components();
            if !matches!(components.next(),Some(std::path::Component::Normal(part)) if part == new)
                || components.next().is_some()
                || new.as_encoded_bytes().contains(&0)
            {
                browser.message("Enter one name, without '/' or '..'.");
            } else if new != original {
                browser.mutate(
                    FileOperation::Rename,
                    source.clone(),
                    Some(source.parent().unwrap().join(new)),
                );
            }
        }
        finish_dialog(dialog);
    });
    dialog.present();
    name.grab_focus();
    name.select_region(0, -1);
}

mod theme;

fn build_ui(app: &Application, start: PathBuf, reveal: Option<PathBuf>) {
    let css = gtk4::CssProvider::new();
    css.load_from_data(theme::CSS);
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Luna Files")
        .default_width(720)
        .default_height(520)
        .icon_name(APP_ID)
        .build();
    window.add_css_class("luna-files");
    let root = Box::new(Orientation::Vertical, 8);
    root.set_margin_top(8);
    root.set_margin_bottom(8);
    root.set_margin_start(8);
    root.set_margin_end(8);
    let toolbar = Box::new(Orientation::Horizontal, 6);
    toolbar.add_css_class("toolbar");
    let home = Button::builder()
        .label("Домой")
        .tooltip_text("Домашняя папка")
        .build();
    let up = Button::builder()
        .icon_name("go-up")
        .tooltip_text("На уровень выше")
        .build();
    let refresh = Button::builder()
        .icon_name("view-refresh")
        .tooltip_text("Обновить")
        .build();
    let folder = Button::with_label("Новая папка");
    let location = Entry::builder().hexpand(true).build();
    for button in [&home, &up, &refresh] {
        toolbar.append(button);
    }
    toolbar.append(&location);
    toolbar.append(&folder);
    root.append(&toolbar);
    let operations = Box::new(Orientation::Horizontal, 6);
    let open = Button::with_label("Открыть…");
    // Короткая панель помещается в половине ноутбучного экрана; названия доступны
    // в подсказках, а клавиатурные действия остаются теми же настоящими операциями.
    let copy = Button::builder()
        .icon_name("edit-copy")
        .tooltip_text("Копировать")
        .build();
    let cut = Button::builder()
        .icon_name("edit-cut")
        .tooltip_text("Вырезать")
        .build();
    let paste = Button::builder()
        .icon_name("edit-paste")
        .tooltip_text("Вставить")
        .build();
    let rename = Button::builder()
        .label("Имя")
        .tooltip_text("Переименовать")
        .build();
    let trash = Button::builder()
        .icon_name("user-trash")
        .tooltip_text("В корзину")
        .build();
    let show_trash = Button::builder()
        .icon_name("user-trash")
        .tooltip_text("Открыть корзину")
        .build();
    let cancel = Button::builder()
        .label("×")
        .tooltip_text("Отмена операции")
        .build();
    cancel.set_sensitive(false);
    for button in [
        &open,
        &copy,
        &cut,
        &paste,
        &rename,
        &trash,
        &show_trash,
        &cancel,
    ] {
        operations.append(button);
    }
    root.append(&operations);
    let list = ListBox::new();
    // Одиночный клик выбирает строку; двойной клик открывает объект.
    // Иначе выбор файла перед Ctrl+O уже создаёт первый диалог.
    list.set_activate_on_single_click(false);
    // Выделение следует за стрелками: копирование/вырезание используют выбранную строку.
    list.set_selection_mode(gtk4::SelectionMode::Browse);
    let content = Box::new(Orientation::Horizontal, 0);
    let sidebar = Box::new(Orientation::Vertical, 8);
    sidebar.add_css_class("sidebar");
    sidebar.set_size_request(150, -1);
    content.append(&sidebar);
    content.append(
        &ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .hexpand(true)
            .build(),
    );
    root.append(&content);
    let status = Label::new(None);
    status.add_css_class("status");
    status.set_halign(gtk4::Align::Start);
    status.set_wrap(true);
    root.append(&status);
    window.set_child(Some(&root));
    let backend = match Backend::start() {
        Ok(backend) => backend,
        Err(error) => {
            status.set_text(&format!("Cannot start file engine: {error}"));
            folder.set_sensitive(false);
            window.present();
            return;
        }
    };
    let browser = Browser {
        backend,
        current: Rc::new(RefCell::new(start.clone())),
        generation: Rc::new(Cell::new(0)),
        entries: Rc::new(RefCell::new(Vec::new())),
        list: list.downgrade(),
        location: location.downgrade(),
        status: status.downgrade(),
        busy: Rc::new(RefCell::new(None)),
        clipboard: Rc::new(RefCell::new(None)),
        open_chooser: Rc::new(Cell::new(false)),
        cancel: cancel.downgrade(),
        application: app.downgrade(),
    };
    // Переходы читают реальные каталоги HOME; отсутствующие папки не подменяются
    // демонстрационными файлами. Ошибку показывает существующий файловый движок.
    for (label, suffix) in [
        ("Домашняя папка", ""),
        ("Документы", "Documents"),
        ("Загрузки", "Downloads"),
        ("Изображения", "Pictures"),
        ("Музыка", "Music"),
    ] {
        let button = Button::with_label(label);
        let browser = browser.clone();
        let path = current_home().join(suffix);
        button.connect_clicked(move |_| browser.navigate(path.clone(), None));
        sidebar.append(&button);
    }
    let navigation_keys = gtk4::EventControllerKey::new();
    navigation_keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    {
        let b = browser.clone();
        navigation_keys.connect_key_pressed(move |_, key, _, modifiers| {
            use gtk4::gdk::{Key, ModifierType};
            if modifiers.intersects(
                ModifierType::CONTROL_MASK
                    | ModifierType::ALT_MASK
                    | ModifierType::SUPER_MASK
                    | ModifierType::SHIFT_MASK,
            ) {
                return gtk4::glib::Propagation::Proceed;
            }
            let delta = match key {
                Key::Down => 1,
                Key::Up => -1,
                _ => return gtk4::glib::Propagation::Proceed,
            };
            if let Some(list) = b.list.upgrade() {
                let count = b.entries.borrow().len() as i32;
                if let Some(index) =
                    next_selection(list.selected_row().map(|row| row.index()), delta, count)
                    && let Some(row) = list.row_at_index(index)
                {
                    list.select_row(Some(&row));
                    row.grab_focus();
                }
            }
            gtk4::glib::Propagation::Stop
        });
    }
    list.add_controller(navigation_keys);
    let action = gtk4::gio::SimpleAction::new("location", None);
    let location_weak = location.downgrade();
    action.connect_activate(move |_, _| {
        if let Some(location) = location_weak.upgrade() {
            location.grab_focus();
            location.select_region(0, -1);
        }
    });
    window.add_action(&action);
    app.set_accels_for_action("win.location", &["<Control>l"]);
    let action = gtk4::gio::SimpleAction::new("new-folder", None);
    let weak = window.downgrade();
    let action_browser = browser.clone();
    action.connect_activate(move |_, _| {
        if let Some(window) = weak.upgrade() {
            new_folder(&window, &action_browser);
        }
    });
    window.add_action(&action);
    app.set_accels_for_action("win.new-folder", &["<Control><Shift>n"]);
    for (name, accel, button) in [
        ("open-file", "<Control>o", &open),
        ("copy", "<Control>c", &copy),
        ("cut", "<Control>x", &cut),
        ("paste", "<Control>v", &paste),
        ("rename", "F2", &rename),
        ("trash", "Delete", &trash),
    ] {
        let action = gtk4::gio::SimpleAction::new(name, None);
        let b = browser.clone();
        let weak = window.downgrade();
        let action_name = name.to_owned();
        action.connect_activate(move |_, _| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            // Поля ввода сохраняют собственные действия буфера обмена и клавиши Delete.
            if gtk4::prelude::GtkWindowExt::focus(&window)
                .is_some_and(|widget| widget.is::<Entry>() || widget.is::<gtk4::Text>())
            {
                return;
            }
            match action_name.as_str() {
                "open-file" => b.open_selected(&window),
                "copy" => b.remember(false),
                "cut" => b.remember(true),
                "paste" => b.paste(),
                "rename" => rename_item(&window, &b),
                "trash" => {
                    if let Some(path) = b.selected() {
                        b.mutate(FileOperation::Delete, path, None);
                    }
                }
                _ => (),
            }
        });
        window.add_action(&action);
        app.set_accels_for_action(&format!("win.{name}", name = name), &[accel]);
        button.set_action_name(Some(&format!("win.{name}", name = name)));
    }
    let action = gtk4::gio::SimpleAction::new("cancel", None);
    {
        let b = browser.clone();
        action.connect_activate(move |_, _| {
            if let Some(control) = b.busy.borrow().as_ref() {
                control.cancel();
            }
        });
    }
    window.add_action(&action);
    app.set_accels_for_action("win.cancel", &["Escape"]);
    cancel.set_action_name(Some("win.cancel"));
    {
        let b = browser.clone();
        show_trash.connect_clicked(move |_| {
            b.navigate(current_home().join(".local/share/Trash/files"), None)
        });
    }
    {
        let b = browser.clone();
        window.connect_close_request(move |_| {
            if let Some(control) = b.busy.borrow().as_ref() {
                control.cancel();
            }
            gtk4::glib::Propagation::Proceed
        });
    }
    {
        let browser = browser.clone();
        home.connect_clicked(move |_| browser.navigate(current_home(), None));
    }
    {
        let browser = browser.clone();
        up.connect_clicked(move |_| {
            let parent = browser.current.borrow().parent().map(Path::to_path_buf);
            if let Some(parent) = parent {
                browser.navigate(parent, None);
            }
        });
    }
    {
        let browser = browser.clone();
        refresh.connect_clicked(move |_| {
            let path = browser.current.borrow().clone();
            browser.navigate(path, None);
        });
    }
    {
        let browser = browser.clone();
        location.connect_activate(move |entry| {
            let current = browser.current.borrow().clone();
            let path = location_path(&current, entry.text().as_str());
            browser.navigate(path, None);
        });
    }
    {
        let browser = browser.clone();
        let weak = window.downgrade();
        folder.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                new_folder(&window, &browser);
            }
        });
    }
    {
        let browser = browser.clone();
        let weak = window.downgrade();
        list.connect_row_activated(move |_, row| {
            let selected = usize::try_from(row.index())
                .ok()
                .and_then(|index| browser.entries.borrow().get(index).cloned());
            if let Some(item) = selected {
                if item.is_directory() {
                    browser.navigate(item.path().to_owned(), None);
                } else {
                    if let Some(list) = browser.list.upgrade() {
                        list.select_row(Some(row));
                    }
                    if let Some(window) = weak.upgrade() {
                        browser.open_selected(&window);
                    }
                }
            }
        });
    }
    browser.navigate(start, reveal);
    window.present();
}

fn main() {
    let mut args = env::args_os().skip(1);
    let first = args.next();
    let (start, reveal) = if first.as_deref() == Some(std::ffi::OsStr::new("--reveal")) {
        let Some(path) = args.next().map(PathBuf::from) else {
            eprintln!("Usage: luna-files --reveal PATH");
            std::process::exit(2);
        };
        let path = if path.is_absolute() {
            path
        } else {
            env::current_dir()
                .unwrap_or_else(|_| current_home())
                .join(path)
        };
        (
            path.parent().unwrap_or(Path::new("/")).to_owned(),
            Some(path),
        )
    } else {
        let start = first.map(PathBuf::from).unwrap_or_else(current_home);
        let start = if start.is_absolute() {
            start
        } else {
            env::current_dir()
                .unwrap_or_else(|_| current_home())
                .join(start)
        };
        (start, None)
    };
    if args.next().is_some() {
        eprintln!("Usage: luna-files [PATH | --reveal PATH]");
        std::process::exit(2);
    }
    // Без сессионной шины GDK берёт Wayland app_id из имени программы.
    // До запуска GTK задаём имя, совпадающее с зарегистрированным бандлом.
    gtk4::glib::set_prgname(Some(APP_ID));
    gtk4::glib::set_application_name("Luna Files");
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gtk4::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| build_ui(app, start.clone(), reveal.clone()));
    // Пути уже обработаны выше; GApplication не должен считать их своими параметрами.
    let status = app.run_with_args(&[APP_ID]);
    std::process::exit(status.into());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;
    #[test]
    fn unchanged_location_retains_original_bytes() {
        let current = PathBuf::from(OsString::from_vec(b"/home/\xff".to_vec()));
        assert_eq!(
            location_path(&current, &current.display().to_string()),
            current
        );
    }
    #[test]
    fn edited_location_uses_the_new_path() {
        assert_eq!(
            location_path(Path::new("/home"), "/data"),
            PathBuf::from("/data")
        );
    }
    #[test]
    fn arrow_selection_tracks_the_row_and_stays_in_bounds() {
        assert_eq!(next_selection(Some(0), 1, 3), Some(1));
        assert_eq!(next_selection(Some(1), 1, 3), Some(2));
        assert_eq!(next_selection(Some(2), 1, 3), Some(2));
        assert_eq!(next_selection(Some(0), -1, 3), Some(0));
        assert_eq!(next_selection(None, 1, 3), Some(0));
        assert_eq!(next_selection(None, 1, 0), None);
    }
}
