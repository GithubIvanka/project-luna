use luna_files::{FileOperation as Op, OperationControl, YaziBackend};
use std::{
    fs, io,
    os::unix::{
        ffi::OsStringExt,
        fs::{PermissionsExt, symlink},
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "luna-ops-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn p(&self, name: impl AsRef<Path>) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn run(future: impl Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future);
}
async fn operate(
    f: &Fixture,
    op: Op,
    src: &Path,
    dst: Option<&Path>,
    control: &OperationControl,
) -> io::Result<PathBuf> {
    YaziBackend.operate(op, src, dst, &f.0, control).await
}
#[test]
fn streaming_copy_preserves_bytes_and_reports_progress() {
    let f = Fixture::new();
    let bytes = vec![42; 700_003];
    fs::write(f.p("src"), &bytes).unwrap();
    let control = OperationControl::default();
    run(async {
        operate(&f, Op::Copy, &f.p("src"), Some(&f.p("dst")), &control)
            .await
            .unwrap();
    });
    assert_eq!(fs::read(f.p("dst")).unwrap(), bytes);
    assert_eq!(control.progress().bytes, 700_003);
    assert_eq!(control.progress().items, 1);
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 2);
}
#[test]
fn directory_copy_preserves_links_without_following_them() {
    let f = Fixture::new();
    fs::create_dir(f.p("src")).unwrap();
    fs::write(f.p("src/a"), b"file").unwrap();
    symlink("a", f.p("src/link")).unwrap();
    symlink("missing", f.p("src/broken")).unwrap();
    symlink("/etc/passwd", f.p("src/outside")).unwrap();
    run(async {
        operate(
            &f,
            Op::Copy,
            &f.p("src"),
            Some(&f.p("dst")),
            &OperationControl::default(),
        )
        .await
        .unwrap();
    });
    assert_eq!(fs::read(f.p("dst/a")).unwrap(), b"file");
    assert_eq!(
        fs::read_link(f.p("dst/broken")).unwrap(),
        Path::new("missing")
    );
    assert_eq!(
        fs::read_link(f.p("dst/outside")).unwrap(),
        Path::new("/etc/passwd")
    );
}
#[test]
fn conflicts_include_dangling_links_and_do_not_replace() {
    let f = Fixture::new();
    fs::write(f.p("src"), b"source").unwrap();
    symlink("missing", f.p("dst")).unwrap();
    run(async {
        for op in [Op::Copy, Op::Move, Op::Rename] {
            assert_eq!(
                operate(
                    &f,
                    op,
                    &f.p("src"),
                    Some(&f.p("dst")),
                    &OperationControl::default()
                )
                .await
                .unwrap_err()
                .kind(),
                io::ErrorKind::AlreadyExists
            );
        }
    });
    assert_eq!(fs::read(f.p("src")).unwrap(), b"source");
    assert_eq!(fs::read_link(f.p("dst")).unwrap(), Path::new("missing"));
}
#[test]
fn move_and_rename_never_copy_delete() {
    let f = Fixture::new();
    fs::create_dir(f.p("src")).unwrap();
    fs::write(f.p("src/file"), b"keep").unwrap();
    run(async {
        operate(
            &f,
            Op::Move,
            &f.p("src"),
            Some(&f.p("moved")),
            &OperationControl::default(),
        )
        .await
        .unwrap();
        operate(
            &f,
            Op::Rename,
            &f.p("moved"),
            Some(&f.p("renamed")),
            &OperationControl::default(),
        )
        .await
        .unwrap();
    });
    assert!(!f.p("src").exists());
    assert_eq!(fs::read(f.p("renamed/file")).unwrap(), b"keep");
}
#[test]
fn leaf_symlinks_move_as_objects() {
    let f = Fixture::new();
    symlink("missing", f.p("src")).unwrap();
    run(async {
        operate(
            &f,
            Op::Move,
            &f.p("src"),
            Some(&f.p("dst")),
            &OperationControl::default(),
        )
        .await
        .unwrap();
    });
    assert_eq!(fs::read_link(f.p("dst")).unwrap(), Path::new("missing"));
}
#[test]
fn symlink_ancestors_are_rejected_for_every_mutation() {
    let f = Fixture::new();
    fs::create_dir(f.p("real")).unwrap();
    fs::write(f.p("real/a"), b"keep").unwrap();
    symlink("real", f.p("alias")).unwrap();
    run(async {
        for op in [Op::Copy, Op::Move, Op::Rename, Op::Delete] {
            assert!(
                operate(
                    &f,
                    op,
                    &f.p("alias/a"),
                    Some(&f.p("dst")),
                    &OperationControl::default()
                )
                .await
                .is_err()
            );
        }
        assert!(
            YaziBackend
                .create_directory(&f.p("alias"), std::ffi::OsStr::new("new"))
                .await
                .is_err()
        );
        assert!(
            operate(
                &f,
                Op::Copy,
                &f.p("real/a"),
                Some(&f.p("alias/new")),
                &OperationControl::default()
            )
            .await
            .is_err()
        );
    });
    assert_eq!(fs::read(f.p("real/a")).unwrap(), b"keep");
    assert!(!f.p("real/new").exists());
}
#[test]
fn self_and_descendant_copy_fail_without_staging_debris() {
    let f = Fixture::new();
    fs::create_dir(f.p("src")).unwrap();
    run(async {
        assert!(
            operate(
                &f,
                Op::Copy,
                &f.p("src"),
                Some(&f.p("src/nested")),
                &OperationControl::default()
            )
            .await
            .is_err()
        );
    });
    assert_eq!(fs::read_dir(f.p("src")).unwrap().count(), 0);
}
#[test]
fn cancelled_before_start_never_mutates() {
    let f = Fixture::new();
    fs::write(f.p("src"), b"keep").unwrap();
    let c = OperationControl::default();
    c.cancel();
    run(async {
        for op in [Op::Copy, Op::Move, Op::Rename, Op::Delete] {
            assert_eq!(
                operate(&f, op, &f.p("src"), Some(&f.p("dst")), &c)
                    .await
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::Interrupted
            );
        }
    });
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
}
#[test]
fn mid_copy_cancel_joins_writer_and_removes_only_staging() {
    let f = Fixture::new();
    let file = fs::File::create(f.p("src")).unwrap();
    file.set_len(128 * 1024 * 1024).unwrap();
    let c = OperationControl::default();
    let thread_c = c.clone();
    let thread = std::thread::spawn(move || {
        while thread_c.progress().bytes == 0 {
            std::thread::yield_now();
        }
        thread_c.cancel();
    });
    run(async {
        assert_eq!(
            operate(&f, Op::Copy, &f.p("src"), Some(&f.p("dst")), &c)
                .await
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted
        );
    });
    thread.join().unwrap();
    assert!(!f.p("dst").exists());
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
}
#[test]
fn destination_created_during_copy_is_preserved() {
    let f = Fixture::new();
    fs::File::create(f.p("src"))
        .unwrap()
        .set_len(64 * 1024 * 1024)
        .unwrap();
    let c = OperationControl::default();
    let thread_c = c.clone();
    let dst = f.p("dst");
    let thread = std::thread::spawn(move || {
        while thread_c.progress().bytes == 0 {
            std::thread::yield_now();
        }
        fs::write(dst, b"concurrent").unwrap();
    });
    run(async {
        assert_eq!(
            operate(&f, Op::Copy, &f.p("src"), Some(&f.p("dst")), &c)
                .await
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
    });
    thread.join().unwrap();
    assert_eq!(fs::read(f.p("dst")).unwrap(), b"concurrent");
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 2);
}
#[test]
fn unsupported_socket_cleans_partial_tree_and_preserves_source() {
    let f = Fixture::new();
    fs::create_dir(f.p("src")).unwrap();
    fs::write(f.p("src/a"), b"keep").unwrap();
    let _socket = std::os::unix::net::UnixListener::bind(f.p("src/socket")).unwrap();
    run(async {
        assert_eq!(
            operate(
                &f,
                Op::Copy,
                &f.p("src"),
                Some(&f.p("dst")),
                &OperationControl::default()
            )
            .await
            .unwrap_err()
            .kind(),
            io::ErrorKind::Unsupported
        );
    });
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
    assert_eq!(fs::read(f.p("src/a")).unwrap(), b"keep");
}
#[test]
fn trash_is_recoverable_and_metadata_encodes_original_bytes() {
    let f = Fixture::new();
    let name = std::ffi::OsString::from_vec(b"original \xff=\n".to_vec());
    fs::write(f.p(&name), b"keep").unwrap();
    run(async {
        let dst = operate(
            &f,
            Op::Delete,
            &f.p(&name),
            None,
            &OperationControl::default(),
        )
        .await
        .unwrap();
        assert_eq!(fs::read(&dst).unwrap(), b"keep");
        assert!(!f.p(&name).exists());
        let info = f.p(".local/share/Trash/info").join(format!(
            "{}.trashinfo",
            dst.file_name().unwrap().to_str().unwrap()
        ));
        let text = fs::read_to_string(info).unwrap();
        assert!(text.starts_with("[Trash Info]\nPath="));
        assert!(text.contains("original%20%FF%3D%0A\nDeletionDate="));
        operate(
            &f,
            Op::Move,
            &dst,
            Some(&f.p(&name)),
            &OperationControl::default(),
        )
        .await
        .unwrap();
        assert_eq!(fs::read(f.p(&name)).unwrap(), b"keep");
    });
}
#[test]
fn trash_directory_keeps_contents_and_does_not_follow_leaf_link() {
    let f = Fixture::new();
    fs::create_dir(f.p("src")).unwrap();
    fs::write(f.p("src/a"), b"keep").unwrap();
    symlink("/etc/passwd", f.p("src/link")).unwrap();
    run(async {
        let dst = operate(
            &f,
            Op::Delete,
            &f.p("src"),
            None,
            &OperationControl::default(),
        )
        .await
        .unwrap();
        assert_eq!(fs::read(dst.join("a")).unwrap(), b"keep");
        assert_eq!(
            fs::read_link(dst.join("link")).unwrap(),
            Path::new("/etc/passwd")
        );
    });
}
#[test]
fn trash_symlink_and_writable_path_fail_without_data_loss() {
    let f = Fixture::new();
    fs::write(f.p("src"), b"keep").unwrap();
    fs::create_dir(f.p(".local")).unwrap();
    fs::set_permissions(f.p(".local"), fs::Permissions::from_mode(0o777)).unwrap();
    run(async {
        assert_eq!(
            operate(
                &f,
                Op::Delete,
                &f.p("src"),
                None,
                &OperationControl::default()
            )
            .await
            .unwrap_err()
            .kind(),
            io::ErrorKind::PermissionDenied
        );
    });
    assert_eq!(fs::read(f.p("src")).unwrap(), b"keep");
    fs::remove_dir(f.p(".local")).unwrap();
    symlink("/tmp", f.p(".local")).unwrap();
    run(async {
        assert!(
            operate(
                &f,
                Op::Delete,
                &f.p("src"),
                None,
                &OperationControl::default()
            )
            .await
            .is_err()
        );
    });
    assert_eq!(fs::read(f.p("src")).unwrap(), b"keep");
}
#[test]
fn trash_rejects_home_itself_and_recursive_trash() {
    let f = Fixture::new();
    run(async {
        assert!(
            operate(&f, Op::Delete, &f.0, None, &OperationControl::default())
                .await
                .is_err()
        );
        assert!(
            operate(
                &f,
                Op::Delete,
                &f.p(".local/share/Trash/files/a"),
                None,
                &OperationControl::default()
            )
            .await
            .is_err()
        );
    });
}
#[test]
fn invalid_paths_do_not_modify_data() {
    let f = Fixture::new();
    fs::write(f.p("src"), b"keep").unwrap();
    run(async {
        for dst in [
            Path::new("relative"),
            Path::new("/tmp/../tmp/escape"),
            Path::new("/"),
        ] {
            assert!(
                operate(
                    &f,
                    Op::Copy,
                    &f.p("src"),
                    Some(dst),
                    &OperationControl::default()
                )
                .await
                .is_err()
            );
        }
    });
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
}

#[test]
fn depth_limit_cleans_the_entire_owned_partial_tree() {
    let f = Fixture::new();
    fs::create_dir(f.p("src")).unwrap();
    let mut path = f.p("src");
    for _ in 0..130 {
        path.push("d");
        fs::create_dir(&path).unwrap();
    }
    run(async {
        assert!(
            operate(
                &f,
                Op::Copy,
                &f.p("src"),
                Some(&f.p("dst")),
                &OperationControl::default()
            )
            .await
            .is_err()
        );
    });
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
}
#[test]
fn changed_source_is_rejected_and_never_published() {
    let f = Fixture::new();
    fs::File::create(f.p("src"))
        .unwrap()
        .set_len(64 * 1024 * 1024)
        .unwrap();
    let c = OperationControl::default();
    let thread_c = c.clone();
    let src = f.p("src");
    let thread = std::thread::spawn(move || {
        while thread_c.progress().bytes == 0 {
            std::thread::yield_now();
        }
        fs::write(src, b"new source").unwrap();
    });
    run(async {
        assert!(
            operate(&f, Op::Copy, &f.p("src"), Some(&f.p("dst")), &c)
                .await
                .is_err()
        );
    });
    thread.join().unwrap();
    assert_eq!(fs::read(f.p("src")).unwrap(), b"new source");
    assert!(!f.p("dst").exists());
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
}
#[test]
fn non_utf8_copy_and_rename_preserve_path_bytes() {
    let f = Fixture::new();
    let name = std::ffi::OsString::from_vec(vec![b'x', 255]);
    let other = std::ffi::OsString::from_vec(vec![b'y', 254]);
    fs::write(f.p(&name), b"keep").unwrap();
    run(async {
        operate(
            &f,
            Op::Copy,
            &f.p(&name),
            Some(&f.p(&other)),
            &OperationControl::default(),
        )
        .await
        .unwrap();
        operate(
            &f,
            Op::Rename,
            &f.p(&other),
            Some(&f.p("renamed")),
            &OperationControl::default(),
        )
        .await
        .unwrap();
    });
    assert_eq!(fs::read(f.p("renamed")).unwrap(), b"keep");
    assert_eq!(fs::read(f.p(&name)).unwrap(), b"keep");
}
