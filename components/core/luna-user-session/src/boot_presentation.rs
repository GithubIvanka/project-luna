//! Загрузочная презентация после ядра, внутри графической границы UserSession.
//! Поток владеет DRM только до появления входа; остановка обязательно ждёт
//! освобождения framebuffer и DRM master. Этапы сообщает системный runtime.

use crate::{DrmDevice, SeatController, renderer::LoginRenderer, rings::Rings};
use std::io;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

pub struct BootPresentation {
    stop: Arc<AtomicBool>,
    stage: Arc<Mutex<&'static str>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl BootPresentation {
    pub fn start() -> io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let stage = Arc::new(Mutex::new("Подготовка системы"));
        let (sender, receiver) = mpsc::channel();
        let stopped = stop.clone();
        let current_stage = stage.clone();
        let worker = std::thread::Builder::new()
            .name("luna-boot-presentation".into())
            .spawn(move || {
                if let Err(error) = run(&stopped, &current_stage, &sender) {
                    let _ = sender.send(Err(error.to_string()));
                    eprintln!("luna-user-session: boot presentation unavailable: {error}");
                }
            })?;
        let presentation = Self {
            stop,
            stage,
            worker: Some(worker),
        };
        receiver
            .recv()
            .map_err(io::Error::other)?
            .map_err(io::Error::other)?;
        Ok(presentation)
    }

    pub fn set_stage(&self, stage: &'static str) {
        if let Ok(mut current) = self.stage.lock() {
            *current = stage;
        }
    }
}

impl Drop for BootPresentation {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(
    stop: &AtomicBool,
    stage: &Mutex<&'static str>,
    ready: &mpsc::Sender<Result<(), String>>,
) -> io::Result<()> {
    let seat = SeatController::discover_default()?;
    let device = seat
        .graphics_devices()
        .first()
        .ok_or_else(|| io::Error::other("no graphics device"))?;
    let handle = DrmDevice::new(&device.node).open()?;
    let _master = handle.acquire_master()?;
    let target = handle
        .select_display_target()?
        .ok_or_else(|| io::Error::other("no display target"))?;
    let old = handle.capture_crtc_state(target.crtc_id)?;
    let buffer = handle.create_dumb(target.mode.width, target.mode.height, 32)?;
    let framebuffer = handle.add_framebuffer(&buffer, 24)?;
    let mut mapping = buffer.map()?;
    let pitch = mapping.pitch();
    let (width, height) = (target.mode.width, target.mode.height);
    let renderer = LoginRenderer::load_default();
    let mut rings = Rings::load_default()?;
    let mut background = vec![0; mapping.as_mut_slice().len()];
    renderer.draw_background(&mut background, pitch, width, height);
    let mut frame = background.clone();
    let start = Instant::now();
    // Пониженное движение — системный параметр, не искусственная пауза загрузки.
    let reduced = std::env::var_os("LUNA_REDUCED_MOTION").is_some();
    let mut active = false;
    while !stop.load(Ordering::Acquire) {
        let frame_start = Instant::now();
        frame.copy_from_slice(&background);
        rings.draw(
            &mut frame,
            pitch,
            width,
            height,
            start.elapsed().as_secs_f64(),
            reduced,
        );
        let label = stage.lock().map(|s| *s).unwrap_or("Подготовка системы");
        renderer.draw_boot_label(&mut frame, pitch, width, height, label);
        mapping.as_mut_slice().copy_from_slice(&frame);
        if !active {
            handle.set_crtc(&target, &framebuffer)?;
            active = true;
            eprintln!(
                "luna-user-session: animated boot presentation ready (48 independent rune cycles)"
            );
            let _ = ready.send(Ok(()));
        }
        let _ = handle.mark_framebuffer_dirty(&framebuffer);
        std::thread::sleep(Duration::from_millis(40).saturating_sub(frame_start.elapsed()));
    }
    // Восстанавливаем CRTC до уничтожения временного framebuffer.
    let result = handle.restore_crtc_state(&old);
    eprintln!("luna-user-session: boot presentation released DRM");
    result
}
