//! Minimal DRM/KMS resource model for UserSession.
//!
//! This module speaks the Linux DRM UAPI directly. Only the small subset needed
//! for early graphical-session ownership is defined here; the full libdrm API
//! is intentionally not required.

use std::fs::OpenOptions;
use std::io;
use std::mem::size_of;
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};

const DRM_IOCTL_BASE: u64 = b'd' as u64;
const IOC_NRBITS: u64 = 8;
const IOC_TYPEBITS: u64 = 8;
const IOC_SIZEBITS: u64 = 14;
const IOC_NRSHIFT: u64 = 0;
const IOC_TYPESHIFT: u64 = IOC_NRSHIFT + IOC_NRBITS;
const IOC_SIZESHIFT: u64 = IOC_TYPESHIFT + IOC_TYPEBITS;
const IOC_DIRSHIFT: u64 = IOC_SIZESHIFT + IOC_SIZEBITS;
const IOC_WRITE: u64 = 1;
const IOC_READ: u64 = 2;

const fn drm_iowr(number: u64, size: usize) -> libc::c_ulong {
    ((IOC_READ | IOC_WRITE) << IOC_DIRSHIFT
        | (DRM_IOCTL_BASE << IOC_TYPESHIFT)
        | (number << IOC_NRSHIFT)
        | ((size as u64) << IOC_SIZESHIFT)) as libc::c_ulong
}

const fn drm_io(number: u64) -> libc::c_ulong {
    ((DRM_IOCTL_BASE << IOC_TYPESHIFT) | (number << IOC_NRSHIFT)) as libc::c_ulong
}

const DRM_IOCTL_SET_MASTER: libc::c_ulong = drm_io(0x1e);
const DRM_IOCTL_DROP_MASTER: libc::c_ulong = drm_io(0x1f);
const DRM_IOCTL_GET_CAP: libc::c_ulong = drm_iowr(0x0c, size_of::<DrmGetCap>());
const DRM_IOCTL_MODE_GETRESOURCES: libc::c_ulong = drm_iowr(0xA0, size_of::<DrmModeCardRes>());
const DRM_IOCTL_MODE_SETCRTC: libc::c_ulong = drm_iowr(0xA2, size_of::<DrmModeCrtc>());
const DRM_IOCTL_MODE_GETENCODER: libc::c_ulong = drm_iowr(0xA6, size_of::<DrmModeGetEncoder>());
const DRM_IOCTL_MODE_GETCONNECTOR: libc::c_ulong = drm_iowr(0xA7, size_of::<DrmModeGetConnector>());
const DRM_IOCTL_MODE_ADDFB: libc::c_ulong = drm_iowr(0xAE, size_of::<DrmModeFbCmd>());
const DRM_IOCTL_MODE_CREATE_DUMB: libc::c_ulong = drm_iowr(0xB2, size_of::<DrmModeCreateDumb>());
const DRM_IOCTL_MODE_MAP_DUMB: libc::c_ulong = drm_iowr(0xB3, size_of::<DrmModeMapDumb>());
const DRM_IOCTL_MODE_DESTROY_DUMB: libc::c_ulong = drm_iowr(0xB4, size_of::<DrmModeDestroyDumb>());
const DRM_IOCTL_MODE_RMFB: libc::c_ulong = drm_iowr(0xAF, size_of::<u32>());

const DRM_CAP_DUMB_BUFFER: u64 = 0x1;
const DRM_MODE_CONNECTED: u32 = 1;
const DRM_MODE_TYPE_PREFERRED: u32 = 1 << 3;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DrmGetCap {
    capability: u64,
    value: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DrmModeCardRes {
    fb_id_ptr: u64,
    crtc_id_ptr: u64,
    connector_id_ptr: u64,
    encoder_id_ptr: u64,
    count_fbs: u32,
    count_crtcs: u32,
    count_connectors: u32,
    count_encoders: u32,
    min_width: u32,
    max_width: u32,
    min_height: u32,
    max_height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DrmModeModeInfo {
    clock: u32,
    hdisplay: u16,
    hsync_start: u16,
    hsync_end: u16,
    htotal: u16,
    hskew: u16,
    vdisplay: u16,
    vsync_start: u16,
    vsync_end: u16,
    vtotal: u16,
    vscan: u16,
    vrefresh: u32,
    flags: u32,
    type_: u32,
    name: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DrmModeGetConnector {
    encoders_ptr: u64,
    modes_ptr: u64,
    props_ptr: u64,
    prop_values_ptr: u64,
    count_modes: u32,
    count_props: u32,
    count_encoders: u32,
    encoder_id: u32,
    connector_id: u32,
    connector_type: u32,
    connector_type_id: u32,
    connection: u32,
    mm_width: u32,
    mm_height: u32,
    subpixel: u32,
    pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DrmModeGetEncoder {
    encoder_id: u32,
    encoder_type: u32,
    crtc_id: u32,
    possible_crtcs: u32,
    possible_clones: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DrmModeFbCmd {
    fb_id: u32,
    width: u32,
    height: u32,
    pitch: u32,
    bpp: u32,
    depth: u32,
    handle: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DrmModeCreateDumb {
    height: u32,
    width: u32,
    bpp: u32,
    flags: u32,
    handle: u32,
    pitch: u32,
    size: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DrmModeMapDumb {
    handle: u32,
    pad: u32,
    offset: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DrmModeDestroyDumb {
    handle: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DrmModeCrtc {
    set_connectors_ptr: u64,
    count_connectors: u32,
    crtc_id: u32,
    fb_id: u32,
    x: u32,
    y: u32,
    gamma_size: u32,
    mode_valid: u32,
    mode: DrmModeModeInfo,
}

#[derive(Debug)]
pub struct DumbBuffer<'a> {
    drm: &'a DrmHandle,
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
    pub pitch: u32,
    pub size: u64,
    handle: u32,
}

impl<'a> DumbBuffer<'a> {
    pub fn handle(&self) -> u32 {
        self.handle
    }

    pub fn map(&self) -> io::Result<MappedDumbBuffer> {
        let mut request = DrmModeMapDumb {
            handle: self.handle,
            ..Default::default()
        };
        ioctl(self.drm.fd(), DRM_IOCTL_MODE_MAP_DUMB, &mut request)?;

        let mapped = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                self.size as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                self.drm.fd(),
                request.offset as libc::off_t,
            )
        };
        if mapped == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }

        Ok(MappedDumbBuffer {
            pointer: mapped.cast(),
            length: self.size as usize,
            pitch: self.pitch as usize,
        })
    }
}

impl Drop for DumbBuffer<'_> {
    fn drop(&mut self) {
        let mut request = DrmModeDestroyDumb {
            handle: self.handle,
        };
        let _ = ioctl(self.drm.fd(), DRM_IOCTL_MODE_DESTROY_DUMB, &mut request);
    }
}

pub struct MappedDumbBuffer {
    pointer: *mut u8,
    length: usize,
    pitch: usize,
}

impl MappedDumbBuffer {
    pub fn pitch(&self) -> usize {
        self.pitch
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.pointer, self.length) }
    }

    pub fn fill_xrgb(&mut self, pixel: u32) {
        let bytes = pixel.to_ne_bytes();
        let pitch = self.pitch;
        for row in self.as_mut_slice().chunks_exact_mut(pitch) {
            for chunk in row.chunks_exact_mut(4) {
                chunk.copy_from_slice(&bytes);
            }
        }
    }
}

impl Drop for MappedDumbBuffer {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::munmap(self.pointer.cast(), self.length);
        }
    }
}

#[derive(Debug)]
pub struct DrmFramebuffer<'a> {
    drm: &'a DrmHandle,
    id: u32,
}

impl<'a> DrmFramebuffer<'a> {
    pub fn id(&self) -> u32 {
        self.id
    }
}

impl Drop for DrmFramebuffer<'_> {
    fn drop(&mut self) {
        let _ = unsafe { libc::ioctl(self.drm.fd(), DRM_IOCTL_MODE_RMFB as _, &mut self.id) };
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrmResources {
    pub framebuffer_count: usize,
    pub crtc_count: usize,
    pub connector_count: usize,
    pub encoder_count: usize,
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
    pub connector_ids: Vec<u32>,
    pub crtc_ids: Vec<u32>,
    pub encoder_ids: Vec<u32>,
    pub framebuffer_ids: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrmMode {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
    pub preferred: bool,
    mode_info: DrmModeModeInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrmConnector {
    pub id: u32,
    pub connected: bool,
    pub encoder_id: u32,
    pub encoders: Vec<u32>,
    pub modes: Vec<DrmMode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayTarget {
    pub connector_id: u32,
    pub crtc_id: u32,
    pub mode: DrmMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrmProbe {
    pub dumb_buffer: bool,
    pub resources: DrmResources,
    pub connectors: Vec<DrmConnector>,
}

impl DrmProbe {
    pub fn display_target(&self) -> Option<DisplayTarget> {
        for connector in &self.connectors {
            if !connector.connected || connector.modes.is_empty() {
                continue;
            }
            let mode = connector
                .modes
                .iter()
                .find(|mode| mode.preferred)
                .cloned()
                .or_else(|| connector.modes.first().cloned())?;
            let encoder_id = if connector.encoder_id != 0 {
                connector.encoder_id
            } else {
                *connector.encoders.first()?
            };
            let crtc_index = self
                .resources
                .encoder_ids
                .iter()
                .position(|id| *id == encoder_id);
            let _ = crtc_index;
            return Some(DisplayTarget {
                connector_id: connector.id,
                crtc_id: 0,
                mode,
            });
        }
        None
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrmDevice {
    node: PathBuf,
}

impl DrmDevice {
    pub fn new(node: impl Into<PathBuf>) -> Self {
        Self { node: node.into() }
    }

    pub fn node(&self) -> &Path {
        &self.node
    }

    pub fn open(&self) -> io::Result<DrmHandle> {
        let file = OpenOptions::new().read(true).write(true).open(&self.node)?;
        Ok(DrmHandle { file })
    }

    pub fn probe(&self) -> io::Result<DrmProbe> {
        let handle = self.open()?;
        handle.probe()
    }
}

pub struct DrmHandle {
    file: std::fs::File,
}

#[derive(Debug)]
pub struct DrmMaster<'a> {
    drm: &'a DrmHandle,
}

impl Drop for DrmMaster<'_> {
    fn drop(&mut self) {
        let _ = unsafe { libc::ioctl(self.drm.fd(), DRM_IOCTL_DROP_MASTER as _) };
    }
}

impl std::fmt::Debug for DrmHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DrmHandle")
            .field("fd", &self.file.as_raw_fd())
            .finish()
    }
}

impl DrmHandle {
    pub fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    pub fn acquire_master(&self) -> io::Result<DrmMaster<'_>> {
        if unsafe { libc::ioctl(self.fd(), DRM_IOCTL_SET_MASTER as _) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(DrmMaster { drm: self })
    }

    pub fn set_crtc(
        &self,
        target: &DisplayTarget,
        framebuffer: &DrmFramebuffer<'_>,
    ) -> io::Result<()> {
        if framebuffer.drm.fd() != self.fd() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "framebuffer belongs to another DRM device",
            ));
        }

        let connectors = [target.connector_id];
        let mut request = DrmModeCrtc {
            set_connectors_ptr: connectors.as_ptr() as u64,
            count_connectors: connectors.len() as u32,
            crtc_id: target.crtc_id,
            fb_id: framebuffer.id,
            mode_valid: 1,
            mode: target.mode.mode_info,
            ..Default::default()
        };
        ioctl(self.fd(), DRM_IOCTL_MODE_SETCRTC, &mut request)
    }

    pub fn create_dumb(&self, width: u32, height: u32, bpp: u32) -> io::Result<DumbBuffer<'_>> {
        if width == 0 || height == 0 || bpp == 0 || bpp % 8 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid dumb-buffer dimensions or pixel depth",
            ));
        }

        let mut request = DrmModeCreateDumb {
            width,
            height,
            bpp,
            ..Default::default()
        };
        ioctl(self.fd(), DRM_IOCTL_MODE_CREATE_DUMB, &mut request)?;

        Ok(DumbBuffer {
            drm: self,
            width,
            height,
            bpp,
            pitch: request.pitch,
            size: request.size,
            handle: request.handle,
        })
    }

    pub fn add_framebuffer(
        &self,
        buffer: &DumbBuffer<'_>,
        depth: u32,
    ) -> io::Result<DrmFramebuffer<'_>> {
        if buffer.drm.fd() != self.fd() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "dumb buffer belongs to another DRM device",
            ));
        }

        let mut request = DrmModeFbCmd {
            width: buffer.width,
            height: buffer.height,
            pitch: buffer.pitch,
            bpp: buffer.bpp,
            depth,
            handle: buffer.handle,
            ..Default::default()
        };
        ioctl(self.fd(), DRM_IOCTL_MODE_ADDFB, &mut request)?;

        Ok(DrmFramebuffer {
            drm: self,
            id: request.fb_id,
        })
    }

    pub fn probe(&self) -> io::Result<DrmProbe> {
        let resources = self.get_resources()?;
        let connectors = self.get_connectors(&resources)?;
        Ok(DrmProbe {
            dumb_buffer: self.get_capability(DRM_CAP_DUMB_BUFFER)? != 0,
            resources,
            connectors,
        })
    }

    pub fn display_target(&self) -> io::Result<Option<DisplayTarget>> {
        let probe = self.probe()?;
        Ok(probe.display_target())
    }

    fn get_capability(&self, capability: u64) -> io::Result<u64> {
        let mut request = DrmGetCap {
            capability,
            value: 0,
        };
        ioctl(self.fd(), DRM_IOCTL_GET_CAP, &mut request)?;
        Ok(request.value)
    }

    fn get_resources(&self) -> io::Result<DrmResources> {
        let mut request = DrmModeCardRes::default();
        ioctl(self.fd(), DRM_IOCTL_MODE_GETRESOURCES, &mut request)?;

        let mut framebuffer_ids = vec![0u32; request.count_fbs as usize];
        let mut crtc_ids = vec![0u32; request.count_crtcs as usize];
        let mut connector_ids = vec![0u32; request.count_connectors as usize];
        let mut encoder_ids = vec![0u32; request.count_encoders as usize];

        request.fb_id_ptr = framebuffer_ids.as_mut_ptr() as u64;
        request.crtc_id_ptr = crtc_ids.as_mut_ptr() as u64;
        request.connector_id_ptr = connector_ids.as_mut_ptr() as u64;
        request.encoder_id_ptr = encoder_ids.as_mut_ptr() as u64;

        ioctl(self.fd(), DRM_IOCTL_MODE_GETRESOURCES, &mut request)?;

        Ok(DrmResources {
            framebuffer_count: request.count_fbs as usize,
            crtc_count: request.count_crtcs as usize,
            connector_count: request.count_connectors as usize,
            encoder_count: request.count_encoders as usize,
            min_width: request.min_width,
            max_width: request.max_width,
            min_height: request.min_height,
            max_height: request.max_height,
            connector_ids,
            crtc_ids,
            encoder_ids,
            framebuffer_ids,
        })
    }

    fn get_connectors(&self, resources: &DrmResources) -> io::Result<Vec<DrmConnector>> {
        resources
            .connector_ids
            .iter()
            .map(|&connector_id| self.get_connector(connector_id))
            .collect()
    }

    fn get_connector(&self, connector_id: u32) -> io::Result<DrmConnector> {
        // DRM_MODE_GETCONNECTOR is a two-phase query: the first ioctl returns
        // the required array sizes, then the second fills the caller buffers.
        let mut request = DrmModeGetConnector {
            connector_id,
            ..Default::default()
        };
        ioctl(self.fd(), DRM_IOCTL_MODE_GETCONNECTOR, &mut request)?;

        let mut encoders = vec![0u32; request.count_encoders as usize];
        let mut modes = vec![DrmModeModeInfo::default(); request.count_modes as usize];
        let mut props = vec![0u32; request.count_props as usize];
        let mut prop_values = vec![0u64; request.count_props as usize];

        request.encoders_ptr = encoders.as_mut_ptr() as u64;
        request.modes_ptr = modes.as_mut_ptr() as u64;
        request.props_ptr = props.as_mut_ptr() as u64;
        request.prop_values_ptr = prop_values.as_mut_ptr() as u64;
        request.count_encoders = encoders.len() as u32;
        request.count_modes = modes.len() as u32;
        request.count_props = props.len() as u32;

        ioctl(self.fd(), DRM_IOCTL_MODE_GETCONNECTOR, &mut request)?;

        let modes = modes
            .into_iter()
            .map(|mode| DrmMode {
                name: c_string(&mode.name),
                width: mode.hdisplay as u32,
                height: mode.vdisplay as u32,
                refresh_hz: mode.vrefresh,
                preferred: mode.type_ & DRM_MODE_TYPE_PREFERRED != 0,
                mode_info: mode,
            })
            .collect();

        Ok(DrmConnector {
            id: connector_id,
            connected: request.connection == DRM_MODE_CONNECTED,
            encoder_id: request.encoder_id,
            encoders,
            modes,
        })
    }

    fn get_encoder(&self, encoder_id: u32) -> io::Result<DrmModeGetEncoder> {
        let mut encoder = DrmModeGetEncoder {
            encoder_id,
            ..Default::default()
        };
        ioctl(self.fd(), DRM_IOCTL_MODE_GETENCODER, &mut encoder)?;
        Ok(encoder)
    }

    pub fn select_display_target(&self) -> io::Result<Option<DisplayTarget>> {
        let resources = self.get_resources()?;
        let connectors = self.get_connectors(&resources)?;

        for connector in connectors {
            if !connector.connected || connector.modes.is_empty() {
                continue;
            }
            let mode = connector
                .modes
                .iter()
                .find(|mode| mode.preferred)
                .cloned()
                .or_else(|| connector.modes.first().cloned())
                .expect("checked non-empty modes");

            let encoder_id = if connector.encoder_id != 0 {
                connector.encoder_id
            } else if let Some(&id) = connector.encoders.first() {
                id
            } else {
                continue;
            };

            let encoder = self.get_encoder(encoder_id)?;
            let crtc_id = if encoder.crtc_id != 0 {
                encoder.crtc_id
            } else {
                resources
                    .crtc_ids
                    .iter()
                    .enumerate()
                    .find(|(index, _)| encoder.possible_crtcs & (1u32 << index) != 0)
                    .map(|(_, &id)| id)
                    .unwrap_or(0)
            };

            if crtc_id != 0 {
                return Ok(Some(DisplayTarget {
                    connector_id: connector.id,
                    crtc_id,
                    mode,
                }));
            }
        }

        Ok(None)
    }
}

fn c_string(value: &[u8]) -> String {
    let length = value
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(value.len());
    String::from_utf8_lossy(&value[..length]).into_owned()
}

fn ioctl<T>(fd: RawFd, request: libc::c_ulong, data: &mut T) -> io::Result<()> {
    let result = unsafe { libc::ioctl(fd, request as _, data as *mut T) };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DRM_CAP_DUMB_BUFFER, DRM_MODE_CONNECTED, DRM_MODE_TYPE_PREFERRED, DrmDevice, DrmGetCap,
        DrmModeCardRes, DrmModeGetConnector, DrmModeGetEncoder, DrmModeModeInfo, drm_iowr,
    };

    #[test]
    fn device_node_is_preserved() {
        let device = DrmDevice::new("/dev/dri/card0");
        assert_eq!(device.node(), std::path::Path::new("/dev/dri/card0"));
    }

    #[test]
    fn ioctl_layout_matches_linux_drm_contract() {
        assert_eq!(std::mem::size_of::<DrmGetCap>(), 16);
        assert_eq!(std::mem::size_of::<DrmModeCardRes>(), 64);
        assert_eq!(std::mem::size_of::<DrmModeModeInfo>(), 68);
        assert_eq!(std::mem::size_of::<DrmModeGetConnector>(), 80);
        assert_eq!(std::mem::size_of::<DrmModeGetEncoder>(), 20);
        assert_eq!(drm_iowr(0x0c, std::mem::size_of::<DrmGetCap>()), 0xc010640c);
        assert_eq!(
            drm_iowr(0xA0, std::mem::size_of::<DrmModeCardRes>()),
            0xc04064a0
        );
        assert_eq!(
            drm_iowr(0xA6, std::mem::size_of::<DrmModeGetEncoder>()),
            0xc01464a6
        );
        assert_eq!(
            drm_iowr(0xA7, std::mem::size_of::<DrmModeGetConnector>()),
            0xc05064a7
        );
        assert_eq!(DRM_CAP_DUMB_BUFFER, 0x1);
        assert_eq!(DRM_MODE_CONNECTED, 1);
        assert_eq!(DRM_MODE_TYPE_PREFERRED, 1 << 3);
    }
}
