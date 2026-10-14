//! Serialized DATA backend. Publication is an atomic directory rename;
//! luna-update-manager owns the durable transaction journal. User data is never
//! visited by installation, rollback or removal.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use luna_bundle::directory::{
    DirectoryBundle, application_filename, materialize_archive, storage_slot,
};
use luna_bundle::lbp::{LbpArchive, LbpManifest};
use luna_common::{BundleId, Version};
use luna_security::{
    AuthorizationRequest, AuthorizationSource, BundleTrustScope, Permission, PolicyAuthority,
    Principal, Resource, authorize,
};
use luna_state::{RedbStateStore, StateKey, StateStore};
use luna_update_manager::{
    UpdateBackend, UpdateEngine, UpdateError, UpdateId, UpdateOperation, UpdatePlan,
};
use serde::{Deserialize, Serialize};

use crate::{
    AppManagerError, ApplicationManager, ApplicationOperation, ApplicationPlan, ApplicationRef,
};

const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundleRepresentation {
    Transport,
    Directory,
}

#[derive(Clone, Debug)]
pub struct BundleInspection {
    pub application: ApplicationRef,
    pub content_identity: [u8; 32],
    pub manifest: LbpManifest,
    pub has_signature: bool,
    pub representation: BundleRepresentation,
}

#[derive(Clone, Debug)]
pub struct InstalledBundle {
    pub inspection: BundleInspection,
    /// Internal backend path; applications receive logical resources instead.
    pub payload_root: PathBuf,
    /// Verified public directory Bundle, not the private registration container.
    pub directory_root: PathBuf,
}

pub fn content_identity_hex(identity: &[u8; 32]) -> String {
    identity.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn parse_version(value: &str) -> Result<Version, AppManagerError> {
    let parts = value
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    if parts.len() != 3 {
        return Err(error("version must be MAJOR.MINOR.PATCH"));
    }
    let version = Version::new(parts[0], parts[1], parts[2]);
    if version.to_string() != value {
        return Err(error("version must use canonical decimal numbers"));
    }
    Ok(version)
}

pub fn inspect_bundle(path: impl AsRef<Path>) -> Result<BundleInspection, AppManagerError> {
    let path = path.as_ref();
    if fs::symlink_metadata(path).map_err(error)?.is_dir() {
        let bundle = DirectoryBundle::read(path).map_err(error)?;
        return directory_inspection(bundle);
    }
    Ok(Candidate::read(path)?.inspection)
}

fn directory_inspection(bundle: DirectoryBundle) -> Result<BundleInspection, AppManagerError> {
    validate_id(&bundle.manifest.bundle.id)?;
    Ok(BundleInspection {
        application: ApplicationRef::new(
            BundleId::from(bundle.manifest.bundle.id.clone()),
            parse_version(&bundle.manifest.bundle.version)?,
        ),
        content_identity: bundle.content_identity,
        manifest: bundle.manifest,
        has_signature: false,
        representation: BundleRepresentation::Directory,
    })
}

struct Candidate {
    archive: LbpArchive,
    inspection: BundleInspection,
}

impl Candidate {
    fn read(path: &Path) -> Result<Self, AppManagerError> {
        let mut bytes = Vec::new();
        open_regular(path)?
            .take(MAX_ARCHIVE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(error)?;
        if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(error("Bundle exceeds the installation size limit"));
        }
        let archive = LbpArchive::from_bytes(bytes.clone()).map_err(error)?;
        validate_id(&archive.manifest.bundle.id)?;
        let inspection = BundleInspection {
            application: ApplicationRef::new(
                BundleId::from(archive.manifest.bundle.id.clone()),
                parse_version(&archive.manifest.bundle.version)?,
            ),
            content_identity: archive.content_identity().map_err(error)?,
            manifest: archive.manifest.clone(),
            has_signature: archive.signature_bytes().map_err(error)?.is_some(),
            representation: BundleRepresentation::Transport,
        };
        Ok(Self {
            archive,
            inspection,
        })
    }
}

#[derive(Clone)]
struct ManagementAuthorization {
    application: ApplicationRef,
    content_identity: Option<[u8; 32]>,
}

impl AuthorizationSource for ManagementAuthorization {
    fn authorization_requests(&self) -> Vec<AuthorizationRequest> {
        let mut requests = vec![AuthorizationRequest {
            principal: Principal::System,
            resource: Resource::Application(self.application.id().clone()),
            permission: Permission::Manage,
        }];
        if let Some(content_identity) = self.content_identity {
            requests.push(AuthorizationRequest {
                principal: Principal::System,
                resource: Resource::BundleTrust {
                    application: self.application.id().clone(),
                    content_identity,
                    scope: BundleTrustScope::Install,
                },
                permission: Permission::Use,
            });
        }
        requests
    }
}

pub struct FilesystemApplicationManager {
    root: PathBuf,
    system_version: Version,
    _lock: File,
}

impl FilesystemApplicationManager {
    /// Caller provides an already selected DATA root. This never discovers,
    /// mounts or substitutes a partition. Managed directories must be owned by
    /// this operator and not writable by group/other users.
    pub fn new(
        data_root: impl AsRef<Path>,
        system_version: Version,
    ) -> Result<Self, AppManagerError> {
        let data_root = data_root.as_ref();
        if !data_root.is_absolute()
            || data_root == Path::new("/")
            || data_root
                .to_string_lossy()
                .split('/')
                .any(|p| matches!(p, "." | ".."))
        {
            return Err(error("DATA root must be an explicit absolute directory"));
        }
        if fs::canonicalize(data_root).map_err(error)? != data_root {
            return Err(error("DATA root must not contain symlinks"));
        }
        checked_directory(data_root)?;
        let system = ensure_directory(&data_root.join("system"))?;
        let apps = ensure_directory(&system.join("apps"))?;
        // Never silently reinterpret or mutate the prior experimental layout.
        // Explicit journalled migration is a separate compatibility stage.
        let legacy = apps.join(".bundles");
        if fs::symlink_metadata(&legacy).is_ok() {
            checked_directory(&legacy)?;
            for entry in fs::read_dir(&legacy).map_err(error)? {
                let name = entry.map_err(error)?.file_name();
                if name != ".lock" && name != ".transactions" && name != ".garbage" {
                    return Err(error(
                        "legacy .bundles registry preserved; directory-v1 migration is required; use a separate offline DATA fixture",
                    ));
                }
                if name != ".lock" {
                    checked_directory(&legacy.join(&name))?;
                    if fs::read_dir(legacy.join(name))
                        .map_err(error)?
                        .next()
                        .is_some()
                    {
                        return Err(error(
                            "legacy application recovery is pending; registry preserved",
                        ));
                    }
                }
            }
        }
        let root = ensure_directory(&apps.join(".bundles-v2"))?;
        ensure_directory(&root.join(".transactions"))?;
        ensure_directory(&root.join(".garbage"))?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root.join(".lock"))
            .map_err(error)?;
        checked_file(&lock)?;
        // SAFETY: flock only uses a live owned descriptor; no pointers or memory mutation.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(error(
                "another application management operation holds the DATA lock",
            ));
        }
        Ok(Self {
            root,
            system_version,
            _lock: lock,
        })
    }

    /// Offline adapter only. A live system-runtime owns this database and must
    /// instead pass its StateStore to the same domain API through its IPC path.
    pub fn open_offline_state(&self) -> Result<RedbStateStore, AppManagerError> {
        let data_root = self
            .root
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or_else(|| error("invalid DATA root"))?;
        let state = ensure_directory(&data_root.join("system/state"))?;
        let path = state.join("luna-state.redb");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(error)?;
        checked_file(&file)?;
        drop(file);
        RedbStateStore::open(path).map_err(error)
    }

    pub fn list(&self) -> Result<Vec<InstalledBundle>, AppManagerError> {
        self.list_bounded(usize::MAX)
    }

    /// Bound registry enumeration before reading/hashing another installation.
    /// The usual receipt, layout and content checks still apply to every entry.
    pub fn list_bounded(&self, maximum: usize) -> Result<Vec<InstalledBundle>, AppManagerError> {
        self.require_reconciled()?;
        let mut bundles = Vec::new();
        for application in fs::read_dir(&self.root).map_err(error)? {
            let application = application.map_err(error)?;
            let slot = application
                .file_name()
                .into_string()
                .map_err(|_| error("invalid Bundle directory"))?;
            if matches!(slot.as_str(), ".lock" | ".transactions" | ".garbage") {
                continue;
            }
            if bundles.len() >= maximum {
                return Err(error("installed application inventory exceeds its limit"));
            }
            validate_slot(&slot)?;
            checked_directory(&application.path())?;
            let receipt: Receipt =
                toml::from_str(&read_text(&application.path().join("receipt.toml"))?)
                    .map_err(error)?;
            let reference =
                ApplicationRef::new(BundleId::from(receipt.id), parse_version(&receipt.version)?);
            if slot != storage_slot(reference.id().as_str(), &reference.version().to_string()) {
                return Err(error("registration slot does not match manifest identity"));
            }
            bundles.push(self.read_installed(&reference)?);
        }
        bundles.sort_by(|a, b| {
            (
                a.inspection.application.id().as_str(),
                a.inspection.application.version(),
            )
                .cmp(&(
                    b.inspection.application.id().as_str(),
                    b.inspection.application.version(),
                ))
        });
        Ok(bundles)
    }

    pub fn installed(
        &self,
        application: &ApplicationRef,
    ) -> Result<InstalledBundle, AppManagerError> {
        self.require_reconciled()?;
        self.read_installed(application)
    }

    pub fn install<S: StateStore>(
        &self,
        path: impl AsRef<Path>,
        policy: &dyn PolicyAuthority,
        store: &mut S,
    ) -> Result<InstalledBundle, AppManagerError> {
        self.require_reconciled()?;
        let candidate = Candidate::read(path.as_ref())?;
        self.install_candidate(candidate, policy, store)
    }

    fn install_candidate<S: StateStore>(
        &self,
        candidate: Candidate,
        policy: &dyn PolicyAuthority,
        store: &mut S,
    ) -> Result<InstalledBundle, AppManagerError> {
        // Never treat a present but unverified signature as either unsigned or trusted.
        if candidate.inspection.has_signature {
            return Err(error(
                "signed installation requires the pending signature verification backend",
            ));
        }
        if candidate
            .inspection
            .manifest
            .platform
            .min_system
            .as_deref()
            .map(parse_version)
            .transpose()?
            .is_some_and(|minimum| minimum > self.system_version)
        {
            return Err(error("Bundle requires a newer Luna system"));
        }
        if !candidate.inspection.manifest.dependencies.is_empty() {
            return Err(error(
                "dependency resolution is not implemented; no dependencies were downloaded",
            ));
        }
        authorize(
            policy,
            ManagementAuthorization {
                application: candidate.inspection.application.clone(),
                content_identity: Some(candidate.inspection.content_identity),
            },
        )
        .map_err(error)?;
        let application = &candidate.inspection.application;
        let target = self.root.join(storage_slot(
            application.id().as_str(),
            &application.version().to_string(),
        ));
        if fs::symlink_metadata(&target).is_ok() {
            return Err(error("this immutable Bundle version already exists"));
        }
        let mut transaction = self.transaction(application, "install")?;
        let staged = transaction.directory.join("staged");
        let preparation = (|| {
            ensure_directory(&staged)?;
            let directory_name = application_filename(&candidate.inspection.manifest);
            let package = ensure_directory(&staged.join(&directory_name))?;
            materialize_archive(&candidate.archive, &package).map_err(error)?;
            let receipt = Receipt {
                format: 2,
                id: application.id().as_str().into(),
                version: application.version().to_string(),
                content_identity: content_identity_hex(&candidate.inspection.content_identity),
                transaction: transaction.id.as_str().into(),
                directory_name,
            };
            write_new(
                &staged.join("receipt.toml"),
                toml::to_string(&receipt).map_err(error)?.as_bytes(),
            )?;
            freeze_tree(&staged)?;
            let verified = DirectoryBundle::read(&package).map_err(error)?;
            if verified.content_identity != candidate.inspection.content_identity {
                return Err(error(
                    "staged directory identity differs from trusted source",
                ));
            }
            sync_directory(&transaction.directory)?;
            Ok::<_, AppManagerError>(())
        })();
        if let Err(preparation_error) = preparation {
            transaction.cleanup().map_err(|cleanup| {
                error(format!(
                    "preparation failed: {preparation_error}; cleanup failed: {cleanup}"
                ))
            })?;
            return Err(preparation_error);
        }
        self.execute(&mut transaction, store)?;
        self.read_installed(application)
    }

    /// Install the new version independently; retain the complete old version.
    /// Default-version selection belongs to the subsequent session integration.
    pub fn update<S: StateStore>(
        &self,
        from: &ApplicationRef,
        path: impl AsRef<Path>,
        policy: &dyn PolicyAuthority,
        store: &mut S,
    ) -> Result<InstalledBundle, AppManagerError> {
        self.verify(from)?;
        let next = Candidate::read(path.as_ref())?;
        if next.inspection.application.id() != from.id()
            || next.inspection.application.version() == from.version()
        {
            return Err(error(
                "update requires the same Bundle ID and a different version",
            ));
        }
        self.install_candidate(next, policy, store)
    }

    pub fn remove<S: StateStore>(
        &self,
        application: &ApplicationRef,
        policy: &dyn PolicyAuthority,
        store: &mut S,
    ) -> Result<(), AppManagerError> {
        self.require_reconciled()?;
        authorize(
            policy,
            ManagementAuthorization {
                application: application.clone(),
                content_identity: None,
            },
        )
        .map_err(error)?;
        self.read_installed(application)?;
        let mut transaction = self.transaction(application, "remove")?;
        self.execute(&mut transaction, store)
    }

    pub fn recover<S: StateStore>(
        &self,
        policy: &dyn PolicyAuthority,
        store: &mut S,
    ) -> Result<usize, AppManagerError> {
        authorize(policy, RecoveryAuthorization).map_err(error)?;
        let mut count = 0;
        // Only completed/discarded checkpoints enter .garbage by atomic rename.
        // They can be cleaned after an interruption without needing metadata
        // that a previous cleanup may already have removed.
        for directory in fs::read_dir(self.root.join(".garbage")).map_err(error)? {
            let directory = directory.map_err(error)?.path();
            checked_directory(&directory)?;
            thaw_and_remove(&directory)?;
            count += 1;
        }
        sync_directory(&self.root.join(".garbage"))?;
        for directory in fs::read_dir(self.root.join(".transactions")).map_err(error)? {
            let directory = directory.map_err(error)?.path();
            checked_directory(&directory)?;
            let checkpoint: Checkpoint =
                toml::from_str(&read_text(&directory.join("operation.toml"))?).map_err(error)?;
            checkpoint.validate()?;
            if directory.file_name().and_then(|s| s.to_str())
                != Some(checkpoint.transaction.as_str())
            {
                return Err(error("transaction directory does not match checkpoint"));
            }
            let application = ApplicationRef::new(
                BundleId::from(checkpoint.id.clone()),
                parse_version(&checkpoint.version)?,
            );
            authorize(
                policy,
                ManagementAuthorization {
                    application: application.clone(),
                    content_identity: None,
                },
            )
            .map_err(error)?;
            let mut transaction = FilesystemTransaction {
                root: self.root.clone(),
                directory,
                application,
                operation: checkpoint.operation,
                id: UpdateId::new(checkpoint.transaction).map_err(error)?,
            };
            let phase = journal_phase(store, &transaction.id)?;
            match phase.as_deref() {
                Some("committed") => transaction.verify(&transaction.plan()).map_err(error)?,
                Some("rolled-back") => transaction
                    .rollback(&transaction.operation())
                    .map_err(error)?,
                Some("failed") => {
                    return Err(error(
                        "transaction rollback failed; checkpoint retained for explicit repair",
                    ));
                }
                None => transaction
                    .rollback(&transaction.operation())
                    .map_err(error)?,
                Some(_) => {
                    let plan = transaction.plan();
                    let id = transaction.id.clone();
                    UpdateEngine::new(store, &mut transaction)
                        .reconcile_interrupted(&id, &plan)
                        .map_err(error)?;
                }
            }
            transaction.cleanup()?;
            count += 1;
        }
        Ok(count)
    }

    pub fn pending_applications(&self) -> Result<Vec<ApplicationRef>, AppManagerError> {
        let mut applications = Vec::new();
        for directory in fs::read_dir(self.root.join(".transactions")).map_err(error)? {
            let directory = directory.map_err(error)?.path();
            checked_directory(&directory)?;
            let checkpoint: Checkpoint =
                toml::from_str(&read_text(&directory.join("operation.toml"))?).map_err(error)?;
            checkpoint.validate()?;
            applications.push(ApplicationRef::new(
                BundleId::from(checkpoint.id),
                parse_version(&checkpoint.version)?,
            ));
        }
        Ok(applications)
    }

    fn require_reconciled(&self) -> Result<(), AppManagerError> {
        if fs::read_dir(self.root.join(".transactions"))
            .map_err(error)?
            .next()
            .is_some()
        {
            return Err(error(
                "pending application transaction; run app recover before accessing the registry",
            ));
        }
        Ok(())
    }

    fn read_installed(
        &self,
        application: &ApplicationRef,
    ) -> Result<InstalledBundle, AppManagerError> {
        Self::read_installed_at(&self.root, application)
    }

    fn read_installed_at(
        root: &Path,
        application: &ApplicationRef,
    ) -> Result<InstalledBundle, AppManagerError> {
        validate_id(application.id().as_str())?;
        let directory = root.join(storage_slot(
            application.id().as_str(),
            &application.version().to_string(),
        ));
        checked_directory(&directory)?;
        if fs::symlink_metadata(&directory).map_err(error)?.mode() & 0o7777 != 0o555 {
            return Err(error("changed installed Bundle directory mode"));
        }
        let file = open_regular(&directory.join("receipt.toml"))?;
        checked_file(&file)?;
        if file.metadata().map_err(error)?.mode() & 0o7777 != 0o444 {
            return Err(error("changed installed metadata mode"));
        }
        let receipt: Receipt =
            toml::from_str(&read_text(&directory.join("receipt.toml"))?).map_err(error)?;
        validate_directory_name(&receipt.directory_name)?;
        let directory_root = directory.join(&receipt.directory_name);
        let bundle = DirectoryBundle::read(&directory_root).map_err(error)?;
        let inspection = directory_inspection(bundle.clone())?;
        if receipt.format != 2
            || inspection.application != *application
            || receipt.id != application.id().as_str()
            || receipt.version != application.version().to_string()
            || receipt.content_identity != content_identity_hex(&inspection.content_identity)
            || receipt.directory_name != application_filename(&inspection.manifest)
        {
            return Err(error(
                "installed Bundle receipt does not match its content identity",
            ));
        }
        let payload_root = bundle.payload_root;
        checked_directory(&directory_root)?;
        let mut names = fs::read_dir(&directory)
            .map_err(error)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        names.sort();
        let mut expected = [
            std::ffi::OsString::from("receipt.toml"),
            std::ffi::OsString::from(&receipt.directory_name),
        ];
        expected.sort();
        if names != expected {
            return Err(error("unexpected installed Bundle entries"));
        }
        Ok(InstalledBundle {
            inspection,
            payload_root,
            directory_root,
        })
    }

    fn transaction(
        &self,
        application: &ApplicationRef,
        operation: &str,
    ) -> Result<FilesystemTransaction, AppManagerError> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(error)?
            .as_nanos();
        let id = UpdateId::new(format!("app-{}-{nonce}", std::process::id())).map_err(error)?;
        // Pre-journal files are disposable and must never appear as a pending
        // transaction until operation.toml is complete and durable.
        let unpublished = self.root.join(".garbage").join(id.as_str());
        let directory = self.root.join(".transactions").join(id.as_str());
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&unpublished)
            .map_err(error)?;
        let checkpoint = Checkpoint {
            format: 1,
            transaction: id.as_str().into(),
            id: application.id().as_str().into(),
            version: application.version().to_string(),
            operation: operation.into(),
        };
        if let Err(failure) = write_new(
            &unpublished.join("operation.toml"),
            toml::to_string(&checkpoint).map_err(error)?.as_bytes(),
        )
        .and_then(|()| sync_directory(&unpublished))
        .and_then(|()| sync_directory(&self.root.join(".garbage")))
        {
            let _ = fs::remove_file(unpublished.join("operation.toml"));
            let _ = fs::remove_dir(&unpublished);
            return Err(failure);
        }
        fs::rename(&unpublished, &directory).map_err(error)?;
        sync_directory(&self.root.join(".transactions"))?;
        sync_directory(&self.root.join(".garbage"))?;
        Ok(FilesystemTransaction {
            root: self.root.clone(),
            directory,
            application: application.clone(),
            operation: operation.into(),
            id,
        })
    }

    fn execute<S: StateStore>(
        &self,
        transaction: &mut FilesystemTransaction,
        store: &mut S,
    ) -> Result<(), AppManagerError> {
        let id = transaction.id.clone();
        let plan = transaction.plan();
        let result = UpdateEngine::new(store, transaction).execute(&id, &plan);
        match result {
            Ok(_) => transaction.cleanup().map_err(|cleanup| {
                error(format!("transaction committed; cleanup pending: {cleanup}"))
            }),
            Err(failure) => {
                if journal_phase(store, &id)?.as_deref() == Some("rolled-back") {
                    transaction.cleanup().map_err(|cleanup| {
                        error(format!(
                            "transaction rolled back: {failure}; cleanup pending: {cleanup}"
                        ))
                    })?;
                }
                Err(error(failure))
            }
        }
    }
}

impl ApplicationManager for FilesystemApplicationManager {
    fn plan(&self, operation: &ApplicationOperation) -> Result<ApplicationPlan, AppManagerError> {
        self.require_reconciled()?;
        match operation {
            ApplicationOperation::Install(application) => validate_id(application.id().as_str())?,
            ApplicationOperation::Update { from, to } => {
                self.verify(from)?;
                if from.id() != to.id() || from.version() == to.version() {
                    return Err(error("invalid application update"));
                }
            }
            ApplicationOperation::Remove(application)
            | ApplicationOperation::Verify(application) => self.verify(application)?,
            ApplicationOperation::Migrate { .. } => {
                return Err(error("application data migration is not implemented"));
            }
        }
        Ok(ApplicationPlan::new(operation.clone()))
    }
    fn verify(&self, application: &ApplicationRef) -> Result<(), AppManagerError> {
        self.installed(application).map(|_| ())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    format: u16,
    id: String,
    version: String,
    content_identity: String,
    transaction: String,
    directory_name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    format: u16,
    transaction: String,
    id: String,
    version: String,
    operation: String,
}
impl Checkpoint {
    fn validate(&self) -> Result<(), AppManagerError> {
        validate_id(&self.id)?;
        parse_version(&self.version)?;
        UpdateId::new(self.transaction.clone()).map_err(error)?;
        if self.format != 1 || !matches!(self.operation.as_str(), "install" | "remove") {
            return Err(error("unsupported application checkpoint"));
        }
        Ok(())
    }
}

struct FilesystemTransaction {
    root: PathBuf,
    directory: PathBuf,
    application: ApplicationRef,
    operation: String,
    id: UpdateId,
}
impl FilesystemTransaction {
    fn target(&self) -> PathBuf {
        self.root.join(storage_slot(
            self.application.id().as_str(),
            &self.application.version().to_string(),
        ))
    }
    fn operation(&self) -> UpdateOperation {
        if self.operation == "install" {
            UpdateOperation::InstallApplication(
                self.application.id().clone(),
                self.application.version(),
            )
        } else {
            UpdateOperation::RemoveApplication(
                self.application.id().clone(),
                self.application.version(),
            )
        }
    }
    fn plan(&self) -> UpdatePlan {
        let mut plan = UpdatePlan::new();
        plan.push(self.operation());
        plan
    }
    fn cleanup(&mut self) -> Result<(), AppManagerError> {
        let garbage = self.root.join(".garbage").join(self.id.as_str());
        if fs::symlink_metadata(&garbage).is_ok() {
            return Err(error("transaction cleanup target occupied"));
        }
        fs::rename(&self.directory, &garbage).map_err(error)?;
        self.directory = garbage;
        sync_directory(&self.root.join(".transactions"))?;
        sync_directory(&self.root.join(".garbage"))?;
        thaw_and_remove(&self.directory)?;
        sync_directory(&self.root.join(".garbage"))
    }
    fn check_operation(&self, operation: &UpdateOperation) -> Result<(), UpdateError> {
        if operation != &self.operation() {
            return Err(UpdateError::new(
                "operation does not match the application checkpoint",
            ));
        }
        Ok(())
    }
}

struct RecoveryAuthorization;
impl AuthorizationSource for RecoveryAuthorization {
    fn authorization_requests(&self) -> Vec<AuthorizationRequest> {
        vec![AuthorizationRequest {
            principal: Principal::System,
            resource: Resource::System,
            permission: Permission::Manage,
        }]
    }
}

impl UpdateBackend for FilesystemTransaction {
    fn prepare(&mut self, plan: &UpdatePlan) -> Result<(), UpdateError> {
        if plan.operations() != [self.operation()] {
            return Err(UpdateError::new("unexpected application update plan"));
        }
        if self.operation == "install" {
            checked_directory(&self.directory.join("staged")).map_err(update_error)?;
        }
        Ok(())
    }
    fn apply(&mut self, operation: &UpdateOperation) -> Result<(), UpdateError> {
        self.check_operation(operation)?;
        let target = self.target();
        checked_directory(
            target
                .parent()
                .ok_or_else(|| UpdateError::new("invalid target"))?,
        )
        .map_err(update_error)?;
        if self.operation == "install" {
            if fs::symlink_metadata(&target).is_ok() {
                return Err(UpdateError::new("immutable version already exists"));
            }
            move_bundle(&self.directory.join("staged"), &target).map_err(update_error)?;
        } else {
            checked_directory(&target).map_err(update_error)?;
            move_bundle(&target, &self.directory.join("removed")).map_err(update_error)?;
        }
        sync_directory(target.parent().unwrap()).map_err(update_error)?;
        sync_directory(&self.directory).map_err(update_error)
    }
    fn verify(&mut self, _: &UpdatePlan) -> Result<(), UpdateError> {
        if self.operation == "install" {
            let target = self.target();
            FilesystemApplicationManager::read_installed_at(&self.root, &self.application)
                .map_err(update_error)?;
            let receipt: Receipt =
                toml::from_str(&read_text(&target.join("receipt.toml")).map_err(update_error)?)
                    .map_err(update_error)?;
            if receipt.transaction != self.id.as_str() {
                return Err(UpdateError::new("installed receipt changed"));
            }
        } else if fs::symlink_metadata(self.target()).is_ok() {
            return Err(UpdateError::new("removed version still registered"));
        }
        Ok(())
    }
    fn rollback(&mut self, operation: &UpdateOperation) -> Result<(), UpdateError> {
        self.check_operation(operation)?;
        let target = self.target();
        if self.operation == "install" {
            if fs::symlink_metadata(&target).is_ok() {
                checked_directory(&target).map_err(update_error)?;
                let receipt: Receipt =
                    toml::from_str(&read_text(&target.join("receipt.toml")).map_err(update_error)?)
                        .map_err(update_error)?;
                if receipt.transaction != self.id.as_str() {
                    return Err(UpdateError::new(
                        "refusing rollback of a different installation",
                    ));
                }
                // Keep discarded content in the checkpoint until the journal
                // confirms rollback, including across a second interruption.
                if fs::symlink_metadata(self.directory.join("discarded")).is_ok() {
                    return Err(UpdateError::new("conflicting rollback checkpoint"));
                }
                move_bundle(&target, &self.directory.join("discarded")).map_err(update_error)?;
            }
        } else if fs::symlink_metadata(self.directory.join("removed")).is_ok() {
            checked_directory(&self.directory.join("removed")).map_err(update_error)?;
            if fs::symlink_metadata(&target).is_ok() {
                return Err(UpdateError::new(
                    "rollback target occupied; old version retained",
                ));
            }
            move_bundle(&self.directory.join("removed"), &target).map_err(update_error)?;
        } else if self.operation == "remove" && fs::symlink_metadata(&target).is_ok() {
            // A failed move can have changed only the outer directory mode.
            checked_directory(&target).map_err(update_error)?;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o555))
                .map_err(update_error)?;
            sync_directory(&target).map_err(update_error)?;
        }
        if let Some(parent) = target.parent().filter(|p| p.exists()) {
            sync_directory(parent).map_err(update_error)?;
        }
        sync_directory(&self.directory).map_err(update_error)
    }
}

fn journal_phase<S: StateStore>(
    store: &S,
    id: &UpdateId,
) -> Result<Option<String>, AppManagerError> {
    store
        .get(&StateKey::new(format!("updates/{}/phase", id.as_str())))
        .map_err(error)?
        .map(|value| String::from_utf8(value.into_bytes()).map_err(error))
        .transpose()
}
fn validate_id(id: &str) -> Result<(), AppManagerError> {
    if id.len() > 200
        || !id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        || id.contains("..")
    {
        return Err(error("Bundle ID is not a safe installation identifier"));
    }
    Ok(())
}
fn validate_slot(slot: &str) -> Result<(), AppManagerError> {
    if slot.len() != 64
        || !slot
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(error("invalid private registration slot"));
    }
    Ok(())
}
fn validate_directory_name(name: &str) -> Result<(), AppManagerError> {
    if !name.ends_with(".lunaapp")
        || name.len() > 100
        || name.contains("..")
        || !name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
    {
        return Err(error("unsafe installed application filename"));
    }
    Ok(())
}
fn checked_directory(path: &Path) -> Result<(), AppManagerError> {
    let metadata = fs::symlink_metadata(path).map_err(error)?;
    if !metadata.is_dir() {
        return Err(error(format!(
            "expected a real directory: {}",
            path.display()
        )));
    }
    checked_owner(&metadata)
}
fn checked_owner(metadata: &fs::Metadata) -> Result<(), AppManagerError> {
    // SAFETY: geteuid takes no arguments and only reads process credentials.
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o022 != 0 {
        return Err(error(
            "managed DATA paths must belong to the operator and exclude group/other writes",
        ));
    }
    Ok(())
}
fn checked_file(file: &File) -> Result<(), AppManagerError> {
    let metadata = file.metadata().map_err(error)?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(error(
            "managed file must be a regular file without hard links",
        ));
    }
    checked_owner(&metadata)
}
fn ensure_directory(path: &Path) -> Result<PathBuf, AppManagerError> {
    match fs::DirBuilder::new().mode(0o755).create(path) {
        Ok(()) => {
            sync_directory(
                path.parent()
                    .ok_or_else(|| error("directory has no parent"))?,
            )?;
        }
        Err(failure) if failure.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(failure) => return Err(error(failure)),
    }
    checked_directory(path)?;
    Ok(path.to_path_buf())
}
fn open_regular(path: &Path) -> Result<File, AppManagerError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(error)?;
    if !file.metadata().map_err(error)?.is_file() {
        return Err(error("expected a regular Bundle file"));
    }
    Ok(file)
}
fn read_text(path: &Path) -> Result<String, AppManagerError> {
    let file = open_regular(path)?;
    checked_file(&file)?;
    let mut text = String::new();
    file.take(1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(error)?;
    if text.len() > 1024 * 1024 {
        return Err(error("metadata exceeds the size limit"));
    }
    Ok(text)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), AppManagerError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(error)?;
    file.write_all(bytes).map_err(error)?;
    file.sync_all().map_err(error)
}
fn sync_directory(path: &Path) -> Result<(), AppManagerError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(error)
}

fn move_bundle(from: &Path, to: &Path) -> Result<(), AppManagerError> {
    checked_directory(from)?;
    // Linux requires write access to a directory moved between parents in
    // order to update '..'. Payload files/dirs remain frozen; the registry is
    // inaccessible while this journalled management transaction is in flight.
    fs::set_permissions(from, fs::Permissions::from_mode(0o755)).map_err(error)?;
    sync_directory(from)?;
    fs::rename(from, to).map_err(error)?;
    fs::set_permissions(to, fs::Permissions::from_mode(0o555)).map_err(error)?;
    sync_directory(to)
}
fn freeze_tree(path: &Path) -> Result<(), AppManagerError> {
    let metadata = fs::symlink_metadata(path).map_err(error)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(error)? {
            freeze_tree(&entry.map_err(error)?.path())?;
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o555)).map_err(error)?;
        sync_directory(path)
    } else if metadata.is_file() && metadata.nlink() == 1 {
        let mode = if metadata.mode() & 0o111 != 0 {
            0o555
        } else {
            0o444
        };
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(error)?;
        File::open(path)
            .and_then(|file| file.sync_all())
            .map_err(error)
    } else {
        Err(error("unsafe entry in application staging"))
    }
}
fn thaw_and_remove(path: &Path) -> Result<(), AppManagerError> {
    let metadata = fs::symlink_metadata(path).map_err(error)?;
    checked_owner(&metadata)?;
    if metadata.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(error)?;
        for entry in fs::read_dir(path).map_err(error)? {
            thaw_and_remove(&entry.map_err(error)?.path())?;
        }
        fs::remove_dir(path).map_err(error)
    } else if metadata.is_file() && metadata.nlink() == 1 {
        fs::remove_file(path).map_err(error)
    } else {
        Err(error(
            "unsafe entry in transaction cleanup; checkpoint retained",
        ))
    }
}
fn error(value: impl std::fmt::Display) -> AppManagerError {
    AppManagerError::new(value.to_string())
}
fn update_error(value: impl std::fmt::Display) -> UpdateError {
    UpdateError::new(value.to_string())
}
