//! Bounded local request protocol; never accepts physical paths or session IDs.
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

pub const APPLICATION_SOCKET: &str = "/run/luna/application.sock";
pub const MAX_REQUEST: usize = 32768;
pub const MAX_RESPONSE: usize = 8192;
pub const MAX_CATALOG_RESPONSE: usize = 128 * 1024;
pub const MAX_DESKTOP_APPLICATIONS: usize = 64;
pub const DESKTOP_SHARE: &str = "/run/luna/desktop-applications/share";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogRequest {
    pub format: u32,
    pub operation: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ApplicationRequest {
    OpenFile(crate::file_handoff::OpenFileRequest),
    Run(RunRequest),
    Catalog(CatalogRequest),
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DesktopApplication {
    pub application: String,
    pub version: String,
    pub content_identity: String,
    pub name: String,
    pub terminal: bool,
}

impl DesktopApplication {
    pub fn validate(&self) -> io::Result<()> {
        if self.application.is_empty()
            || self.application.len() > 255
            || !self
                .application
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || luna_app_manager::parse_version(&self.version).is_err()
            || !valid_content_identity(&self.content_identity)
            || self.name.trim().is_empty()
            || self.name.len() > 512
            || self.name.chars().any(char::is_control)
        {
            return Err(io::Error::other("invalid desktop application metadata"));
        }
        Ok(())
    }

    pub fn filename(&self) -> io::Result<String> {
        self.validate()?;
        Ok(format!(
            "luna-{}.desktop",
            luna_bundle::directory::storage_slot(&self.application, &self.version)
        ))
    }

    pub fn desktop_entry(&self) -> io::Result<String> {
        self.validate()?;
        // Name is a key-file string, never part of Exec. Every Exec token is
        // fixed or restricted to ASCII identifiers, decimal version or hex.
        let name = self.name.trim().replace('\\', "\\\\");
        let icon = if self.terminal {
            "utilities-terminal"
        } else {
            "application-x-executable"
        };
        Ok(format!(
            "[Desktop Entry]\nType=Application\nName={name}\nExec=/usr/bin/luna app run {} {} --expect-content {} --\nTryExec=/usr/bin/luna\nIcon={icon}\nTerminal=false\nDBusActivatable=false\nStartupWMClass={}\nCategories=System;\nX-Luna-ContentIdentity={}\n",
            self.application,
            self.version,
            self.content_identity,
            self.application,
            self.content_identity
        ))
    }
}

pub fn valid_content_identity(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogResponse {
    pub format: u32,
    pub applications: Vec<DesktopApplication>,
    pub error: Option<String>,
}
impl CatalogResponse {
    pub fn to_toml(&self) -> io::Result<String> {
        if self.format != 1 || self.applications.len() > MAX_DESKTOP_APPLICATIONS {
            return Err(io::Error::other("invalid application catalog"));
        }
        let mut unique = std::collections::BTreeSet::new();
        for app in &self.applications {
            app.validate()?;
            if !unique.insert((&app.application, &app.version)) {
                return Err(io::Error::other("duplicate catalog application"));
            }
        }
        let text = toml::to_string(self).map_err(io::Error::other)?;
        if text.len() > MAX_CATALOG_RESPONSE {
            return Err(io::Error::other("catalog response too large"));
        }
        Ok(text)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    pub format: u32,
    pub application: String,
    pub version: String,
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_identity: Option<String>,
}
impl RunRequest {
    pub fn validate(&self) -> io::Result<()> {
        if self.format != 1
            || self
                .content_identity
                .as_ref()
                .is_some_and(|pin| !valid_content_identity(pin))
            || self.application.is_empty()
            || self.application.len() > 255
            || self.version.len() > 32
            || self.args.len() > 64
            || self
                .args
                .iter()
                .any(|arg| arg.len() > 4096 || arg.contains('\0'))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid application request",
            ));
        }
        Ok(())
    }
}

pub fn catalog_applications() -> io::Result<CatalogResponse> {
    let request = CatalogRequest {
        format: 1,
        operation: "catalog".into(),
    };
    let mut stream = UnixStream::connect(APPLICATION_SOCKET)?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(60)))?;
    stream.write_all(
        toml::to_string(&request)
            .map_err(io::Error::other)?
            .as_bytes(),
    )?;
    stream.shutdown(Shutdown::Write)?;
    let mut text = String::new();
    stream
        .take((MAX_CATALOG_RESPONSE + 1) as u64)
        .read_to_string(&mut text)?;
    if text.len() > MAX_CATALOG_RESPONSE {
        return Err(io::Error::other("catalog response too large"));
    }
    decode_catalog(&text)
}

fn decode_catalog(text: &str) -> io::Result<CatalogResponse> {
    let response: CatalogResponse = match toml::from_str(text) {
        Ok(response) => response,
        Err(error) => {
            // The broker rejects an unauthenticated peer before parsing its
            // request, using the existing bounded common error response.
            if let Ok(response) = toml::from_str::<RunResponse>(text)
                && response.format == 1
                && let Some(error) = response.error
            {
                return Err(io::Error::other(error));
            }
            return Err(io::Error::other(error));
        }
    };
    response.to_toml()?;
    if let Some(error) = &response.error {
        return Err(io::Error::other(error.clone()));
    }
    Ok(response)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunResponse {
    pub format: u32,
    pub instance: Option<String>,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub error: Option<String>,
}
impl RunResponse {
    pub fn failed(error: impl ToString) -> Self {
        let error = error.to_string();
        Self {
            format: 1,
            instance: None,
            exit_code: None,
            signal: None,
            error: Some(error.chars().take(1500).collect()),
        }
    }
}

pub fn run_application(request: &RunRequest) -> io::Result<RunResponse> {
    request.validate()?;
    let text = toml::to_string(request).map_err(io::Error::other)?;
    if text.len() > MAX_REQUEST {
        return Err(io::Error::other("request too large"));
    }
    let mut stream = UnixStream::connect(APPLICATION_SOCKET)?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
    stream.write_all(text.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;
    // Completion may take as long as the application's lifetime. The bounded
    // response arrives on exit, launch failure, or logout termination.
    let mut response = String::new();
    stream
        .take((MAX_RESPONSE + 1) as u64)
        .read_to_string(&mut response)?;
    if response.len() > MAX_RESPONSE {
        return Err(io::Error::other("response too large"));
    }
    let response: RunResponse = toml::from_str(&response).map_err(io::Error::other)?;
    if response.format != 1 {
        return Err(io::Error::other("unsupported response"));
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn desktop() -> DesktopApplication {
        DesktopApplication {
            application: "org.luna.files".into(),
            version: "0.1.0".into(),
            content_identity: "a".repeat(64),
            name: "Luna Files".into(),
            terminal: false,
        }
    }
    #[test]
    fn legacy_run_and_catalog_are_distinct_and_bounded() {
        let legacy = "format=1\napplication='org.luna.files'\nversion='0.1.0'\nargs=[]";
        let ApplicationRequest::Run(request) = toml::from_str(legacy).unwrap() else {
            panic!()
        };
        assert!(request.content_identity.is_none());
        assert!(request.validate().is_ok());
        assert!(matches!(
            toml::from_str::<ApplicationRequest>("format=1\noperation='catalog'").unwrap(),
            ApplicationRequest::Catalog(_)
        ));
        assert!(
            toml::from_str::<ApplicationRequest>(&format!("{legacy}\noperation='catalog'"))
                .is_err()
        );
        assert!(
            toml::from_str::<ApplicationRequest>("format=1\noperation='catalog'\nuser='root'")
                .is_err()
        );
    }
    #[test]
    fn desktop_exec_is_pinned_and_name_is_not_a_command() {
        let mut app = desktop();
        app.name = "Files $(touch /tmp/evil) \\ %f".into();
        let text = app.desktop_entry().unwrap();
        let exec = text.lines().find(|line| line.starts_with("Exec=")).unwrap();
        assert_eq!(
            exec,
            format!(
                "Exec=/usr/bin/luna app run org.luna.files 0.1.0 --expect-content {} --",
                "a".repeat(64)
            )
        );
        assert!(!exec.contains("touch"));
        assert!(text.contains("Name=Files $(touch /tmp/evil)"));
        assert!(
            desktop()
                .desktop_entry()
                .unwrap()
                .contains("Name=Luna Files\n")
        );
        assert!(text.contains("Terminal=false\nDBusActivatable=false"));
        assert!(text.contains("StartupWMClass=org.luna.files"));
    }
    #[test]
    fn desktop_metadata_rejects_injection_and_unbounded_names() {
        for name in ["x\nExec=bad", "x\rfoo", "x\0foo", "x\tfoo", " "] {
            let mut app = desktop();
            app.name = name.into();
            assert!(app.desktop_entry().is_err());
        }
        let mut app = desktop();
        app.name = "x".repeat(513);
        assert!(app.validate().is_err());
        for id in ["org.bad/x", "bad %f", "bad;sh", "bad\n"] {
            let mut app = desktop();
            app.application = id.into();
            assert!(app.filename().is_err());
        }
        for version in ["01.0.0", "1.0;sh", "1.0", "1.0.0\n"] {
            let mut app = desktop();
            app.version = version.into();
            assert!(app.validate().is_err());
        }
        let mut app = desktop();
        app.application = "x".repeat(255);
        assert!(app.filename().unwrap().len() < 100);
    }
    #[test]
    fn catalog_roundtrip_limit_duplicates_and_common_rejection() {
        let response = CatalogResponse {
            format: 1,
            applications: vec![desktop()],
            error: None,
        };
        assert_eq!(
            decode_catalog(&response.to_toml().unwrap())
                .unwrap()
                .applications,
            response.applications
        );
        let duplicate = CatalogResponse {
            format: 1,
            applications: vec![desktop(), desktop()],
            error: None,
        };
        assert!(duplicate.to_toml().is_err());
        let large = CatalogResponse {
            format: 1,
            applications: vec![desktop(); 65],
            error: None,
        };
        assert!(large.to_toml().is_err());
        let text = toml::to_string(&RunResponse::failed("peer denied")).unwrap();
        assert!(
            decode_catalog(&text)
                .unwrap_err()
                .to_string()
                .contains("peer denied")
        );
        assert!(decode_catalog("format=1").is_err());
    }
    #[test]
    fn content_pin_is_exact_lowercase_hex() {
        assert!(valid_content_identity(&"a".repeat(64)));
        for pin in ["A".repeat(64), "g".repeat(64), "a".repeat(63), "a\0".into()] {
            assert!(!valid_content_identity(&pin));
        }
    }
    #[test]
    fn protocol_rejects_unbounded_and_foreign_inputs() {
        let mut request = RunRequest {
            format: 1,
            application: "org.luna.cli".into(),
            version: "0.1.0".into(),
            args: vec![],
            content_identity: None,
        };
        assert!(request.validate().is_ok());
        request.args = vec!["a".repeat(4097)];
        assert!(request.validate().is_err());
        request.args = vec!["a\0b".into()];
        assert!(request.validate().is_err());
        assert!(
            toml::from_str::<RunRequest>(
                "format=1\napplication='x'\nversion='0.1.0'\nargs=[]\nsession=7"
            )
            .is_err()
        );
    }
}
