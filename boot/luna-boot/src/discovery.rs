//! Discovery of System Images, luna-init cores and compatible kernels from LUNA-SYS.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::error::{BootError, BootResult};
use crate::filesystem::SystemFilesystem;
use crate::target::BootTarget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageRole {
    Normal,
    Factory,
    Recovery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageManifest {
    pub name: String,
    pub version: String,
    pub format: String,
    pub arch: String,
    pub role: ImageRole,
    pub compatible_inits: Vec<String>,
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
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = &line[1..line.len() - 1];
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match (section, key.trim()) {
                ("image", "name") => name = parse_string(value),
                ("image", "version") => version = parse_string(value),
                ("image", "format") => format = parse_string(value),
                ("image", "role") => {
                    role = match parse_string(value).as_deref() {
                        Some("factory") => ImageRole::Factory,
                        Some("recovery") => ImageRole::Recovery,
                        _ => ImageRole::Normal,
                    }
                }
                ("architecture", "arch") => arch = parse_string(value),
                ("init", "compatible") => compatible = parse_string_array(value),
                _ => {}
            }
        }
        let result = Self {
            name: name.ok_or(BootError::InvalidConfig)?,
            version: version.ok_or(BootError::InvalidConfig)?,
            format: format.ok_or(BootError::InvalidConfig)?,
            arch: arch.ok_or(BootError::InvalidConfig)?,
            role,
            compatible_inits: compatible,
        };
        if result.format != "squashfs"
            || result.arch != "x86_64"
            || result.compatible_inits.is_empty()
        {
            return Err(BootError::InvalidConfig);
        }
        Ok(result)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitManifest {
    pub name: String,
    pub version: String,
    pub arch: String,
    pub compatible_kernels: Vec<String>,
}

impl InitManifest {
    pub fn parse(bytes: &[u8]) -> BootResult<Self> {
        let text = core::str::from_utf8(bytes).map_err(|_| BootError::InvalidConfig)?;
        let mut section = "";
        let mut name = None;
        let mut version = None;
        let mut arch = None;
        let mut compatible = Vec::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = &line[1..line.len() - 1];
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match (section, key.trim()) {
                ("init", "name") => name = parse_string(value),
                ("init", "version") => version = parse_string(value),
                ("architecture", "arch") => arch = parse_string(value),
                ("kernels", "compatible") => compatible = parse_string_array(value),
                _ => {}
            }
        }
        let result = Self {
            name: name.ok_or(BootError::InvalidConfig)?,
            version: version.ok_or(BootError::InvalidConfig)?,
            arch: arch.ok_or(BootError::InvalidConfig)?,
            compatible_kernels: compatible,
        };
        if result.name != "luna-init"
            || result.arch != "x86_64"
            || result.compatible_kernels.is_empty()
        {
            return Err(BootError::InvalidConfig);
        }
        Ok(result)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootTargetRef {
    pub image: String,
    pub init: String,
    pub kernel: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BootStateConfig {
    pub format: u64,
    pub generation: u64,
    pub current: Option<BootTargetRef>,
    pub fallback: Option<BootTargetRef>,
    pub recovery: Option<BootTargetRef>,
    pub factory: Option<BootTargetRef>,
    pub attempt_id: u64,
    pub previous_attempt_failed: bool,
    pub fallback_depth: u8,
    pub failure_code: u32,
}

impl BootStateConfig {
    pub fn parse(bytes: &[u8]) -> BootResult<Self> {
        let text = core::str::from_utf8(bytes).map_err(|_| BootError::InvalidConfig)?;
        let mut section = "";
        let mut result = Self::default();
        let mut targets = [
            TargetFields::default(),
            TargetFields::default(),
            TargetFields::default(),
            TargetFields::default(),
        ];

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = &line[1..line.len() - 1];
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            match (section, key) {
                ("state", "format") => {
                    result.format = parse_u64(value).ok_or(BootError::InvalidConfig)?;
                }
                ("state", "generation") => {
                    result.generation = parse_u64(value).ok_or(BootError::InvalidConfig)?;
                }
                ("boot", "attempt_id") => {
                    result.attempt_id = parse_u64(value).ok_or(BootError::InvalidConfig)?;
                }
                ("boot", "previous_attempt_failed") => {
                    result.previous_attempt_failed =
                        parse_bool(value).ok_or(BootError::InvalidConfig)?;
                }
                ("boot", "fallback_depth") => {
                    result.fallback_depth = parse_u64(value)
                        .and_then(|value| u8::try_from(value).ok())
                        .ok_or(BootError::InvalidConfig)?;
                }
                ("boot", "failure_code") => {
                    result.failure_code = parse_u64(value)
                        .and_then(|value| u32::try_from(value).ok())
                        .ok_or(BootError::InvalidConfig)?;
                }
                ("targets.current", "image") => targets[0].image = parse_string(value),
                ("targets.current", "init") => targets[0].init = parse_string(value),
                ("targets.current", "kernel") => targets[0].kernel = parse_string(value),
                ("targets.fallback", "image") => targets[1].image = parse_string(value),
                ("targets.fallback", "init") => targets[1].init = parse_string(value),
                ("targets.fallback", "kernel") => targets[1].kernel = parse_string(value),
                ("targets.recovery", "image") => targets[2].image = parse_string(value),
                ("targets.recovery", "init") => targets[2].init = parse_string(value),
                ("targets.recovery", "kernel") => targets[2].kernel = parse_string(value),
                ("targets.factory", "image") => targets[3].image = parse_string(value),
                ("targets.factory", "init") => targets[3].init = parse_string(value),
                ("targets.factory", "kernel") => targets[3].kernel = parse_string(value),
                _ => {}
            }
        }

        if result.format != 1 {
            return Err(BootError::InvalidConfig);
        }

        result.current = targets[0].clone().finish()?;
        result.fallback = targets[1].clone().finish()?;
        result.recovery = targets[2].clone().finish()?;
        result.factory = targets[3].clone().finish()?;
        Ok(result)
    }
}

#[derive(Clone, Default)]
struct TargetFields {
    image: Option<String>,
    init: Option<String>,
    kernel: Option<String>,
}

impl TargetFields {
    fn finish(self) -> BootResult<Option<BootTargetRef>> {
        match (self.image, self.init, self.kernel) {
            (None, None, None) => Ok(None),
            (Some(image), Some(init), Some(kernel)) => Ok(Some(BootTargetRef {
                image,
                init,
                kernel,
            })),
            _ => Err(BootError::InvalidConfig),
        }
    }
}

#[derive(Clone, Debug)]
pub struct InitRecord {
    pub version: String,
    pub init_path: String,
    pub manifest: InitManifest,
}

#[derive(Clone, Debug)]
pub struct KernelRecord {
    pub version: String,
    pub kernel_path: String,
}

#[derive(Clone, Debug, Default)]
pub struct BootCatalog {
    pub targets: Vec<BootTarget>,
    pub recovery: Option<BootTarget>,
    pub factory: Option<BootTarget>,
    pub default_target: usize,
    pub boot_state: BootStateConfig,
}

impl BootCatalog {
    pub fn discover(fs: &mut SystemFilesystem) -> BootResult<Self> {
        let images = fs.read_dir("/images")?;
        let recovery_dir = fs.read_dir("/recovery").unwrap_or_default();
        let cores = fs.read_dir("/cores")?;
        let kernel_dirs = fs.read_dir("/kernels")?;
        let boot_state = match fs.read_file("/config/boot-state.toml") {
            Ok(bytes) => BootStateConfig::parse(&bytes).unwrap_or_default(),
            Err(_) => BootStateConfig::default(),
        };

        let mut kernels = Vec::new();
        for entry in kernel_dirs.iter().filter(|entry| entry.is_dir()) {
            let base = format!("/kernels/{}/", entry.name);
            let kernel_path = find_file(
                fs,
                &[format!("{}bzImage", base), format!("{}vmlinuz", base)],
            )?;
            let Some(kernel_path) = kernel_path else {
                continue;
            };
            kernels.push(KernelRecord {
                version: entry.name.clone(),
                kernel_path,
            });
        }

        let mut inits = Vec::new();
        for entry in cores
            .iter()
            .filter(|entry| entry.is_file() && entry.name.ends_with(".init"))
        {
            let Some(stem) = entry.name.strip_suffix(".init") else {
                continue;
            };
            let init_path = format!("/cores/{}", entry.name);
            let manifest_path = format!("/cores/{}.toml", stem);
            let manifest_bytes = match fs.read_file(&manifest_path) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let manifest = match InitManifest::parse(&manifest_bytes) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if manifest.version != stem.strip_prefix("luna-").unwrap_or(stem) {
                continue;
            }
            inits.push(InitRecord {
                version: manifest.version.clone(),
                init_path,
                manifest,
            });
        }
        inits.sort_by(|a, b| version_cmp(&b.version, &a.version));

        let mut targets = Vec::new();
        let mut factory_candidates = Vec::new();

        for image in images
            .iter()
            .filter(|entry| entry.is_file() && entry.name.ends_with(".squashfs"))
        {
            let Some(stem) = image.name.strip_suffix(".squashfs") else {
                continue;
            };
            let manifest_path = format!("/images/{}.toml", stem);
            let manifest_bytes = match fs.read_file(&manifest_path) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let manifest = match ImageManifest::parse(&manifest_bytes) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if manifest.version != stem.strip_prefix("luna-").unwrap_or(stem) {
                continue;
            }

            let Some(init) = select_init(&manifest, &inits) else {
                continue;
            };
            let Some(kernel) = select_kernel(&init.manifest, &kernels) else {
                continue;
            };
            let mut target = BootTarget::new(
                match manifest.role {
                    ImageRole::Normal => format!("Luna {}", manifest.version),
                    ImageRole::Factory => String::from("Factory Environment"),
                    ImageRole::Recovery => String::from("Recovery Environment"),
                },
                manifest.name.clone(),
                manifest.version.clone(),
                format!("/images/{}", image.name),
                manifest_path,
                init.init_path.clone(),
                kernel.kernel_path,
                kernel.version.clone(),
            );
            target = target.with_cmdline("console=tty0 console=ttyS0,115200n8 loglevel=7 ignore_loglevel initcall_debug");
            match manifest.role {
                ImageRole::Normal => targets.push(target),
                ImageRole::Factory => factory_candidates.push(target.factory()),
                ImageRole::Recovery => {}
            }
        }

        let mut recovery_candidates = Vec::new();
        for image in recovery_dir
            .iter()
            .filter(|entry| entry.is_file() && entry.name.ends_with(".squashfs"))
        {
            let Some(stem) = image.name.strip_suffix(".squashfs") else {
                continue;
            };
            let Some(version) = stem.strip_prefix("recovery-") else {
                continue;
            };
            let manifest_path = format!("/recovery/{}.toml", stem);
            let manifest_bytes = match fs.read_file(&manifest_path) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let manifest = match ImageManifest::parse(&manifest_bytes) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if manifest.version != version
                || manifest.format != "squashfs"
                || manifest.arch != "x86_64"
            {
                continue;
            }

            let Some(init) = select_init(&manifest, &inits) else {
                continue;
            };
            let Some(kernel) = select_kernel(&init.manifest, &kernels) else {
                continue;
            };
            let target = BootTarget::new(
                String::from("Recovery Environment"),
                manifest.name.clone(),
                manifest.version.clone(),
                format!("/recovery/{}", image.name),
                manifest_path,
                init.init_path.clone(),
                kernel.kernel_path,
                kernel.version.clone(),
            )
            .with_cmdline("quiet loglevel=3")
            .recovery();
            recovery_candidates.push(target);
        }

        targets.sort_by(|a, b| version_cmp(&b.system_version, &a.system_version));
        recovery_candidates.sort_by(|a, b| version_cmp(&b.system_version, &a.system_version));
        factory_candidates.sort_by(|a, b| version_cmp(&b.system_version, &a.system_version));

        let recovery = boot_state
            .recovery
            .as_ref()
            .and_then(|reference| {
                recovery_candidates
                    .iter()
                    .find(|target| target_matches(target, Some(reference)))
                    .cloned()
            })
            .or_else(|| recovery_candidates.first().cloned());
        let factory = boot_state
            .factory
            .as_ref()
            .and_then(|reference| {
                factory_candidates
                    .iter()
                    .find(|target| target_matches(target, Some(reference)))
                    .cloned()
            })
            .or_else(|| factory_candidates.first().cloned());

        let default_target = boot_state
            .current
            .as_ref()
            .and_then(|reference| {
                targets
                    .iter()
                    .position(|target| target_matches(target, Some(reference)))
            })
            .unwrap_or(0);

        if targets.is_empty() && factory.is_none() && recovery.is_none() {
            return Err(BootError::NoBootTargets);
        }
        Ok(Self {
            targets,
            recovery,
            factory,
            default_target,
            boot_state,
        })
    }

    pub fn target_for_ref(&self, reference: &BootTargetRef) -> Option<BootTarget> {
        if let Some(target) = self
            .targets
            .iter()
            .find(|target| target_matches(target, Some(reference)))
        {
            return Some(target.clone());
        }
        if self
            .recovery
            .as_ref()
            .is_some_and(|target| target_matches(target, Some(reference)))
        {
            return self.recovery.clone();
        }
        if self
            .factory
            .as_ref()
            .is_some_and(|target| target_matches(target, Some(reference)))
        {
            return self.factory.clone();
        }
        None
    }
}

fn target_matches(target: &BootTarget, reference: Option<&BootTargetRef>) -> bool {
    let Some(reference) = reference else {
        return false;
    };
    target.system_version == reference.image
        && target.init_path == format!("/cores/luna-{}.init", reference.init)
        && target.kernel_id == reference.kernel
}

fn select_init(manifest: &ImageManifest, inits: &[InitRecord]) -> Option<InitRecord> {
    inits
        .iter()
        .filter(|init| {
            manifest
                .compatible_inits
                .iter()
                .any(|allowed| allowed == "*" || allowed == &init.version)
        })
        .max_by(|a, b| version_cmp(&a.version, &b.version))
        .cloned()
}

fn select_kernel(manifest: &InitManifest, kernels: &[KernelRecord]) -> Option<KernelRecord> {
    kernels
        .iter()
        .filter(|kernel| {
            manifest
                .compatible_kernels
                .iter()
                .any(|allowed| allowed == "*" || allowed == &kernel.version)
        })
        .max_by(|a, b| version_cmp(&a.version, &b.version))
        .cloned()
}

fn find_file(fs: &mut SystemFilesystem, paths: &[String]) -> BootResult<Option<String>> {
    for path in paths {
        if fs.file_exists(path)? {
            return Ok(Some(path.clone()));
        }
    }
    Ok(None)
}

fn parse_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Some(value[1..value.len() - 1].to_string())
    } else {
        None
    }
}

fn parse_string_array(value: &str) -> Vec<String> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Vec::new();
    }
    value[1..value.len() - 1]
        .split(',')
        .filter_map(parse_string)
        .collect()
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_u64(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}

fn version_cmp(a: &str, b: &str) -> Ordering {
    let mut left = a.split('.');
    let mut right = b.split('.');
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => match x
                .parse::<u64>()
                .unwrap_or(0)
                .cmp(&y.parse::<u64>().unwrap_or(0))
            {
                Ordering::Equal => {}
                other => return other,
            },
        }
    }
}
