#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use luna_app_runtime::{
    ApplicationLaunchContext, ApplicationPlan, ApplicationPlanLauncher, CleanupOutcome,
    ExecutableSpec, InstanceState, LinuxApplicationRuntime, authorize_application_plan,
};
use luna_bundle::{BundleKind, BundleManifest, BundleMetadata, BundleResource, ResourceAccess};
use luna_common::{BundleId, RuntimeSpec, UserId, Version};
use luna_namespace::LinuxMountNamespace;
use luna_root_mapping::{LogicalPath, MappingRule, MappingTable, PhysicalPath};
use luna_security::{AuthorizationRequest, Decision, PolicyAuthority, SecurityError};
use luna_system_runtime::SystemRuntimeService;
use luna_user_session::{SessionId, SessionState, UserSession};

struct AllowAll;
impl PolicyAuthority for AllowAll {
    fn authorize(&self, _: &AuthorizationRequest) -> Result<Decision, SecurityError> {
        Ok(Decision::Allow)
    }
}

struct Fixture {
    root: PathBuf,
    mounts: Vec<PathBuf>,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(format!("/tmp/luna-p0-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        Self {
            root,
            mounts: Vec::new(),
        }
    }

    fn bind_runtime(&mut self, logical: &str) {
        let source = fs::canonicalize(logical).unwrap();
        let target = self
            .root
            .join("system")
            .join(logical.trim_start_matches('/'));
        fs::create_dir_all(&target).unwrap();
        let status = Command::new("mount")
            .arg("--bind")
            .arg(source)
            .arg(&target)
            .status()
            .unwrap();
        assert!(status.success(), "bind mount for {logical} failed");
        self.mounts.push(target);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for mount in self.mounts.iter().rev() {
            let _ = Command::new("umount").arg("-l").arg(mount).status();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn resource(manifest: &mut BundleManifest, path: &str, access: &[ResourceAccess]) {
    manifest.add_resource(
        BundleResource::new(path, path.trim_start_matches('/')).with_access(access.iter().copied()),
    );
}

fn rule(
    table: &mut MappingTable,
    logical: &str,
    physical: &Path,
    subtree: bool,
    access: &[ResourceAccess],
) {
    let logical = LogicalPath::new(logical).unwrap();
    let physical = PhysicalPath::new(physical);
    let rule = if subtree {
        MappingRule::subtree(logical, physical)
    } else {
        MappingRule::file(logical, physical)
    };
    table
        .insert(rule.with_access(access.iter().copied()))
        .unwrap();
}

#[test]
#[ignore = "requires root mount namespace privileges and Landlock ABI v3"]
fn canonical_authorized_launch_is_kernel_enforced() {
    assert_eq!(unsafe { libc::geteuid() }, 0, "run with root privileges");
    let mut fixture = Fixture::new();
    fixture.bind_runtime("/usr");
    fixture.bind_runtime("/lib");
    fixture.bind_runtime("/lib64");

    let system = fixture.root.join("system");
    fs::write(system.join("system-secret"), "must-not-be-visible").unwrap();
    let app = fixture.root.join("app");
    let tree = app.join("tree");
    fs::create_dir_all(&tree).unwrap();
    fs::write(tree.join("value"), "tree-ok").unwrap();
    fs::write(app.join("allowed-file"), "file-ok").unwrap();
    fs::write(app.join("read-only"), "read-only").unwrap();
    fs::write(app.join("write-only"), "write-only").unwrap();
    fs::write(app.join("output"), "").unwrap();

    let source = fixture.root.join("probe.c");
    fs::write(
        &source,
        r#"
#include <errno.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#define TMPFS_MAGIC 0x01021994
static void die(int code) { _exit(code); }
int main(int argc, char **argv) {
  struct statfs fs; struct stat ns; char buf[16] = {0}; int fd; pid_t child; int status;
  if (argc != 2 || statfs("/", &fs) || (unsigned long)fs.f_type != TMPFS_MAGIC) die(10);
  if (stat("/proc/self/ns/mnt", &ns) || (unsigned long long)ns.st_ino == strtoull(argv[1], 0, 10)) die(11);
  fd=open("/usr", O_RDONLY|O_DIRECTORY); if(fd<0) die(12); close(fd);
  fd=open("/lib", O_RDONLY|O_DIRECTORY); if(fd<0) die(13); close(fd);
  fd=open("/lib64", O_RDONLY|O_DIRECTORY); if(fd<0) die(14); close(fd);
  if (open("/usr/luna-write-denied", O_WRONLY|O_CREAT, 0600) >= 0) die(15);
  fd=open("/app/allowed-file", O_RDONLY); if(fd<0 || read(fd,buf,7)!=7 || memcmp(buf,"file-ok",7)) die(16); close(fd);
  memset(buf,0,sizeof(buf)); fd=open("/app/tree/value", O_RDONLY); if(fd<0 || read(fd,buf,7)!=7 || memcmp(buf,"tree-ok",7)) die(17); close(fd);
  errno=0; if(open("/app/write-only", O_RDONLY)>=0 || errno!=EACCES) die(18);
  errno=0; if(open("/app/read-only", O_WRONLY)>=0 || (errno!=EACCES && errno!=EROFS)) die(19);
  if(access("/system-secret", F_OK)==0) die(20);
  child=fork(); if(child<0) die(21);
  if(child==0){ execl("/app/no-exec","no-exec",argv[1],(char*)0); _exit(errno==EACCES?77:78); }
  if(waitpid(child,&status,0)<0 || !WIFEXITED(status) || WEXITSTATUS(status)!=77) die(22);
  fd=open("/app/output", O_WRONLY|O_TRUNC); if(fd<0 || write(fd,"PASS\n",5)!=5) die(23); close(fd);
  return 0;
}
"#,
    )
    .unwrap();
    let probe = app.join("probe");
    let status = Command::new("cc")
        .args(["-O2", "-o"])
        .arg(&probe)
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());
    fs::copy(&probe, app.join("no-exec")).unwrap();

    let mut session = UserSession::new(SessionId::new(700), UserId::from("p0"));
    session.transition(SessionState::Authenticating).unwrap();
    session.login_succeeded().unwrap();
    let mut manifest = BundleManifest::new(BundleMetadata::new(
        BundleId::from("p0.probe"),
        Version::new(1, 0, 0),
        BundleKind::Application,
    ));
    resource(
        &mut manifest,
        "/app/probe",
        &[ResourceAccess::Read, ResourceAccess::Execute],
    );
    resource(&mut manifest, "/app/no-exec", &[ResourceAccess::Read]);
    resource(&mut manifest, "/app/allowed-file", &[ResourceAccess::Read]);
    resource(&mut manifest, "/app/tree", &[ResourceAccess::Read]);
    resource(&mut manifest, "/app/read-only", &[ResourceAccess::Read]);
    resource(&mut manifest, "/app/write-only", &[ResourceAccess::Write]);
    resource(&mut manifest, "/app/output", &[ResourceAccess::Write]);

    let mut mappings = MappingTable::new();
    rule(
        &mut mappings,
        "/app/probe",
        &probe,
        false,
        &[ResourceAccess::Read, ResourceAccess::Execute],
    );
    rule(
        &mut mappings,
        "/app/no-exec",
        &app.join("no-exec"),
        false,
        &[ResourceAccess::Read],
    );
    rule(
        &mut mappings,
        "/app/allowed-file",
        &app.join("allowed-file"),
        false,
        &[ResourceAccess::Read],
    );
    rule(
        &mut mappings,
        "/app/tree",
        &tree,
        true,
        &[ResourceAccess::Read],
    );
    rule(
        &mut mappings,
        "/app/read-only",
        &app.join("read-only"),
        false,
        &[ResourceAccess::Read],
    );
    rule(
        &mut mappings,
        "/app/write-only",
        &app.join("write-only"),
        false,
        &[ResourceAccess::Write],
    );
    rule(
        &mut mappings,
        "/app/output",
        &app.join("output"),
        false,
        &[ResourceAccess::Write],
    );

    let host_ns = fs::metadata("/proc/self/ns/mnt").unwrap().ino().to_string();
    let plan = ApplicationPlan::new(
        manifest,
        mappings,
        &session,
        RuntimeSpec::luna(),
        ExecutableSpec::new("/app/probe").with_args([host_ns]),
        vec![],
    )
    .unwrap();
    let authorized = authorize_application_plan(plan, &AllowAll).unwrap();
    let context =
        ApplicationLaunchContext::new(LinuxMountNamespace, &system, fixture.root.join("staging"))
            .with_trusted_source_root(&app);
    let mut system_runtime = SystemRuntimeService::new();
    system_runtime.start();
    let mut application_runtime = LinuxApplicationRuntime::new();
    let id = application_runtime
        .launch_authorized_plan(authorized, &session, &mut system_runtime, &context)
        .unwrap();
    loop {
        match application_runtime.poll(id, &mut system_runtime).unwrap() {
            InstanceState::Running => std::thread::sleep(Duration::from_millis(10)),
            InstanceState::Stopped => break,
            state => panic!(
                "probe terminated in {state:?}: {:?}",
                application_runtime.instance(id).unwrap().exit()
            ),
        }
    }
    assert_eq!(fs::read_to_string(app.join("output")).unwrap(), "PASS\n");
    let instance = application_runtime.instance(id).unwrap();
    assert!(matches!(
        instance.cleanup(),
        Some(CleanupOutcome::Succeeded)
    ));
    assert!(!fixture.root.join("staging/instance-1").exists());
}
