use std::fs;

#[derive(Debug, PartialEq, Eq)]
pub struct BootstrapManifest {
    pub critical: Vec<String>,
}

pub fn parse_manifest(bytes: &[u8]) -> Result<BootstrapManifest, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| "System Image manifest is not UTF-8".to_owned())?;
    let mut in_bootstrap = false;
    let mut critical = None;

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_bootstrap = line == "[bootstrap]";
            continue;
        }
        if !in_bootstrap || !line.starts_with("critical") {
            continue;
        }
        let (_, value) = line
            .split_once('=')
            .ok_or_else(|| "invalid bootstrap.critical assignment".to_owned())?;
        critical = Some(parse_string_array(value.trim())?);
    }

    let critical = critical.ok_or_else(|| "missing [bootstrap].critical".to_owned())?;
    if critical.is_empty() {
        return Err("[bootstrap].critical must not be empty".to_owned());
    }
    for path in &critical {
        validate_path(path)?;
    }
    Ok(BootstrapManifest { critical })
}

fn parse_string_array(value: &str) -> Result<Vec<String>, String> {
    let value = value
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or_else(|| "bootstrap.critical must be an array".to_owned())?;
    let mut result = Vec::new();
    for item in value.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        if item.len() < 2 || !item.starts_with('"') || !item.ends_with('"') {
            return Err("bootstrap.critical entries must be quoted strings".to_owned());
        }
        result.push(item[1..item.len() - 1].to_owned());
    }
    Ok(result)
}

fn validate_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') || path == "/" || path.contains('\0') || path.contains("//") {
        return Err(format!("invalid bootstrap critical path: {path:?}"));
    }
    for component in path.split('/') {
        if component == ".." || component == "." {
            return Err(format!(
                "bootstrap critical path escapes logical root: {path:?}"
            ));
        }
    }
    Ok(())
}

pub fn validate_resources(root: &str, manifest: &BootstrapManifest) -> Result<(), String> {
    for path in &manifest.critical {
        let relative = path.trim_start_matches('/');
        let candidate = std::path::Path::new(root).join(relative);
        let metadata = fs::metadata(&candidate)
            .map_err(|e| format!("missing bootstrap critical resource {path}: {e}"))?;
        if !metadata.is_file() {
            return Err(format!(
                "bootstrap critical resource is not a regular file: {path}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bootstrap_critical() {
        let manifest = parse_manifest(
            br#"
[image]
format = "squashfs"

[bootstrap]
critical = ["/sbin/luna-system-runtime"]
"#,
        )
        .unwrap();
        assert_eq!(manifest.critical, vec!["/sbin/luna-system-runtime"]);
    }

    #[test]
    fn rejects_relative_or_parent_paths() {
        assert!(
            parse_manifest(
                br#"[bootstrap]
critical = ["../escape"]
"#
            )
            .is_err()
        );
        assert!(
            parse_manifest(
                br#"[bootstrap]
critical = ["usr/bin/foo"]
"#
            )
            .is_err()
        );
    }

    #[test]
    fn validates_regular_resources() {
        let dir = std::env::temp_dir().join(format!("luna-init-bootstrap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sbin")).unwrap();
        fs::write(dir.join("sbin/luna-system-runtime"), b"runtime").unwrap();
        let manifest = BootstrapManifest {
            critical: vec!["/sbin/luna-system-runtime".into()],
        };
        assert!(validate_resources(dir.to_str().unwrap(), &manifest).is_ok());
        fs::remove_dir_all(dir).unwrap();
    }
}
