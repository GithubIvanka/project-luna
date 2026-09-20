//! Minimal UserSession seat/resource model.
//!
//! The seat layer describes ownership of graphical resources. It intentionally
//! avoids implementing a separate seat daemon.

use std::io;
use std::path::PathBuf;

use crate::input::{InputBackend, InputDevice};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphicsDevice {
    pub node: PathBuf,
    pub sysfs_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeatResources {
    pub name: String,
    pub input: Vec<InputDevice>,
    pub graphics: Vec<GraphicsDevice>,
}

#[derive(Debug)]
pub struct SeatController {
    resources: SeatResources,
}

impl SeatController {
    pub fn discover_default() -> io::Result<Self> {
        let input = InputBackend::discover()?.devices().to_vec();
        let graphics = discover_graphics()?;
        Ok(Self {
            resources: SeatResources {
                name: "seat0".to_owned(),
                input,
                graphics,
            },
        })
    }

    pub fn resources(&self) -> &SeatResources {
        &self.resources
    }

    pub fn input_devices(&self) -> &[InputDevice] {
        &self.resources.input
    }

    pub fn graphics_devices(&self) -> &[GraphicsDevice] {
        &self.resources.graphics
    }
}

fn discover_graphics() -> io::Result<Vec<GraphicsDevice>> {
    let mut result = Vec::new();
    let directory = std::fs::read_dir("/dev/dri")?;
    for entry in directory {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if !file_name.starts_with("card") || file_name.contains('-') {
            continue;
        }

        let node = entry.path();
        let sysfs_path =
            std::fs::canonicalize(std::path::Path::new("/sys/class/drm").join(&file_name)).ok();

        result.push(GraphicsDevice { node, sysfs_path });
    }

    result.sort_by(|left, right| left.node.cmp(&right.node));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::SeatController;

    #[test]
    fn default_seat_name_is_stable() {
        if let Ok(seat) = SeatController::discover_default() {
            assert_eq!(seat.resources().name, "seat0");
        }
    }
}
