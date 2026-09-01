//! Discovery of System Images, manifests and compatible kernels from SYSTEM.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::error::{BootError, BootResult};
use crate::filesystem::SystemFilesystem;
use crate::target::BootTarget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageRole { Normal, Factory, Recovery }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageManifest {
    pub name: String,
    pub version: String,
    pub format: String,
    pub arch: String,
    pub role: ImageRole,
    pub compatible_kernels: Vec<String>,
}

impl ImageManifest {
    pub fn parse(bytes: &[u8]) -> BootResult<Self> {
        let text = core::str::from_utf8(bytes).map_err(|_| BootError::InvalidConfig)?;
        let mut section = "";
        let mut name = None;
        let mut version = None;
        let mut format = None;
        let mut arch = None;
        let mut role = ImageRole::Normal;
        let mut compatible = Vec::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            if line.starts_with('[') && line.ends_with(']') { section = &line[1..line.len() - 1]; continue; }
            let Some((key, value)) = line.split_once('=') else { continue; };
            match (section, key.trim()) {
                ("image", "name") => name = parse_string(value),
                ("image", "version") => version = parse_string(value),
                ("image", "format") => format = parse_string(value),
                ("image", "role") => role = match parse_string(value).as_deref() { Some("factory") => ImageRole::Factory, Some("recovery") => ImageRole::Recovery, _ => ImageRole::Normal },
                ("architecture", "arch") => arch = parse_string(value),
                ("kernels", "compatible") => compatible = parse_string_array(value),
                _ => {}
            }
        }
        let result = Self {
            name: name.ok_or(BootError::InvalidConfig)?, version: version.ok_or(BootError::InvalidConfig)?,
            format: format.ok_or(BootError::InvalidConfig)?, arch: arch.ok_or(BootError::InvalidConfig)?,
            role, compatible_kernels: compatible,
        };
        if result.format != "squashfs" || result.arch != "x86_64" || result.compatible_kernels.is_empty() { return Err(BootError::InvalidConfig); }
        Ok(result)
    }
}

#[derive(Clone, Debug)]
pub struct KernelRecord { pub version: String, pub kernel_path: String, pub initrd_path: Option<String> }

#[derive(Clone, Debug, Default)]
pub struct BootCatalog { pub targets: Vec<BootTarget>, pub recovery: Option<BootTarget>, pub factory: Option<BootTarget>, pub default_target: usize }

impl BootCatalog {
    pub fn discover(fs: &mut SystemFilesystem) -> BootResult<Self> {
        let images = fs.read_dir("/images")?;
        let kernel_dirs = fs.read_dir("/kernels")?;
        let mut kernels = Vec::new();
        for entry in kernel_dirs.iter().filter(|entry| entry.is_dir()) {
            let base = format!("/kernels/{}/", entry.name);
            let kernel_path = find_file(fs, &[format!("{}bzImage", base), format!("{}vmlinuz", base)])?;
            let Some(kernel_path) = kernel_path else { continue; };
            let initrd_path = find_file(fs, &[format!("{}initramfs.img", base), format!("{}initrd.img", base)])?;
            kernels.push(KernelRecord { version: entry.name.clone(), kernel_path, initrd_path });
        }
        let mut targets = Vec::new(); let mut recovery = None; let mut factory = None;
        for image in images.iter().filter(|entry| entry.is_file() && entry.name.ends_with(".squashfs")) {
            let stem = &image.name[..image.name.len() - 8];
            let manifest_bytes = match fs.read_file(&format!("/images/{}.toml", stem)) { Ok(bytes) => bytes, Err(_) => continue };
            let manifest = match ImageManifest::parse(&manifest_bytes) { Ok(value) => value, Err(_) => continue };
            let Some(kernel) = select_kernel(&manifest, &kernels) else { continue; };
            let mut target = BootTarget::new(
                match manifest.role { ImageRole::Normal => format!("Luna {}", manifest.version), ImageRole::Factory => "Factory Environment".to_owned(), ImageRole::Recovery => "Recovery Environment".to_owned() },
                manifest.version.clone(), format!("/images/{}", image.name), kernel.kernel_path,
            );
            if let Some(initrd) = kernel.initrd_path { target = target.with_initrd(initrd); }
            target = target.with_cmdline(format!(
                "quiet loglevel=3 root=/dev/ram0 ro rdinit=/init luna.system_image=/images/{} luna.system_device=LABEL=LUNA-SYSTEM luna.data_device=LABEL=LUNA-DATA luna.kernel_version={} luna.boot_mode={}",
                image.name, kernel.version, match manifest.role { ImageRole::Normal => "normal", ImageRole::Factory => "factory", ImageRole::Recovery => "recovery" }
            ));
            match manifest.role { ImageRole::Normal => targets.push(target), ImageRole::Factory => factory = Some(target.factory()), ImageRole::Recovery => recovery = Some(target.recovery()) }
        }
        targets.sort_by(|a, b| version_cmp(&b.system_version, &a.system_version));
        if targets.is_empty() && factory.is_none() && recovery.is_none() { return Err(BootError::NoBootTargets); }
        Ok(Self { targets, recovery, factory, default_target: 0 })
    }
}

fn select_kernel(manifest: &ImageManifest, kernels: &[KernelRecord]) -> Option<KernelRecord> {
    kernels.iter().filter(|kernel| manifest.compatible_kernels.iter().any(|allowed| allowed == "*" || allowed == &kernel.version)).max_by(|a,b| version_cmp(&a.version, &b.version)).cloned()
}

fn find_file(fs: &mut SystemFilesystem, paths: &[String]) -> BootResult<Option<String>> {
    for path in paths { if fs.file_exists(path)? { return Ok(Some(path.clone())); } }
    Ok(None)
}

fn parse_string(value: &str) -> Option<String> { let value = value.trim(); if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') { Some(value[1..value.len()-1].to_string()) } else { None } }
fn parse_string_array(value: &str) -> Vec<String> { let value = value.trim(); if !value.starts_with('[') || !value.ends_with(']') { return Vec::new(); } value[1..value.len()-1].split(',').filter_map(parse_string).collect() }
fn version_cmp(a: &str, b: &str) -> Ordering { let mut left=a.split('.'); let mut right=b.split('.'); loop { match (left.next(), right.next()) { (None,None)=>return Ordering::Equal, (None,Some(_))=>return Ordering::Less, (Some(_),None)=>return Ordering::Greater, (Some(x),Some(y))=>match x.parse::<u64>().unwrap_or(0).cmp(&y.parse::<u64>().unwrap_or(0)) { Ordering::Equal=>{}, other=>return other } } } }
