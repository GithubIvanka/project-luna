use std::io;
use std::time::{Duration, Instant};

use crate::auth::{self, AuthenticationError};
use crate::graphics::{DrmCrtcState, DrmHandle};
use crate::keymap::{KeyboardState, UsKeyLayout};
use crate::renderer::{LoginPage, LoginUserVisual, LoginVisualState, hit_test_login_button};
use crate::{AccountDirectory, DrmDevice, InputBackend, InputDeviceKind, UserCredentials};

#[derive(Debug)]
pub enum SessionUiError {
    Io(io::Error),
    Authentication(AuthenticationError),
    Cancelled,
    NoUsers,
    NoDisplay,
    NoKeyboard,
}

impl std::fmt::Display for SessionUiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "session UI I/O failed: {error}"),
            Self::Authentication(error) => write!(f, "session UI authentication failed: {error}"),
            Self::Cancelled => write!(f, "session UI login cancelled"),
            Self::NoUsers => write!(f, "session UI found no eligible local users"),
            Self::NoDisplay => write!(f, "session UI found no connected display"),
            Self::NoKeyboard => write!(f, "session UI found no keyboard"),
        }
    }
}

impl std::error::Error for SessionUiError {}
impl From<io::Error> for SessionUiError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<AuthenticationError> for SessionUiError {
    fn from(value: AuthenticationError) -> Self {
        Self::Authentication(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Field {
    Password,
}

#[derive(Debug)]
struct LoginForm {
    selected_user: usize,
    page: LoginPage,
    password: String,
    focus: Option<Field>,
    error: bool,
}

impl LoginForm {
    fn new(initial: Option<String>, accounts: &AccountDirectory) -> Self {
        let selected_user = initial
            .as_deref()
            .and_then(|username| {
                accounts
                    .accounts()
                    .iter()
                    .position(|account| account.username == username)
            })
            .unwrap_or(0);
        let page = if accounts.accounts().len() == 1 {
            LoginPage::Password
        } else {
            LoginPage::UserSelection
        };
        Self {
            selected_user,
            page,
            password: String::new(),
            focus: (page == LoginPage::Password).then_some(Field::Password),
            error: false,
        }
    }
}

pub struct SessionUi {
    initial_username: Option<String>,
    renderer: crate::renderer::LoginRenderer,
}

struct DisplayStateGuard<'a> {
    drm: &'a DrmHandle,
    state: Option<DrmCrtcState>,
}

impl<'a> DisplayStateGuard<'a> {
    fn new(drm: &'a DrmHandle, state: DrmCrtcState) -> Self {
        Self {
            drm,
            state: Some(state),
        }
    }

    fn restore(&mut self) {
        let Some(state) = self.state.take() else {
            return;
        };
        match self.drm.restore_crtc_state(&state) {
            Ok(()) => eprintln!("luna-user-session: login UI restored previous display state"),
            Err(error) => eprintln!(
                "luna-user-session: login UI could not restore previous display state: {error}"
            ),
        }
    }
}

impl Drop for DisplayStateGuard<'_> {
    fn drop(&mut self) {
        let Some(state) = self.state.take() else {
            return;
        };
        match self.drm.restore_crtc_state(&state) {
            Ok(()) => eprintln!("luna-user-session: login UI restored previous display state"),
            Err(error) => eprintln!(
                "luna-user-session: login UI could not restore previous display state: {error}"
            ),
        }
    }
}

impl SessionUi {
    pub fn new(initial_username: Option<String>) -> Self {
        Self {
            initial_username,
            renderer: crate::renderer::LoginRenderer::load_default(),
        }
    }

    pub fn authenticate(&self) -> Result<UserCredentials, SessionUiError> {
        let accounts = AccountDirectory::discover()?;
        if accounts.accounts().is_empty() {
            eprintln!("luna-user-session: login UI found no eligible local users");
            return Err(SessionUiError::NoUsers);
        }
        let mut form = LoginForm::new(self.initial_username.clone(), &accounts);
        let user_views = accounts
            .accounts()
            .iter()
            .map(|account| LoginUserVisual {
                display_name: account.display_name(),
                username: account.login_name(),
            })
            .collect::<Vec<_>>();
        let mut keyboard = KeyboardState::default();
        let layout = UsKeyLayout;

        eprintln!("luna-user-session: login UI discovering seat");
        let seat = crate::SeatController::discover_default()?;
        let device = seat
            .graphics_devices()
            .first()
            .ok_or(SessionUiError::NoDisplay)?;
        eprintln!(
            "luna-user-session: login UI graphics device {}",
            device.node.display()
        );
        let drm = DrmDevice::new(&device.node);
        let handle = drm.open()?;
        let _master = handle.acquire_master()?;
        eprintln!("luna-user-session: login UI acquired DRM master");
        let target = handle
            .select_display_target()?
            .ok_or(SessionUiError::NoDisplay)?;
        eprintln!(
            "luna-user-session: login UI KMS target connector={} crtc={} mode={}x{}@{}Hz",
            target.connector_id,
            target.crtc_id,
            target.mode.width,
            target.mode.height,
            target.mode.refresh_hz,
        );

        let display_state = handle.capture_crtc_state(target.crtc_id)?;
        let buffer = handle.create_dumb(target.mode.width, target.mode.height, 32)?;
        let framebuffer = handle.add_framebuffer(&buffer, 24)?;
        let mut mapping = buffer.map()?;

        let pitch = mapping.pitch();
        let mut base_frame = vec![0u8; mapping.as_mut_slice().len()];
        render_login_base(
            &self.renderer,
            &mut base_frame,
            pitch,
            target.mode.width,
            target.mode.height,
            &user_views,
            &form,
            true,
            None,
        );
        mapping.as_mut_slice().copy_from_slice(&base_frame);
        self.renderer.draw_pointer(
            mapping.as_mut_slice(),
            pitch,
            target.mode.width,
            target.mode.height,
            (target.mode.width / 2) as i32,
            (target.mode.height / 2) as i32,
        );
        handle.set_crtc(&target, &framebuffer)?;
        // Declare the guard only after the framebuffer is active and mapped. Rust
        // then drops it before the framebuffer, so a cancellation/error restores
        // the previous CRTC while the temporary framebuffer still exists.
        let mut display_guard = DisplayStateGuard::new(&handle, display_state);
        match handle.mark_framebuffer_dirty(&framebuffer) {
            Ok(()) => eprintln!("luna-user-session: login UI initial frame presented"),
            Err(error) => eprintln!(
                "luna-user-session: login UI framebuffer dirty notification failed: {error}"
            ),
        }
        eprintln!("luna-user-session: login UI first frame drawn");

        let input = InputBackend::discover()?;
        let mut inputs = input
            .devices()
            .iter()
            .enumerate()
            .filter(|(_, device)| {
                matches!(
                    device.kind,
                    InputDeviceKind::Keyboard | InputDeviceKind::Pointer
                )
            })
            .map(|(index, _)| input.open(index))
            .collect::<io::Result<Vec<_>>>()?;

        if !inputs
            .iter()
            .any(|device| device.device().kind == InputDeviceKind::Keyboard)
        {
            return Err(SessionUiError::NoKeyboard);
        }
        eprintln!("luna-user-session: login UI keyboard/pointer input ready");
        for input_device in &inputs {
            input_device.set_nonblocking(true)?;
        }

        let mut pointer_x = (target.mode.width / 2) as i32;
        let mut pointer_y = (target.mode.height / 2) as i32;

        let mut cursor_visible = true;
        let mut last_content_draw = Instant::now() - Duration::from_secs(1);
        let mut content_dirty = true;
        let mut pointer_dirty = true;
        loop {
            if last_content_draw.elapsed() >= Duration::from_millis(450) {
                cursor_visible = !cursor_visible;
                content_dirty = true;
            }

            if content_dirty {
                let hovered_user = if form.page == LoginPage::UserSelection {
                    crate::renderer::hit_test_user(
                        target.mode.width,
                        target.mode.height,
                        pointer_x as u32,
                        pointer_y as u32,
                        user_views.len(),
                    )
                } else {
                    None
                };
                render_login_base(
                    &self.renderer,
                    &mut base_frame,
                    pitch,
                    target.mode.width,
                    target.mode.height,
                    &user_views,
                    &form,
                    cursor_visible,
                    hovered_user,
                );
                last_content_draw = Instant::now();
                content_dirty = false;
                pointer_dirty = true;
            }

            if pointer_dirty {
                mapping.as_mut_slice().copy_from_slice(&base_frame);
                self.renderer.draw_pointer(
                    mapping.as_mut_slice(),
                    pitch,
                    target.mode.width,
                    target.mode.height,
                    pointer_x,
                    pointer_y,
                );
                if let Err(error) = handle.mark_framebuffer_dirty(&framebuffer) {
                    eprintln!(
                        "luna-user-session: login UI framebuffer dirty notification failed: {error}"
                    );
                }
                pointer_dirty = false;
            }

            let mut pollfds = inputs
                .iter()
                .map(|input_device| libc::pollfd {
                    fd: input_device.fd(),
                    events: libc::POLLIN,
                    revents: 0,
                })
                .collect::<Vec<_>>();
            let ready = unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as _, 150) };
            if ready < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(SessionUiError::Io(error));
            }

            for index in 0..inputs.len() {
                if pollfds[index].revents & libc::POLLIN == 0 {
                    continue;
                }
                while let Some(event) = inputs[index].read_event()? {
                    match inputs[index].device().kind {
                        InputDeviceKind::Keyboard => {
                            if event.event_type != crate::InputEvent::KEY {
                                continue;
                            }
                            if event.value != 0 && event.code == 1 {
                                if form.page == LoginPage::Password && accounts.accounts().len() > 1
                                {
                                    form.page = LoginPage::UserSelection;
                                    form.focus = None;
                                    form.password.clear();
                                    form.error = false;
                                    content_dirty = true;
                                    eprintln!(
                                        "luna-user-session: login returned to user selection"
                                    );
                                    continue;
                                }
                                eprintln!("luna-user-session: login cancelled");
                                return Err(SessionUiError::Cancelled);
                            }
                            if event.value != 0 {
                                content_dirty = true;
                            }
                            if let Some(credentials) = handle_key(
                                &mut form,
                                &accounts,
                                &mut keyboard,
                                &layout,
                                event.value != 0,
                                event.code,
                            ) {
                                eprintln!("luna-user-session: login credentials accepted");
                                // The temporary login framebuffer must not be removed while
                                // it is still attached to the CRTC. Restore the pre-login
                                // state first, then let the compositor take ownership.
                                display_guard.restore();
                                return Ok(credentials);
                            }
                        }
                        InputDeviceKind::Pointer => {
                            let old_hovered_user = if form.page == LoginPage::UserSelection {
                                crate::renderer::hit_test_user(
                                    target.mode.width,
                                    target.mode.height,
                                    pointer_x as u32,
                                    pointer_y as u32,
                                    user_views.len(),
                                )
                            } else {
                                None
                            };
                            if let Some(credentials) = handle_pointer_event(
                                &mut form,
                                &accounts,
                                &mut keyboard,
                                &layout,
                                &event,
                                &mut pointer_x,
                                &mut pointer_y,
                                target.mode.width,
                                target.mode.height,
                            ) {
                                eprintln!("luna-user-session: login credentials accepted");
                                display_guard.restore();
                                return Ok(credentials);
                            }
                            pointer_dirty = true;
                            if event.event_type == crate::InputEvent::KEY
                                && event.code == crate::InputEvent::BTN_LEFT
                            {
                                content_dirty = true;
                            }
                            if form.page == LoginPage::UserSelection {
                                let new_hovered_user = crate::renderer::hit_test_user(
                                    target.mode.width,
                                    target.mode.height,
                                    pointer_x as u32,
                                    pointer_y as u32,
                                    user_views.len(),
                                );
                                if new_hovered_user != old_hovered_user {
                                    content_dirty = true;
                                }
                            }
                        }
                        InputDeviceKind::Other => {}
                    }
                }
            }
        }
    }
}

fn render_login_base(
    renderer: &crate::renderer::LoginRenderer,
    pixels: &mut [u8],
    pitch: usize,
    width: u32,
    height: u32,
    users: &[LoginUserVisual<'_>],
    form: &LoginForm,
    cursor_visible: bool,
    hovered_user: Option<usize>,
) {
    let visual = LoginVisualState {
        users,
        selected_user: form.selected_user,
        page: form.page,
        password_len: form.password.chars().count(),
        password_focused: form.focus == Some(Field::Password),
        error: form.error,
        cursor_visible,
        pointer_x: -1,
        pointer_y: -1,
        hovered_user,
    };
    renderer.draw_login_screen(pixels, pitch, width, height, &visual);
}

fn handle_pointer_event(
    form: &mut LoginForm,
    accounts: &AccountDirectory,
    keyboard: &mut KeyboardState,
    layout: &UsKeyLayout,
    event: &crate::InputEvent,
    pointer_x: &mut i32,
    pointer_y: &mut i32,
    width: u32,
    height: u32,
) -> Option<UserCredentials> {
    match (event.event_type, event.code) {
        (crate::InputEvent::REL, crate::InputEvent::REL_X) => {
            *pointer_x =
                (*pointer_x + pointer_delta(event.value)).clamp(0, width.saturating_sub(1) as i32);
        }
        (crate::InputEvent::REL, crate::InputEvent::REL_Y) => {
            *pointer_y =
                (*pointer_y + pointer_delta(event.value)).clamp(0, height.saturating_sub(1) as i32);
        }
        (crate::InputEvent::KEY, crate::InputEvent::BTN_LEFT) if event.value == 1 => {
            if form.page == LoginPage::UserSelection {
                if let Some(index) = crate::renderer::hit_test_user(
                    width,
                    height,
                    *pointer_x as u32,
                    *pointer_y as u32,
                    accounts.accounts().len(),
                ) {
                    form.selected_user = index;
                    form.page = LoginPage::Password;
                    form.focus = Some(Field::Password);
                    form.password.clear();
                    form.error = false;
                    eprintln!("luna-user-session: login pointer selected user {index}");
                }
            } else if crate::renderer::hit_test_password(
                width,
                height,
                *pointer_x as u32,
                *pointer_y as u32,
            ) {
                form.focus = Some(Field::Password);
                form.error = false;
                eprintln!("luna-user-session: login pointer focused password");
            } else if hit_test_login_button(width, height, *pointer_x as u32, *pointer_y as u32)
                && form.focus == Some(Field::Password)
            {
                return handle_key(form, accounts, keyboard, layout, true, 28);
            }
        }
        _ => {}
    }
    None
}

fn pointer_delta(raw: i32) -> i32 {
    let magnitude = raw.unsigned_abs().min(24);
    if magnitude == 0 {
        return 0;
    }
    let scaled = magnitude
        .saturating_mul(3)
        .saturating_add(magnitude.saturating_sub(3) / 2)
        .min(48) as i32;
    if raw < 0 { -scaled } else { scaled }
}

fn handle_key(
    form: &mut LoginForm,
    accounts: &AccountDirectory,
    keyboard: &mut KeyboardState,
    layout: &UsKeyLayout,
    pressed: bool,
    code: u16,
) -> Option<UserCredentials> {
    if keyboard.update_modifiers(pressed, code) {
        return None;
    }
    if !pressed {
        return None;
    }

    match form.page {
        LoginPage::UserSelection => match code {
            28 => {
                form.page = LoginPage::Password;
                form.focus = Some(Field::Password);
                form.password.clear();
                form.error = false;
            }
            15 => {
                if !accounts.accounts().is_empty() {
                    form.selected_user = (form.selected_user + 1) % accounts.accounts().len();
                }
            }
            105 => {
                form.selected_user = form.selected_user.saturating_sub(1);
            }
            106 => {
                if !accounts.accounts().is_empty() {
                    form.selected_user = (form.selected_user + 1) % accounts.accounts().len();
                }
            }
            103 | 108 => {
                let columns = crate::renderer::user_grid_columns(accounts.accounts().len());
                if code == 103 {
                    form.selected_user = form.selected_user.saturating_sub(columns);
                } else if !accounts.accounts().is_empty() {
                    form.selected_user = (form.selected_user + columns)
                        .min(accounts.accounts().len().saturating_sub(1));
                }
            }
            _ => {}
        },
        LoginPage::Password => match code {
            14 => {
                form.password.pop();
                form.error = false;
            }
            28 => {
                if form.focus == Some(Field::Password) {
                    let Some(account) = accounts.accounts().get(form.selected_user) else {
                        form.error = true;
                        return None;
                    };
                    eprintln!("luna-user-session: login submit");
                    match auth::authenticate(&account.username, &form.password) {
                        Ok(credentials) => {
                            eprintln!("luna-user-session: login authentication accepted");
                            return Some(credentials);
                        }
                        Err(_) => {
                            eprintln!("luna-user-session: login authentication rejected");
                            form.error = true;
                        }
                    }
                    form.password.clear();
                }
            }
            _ => {
                if let Some(ch) = keyboard.character(layout, code) {
                    if form.password.len() < 1024 && !ch.is_control() {
                        form.password.push(ch);
                        form.error = false;
                    }
                }
            }
        },
    }
    None
}

#[cfg(test)]
mod tests {
    use super::LoginForm;

    #[test]
    fn single_account_starts_directly_on_password() {
        let passwd = "luna:x:1000:1000:Luna User:/home/luna:/bin/ash
";
        let accounts = crate::AccountDirectory::from_passwd(passwd);
        let form = LoginForm::new(None, &accounts);
        assert_eq!(form.selected_user, 0);
        assert_eq!(form.page, crate::renderer::LoginPage::Password);
        assert_eq!(form.focus, Some(super::Field::Password));
    }

    #[test]
    fn multiple_accounts_start_on_user_selection() {
        let passwd = concat!(
            "alice:x:1000:1000:Alice:/home/alice:/bin/ash
",
            "bob:x:1001:1001:Bob:/home/bob:/bin/ash
"
        );
        let accounts = crate::AccountDirectory::from_passwd(passwd);
        let form = LoginForm::new(None, &accounts);
        assert_eq!(form.selected_user, 0);
        assert_eq!(form.page, crate::renderer::LoginPage::UserSelection);
        assert_eq!(form.focus, None);
    }
}
