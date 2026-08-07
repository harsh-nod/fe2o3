use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BackendPublicationReceiptV1, BuildAttempt, BuildSession,
    CanonicalLinkRequestIdentityV1, DurableLinkPublicationPlanV1, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    PinnedWorkerIdentityV1, ProducerIdentity, RecoveredWorkerV2PublicationIntentV1,
    TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1,
    WorkerV2PublicationIntentErrorV1, WorkerV2PublicationIntentIdentityV1,
    persist_worker_v2_publication_intent_v1, producer_package_identity_v1,
    recover_worker_v2_publication_intent_v1,
};
use fe2o3_compiler_ffi::CodeObjectVersion;
use fe2o3_hsaco_finalize::{
    InspectedRawWorkerV2HsacoV1, PreparedWorkerV2HsacoPublicationV1, WorkerV2HsacoPublicationError,
    prepare_worker_v2_hsaco_publication_v1,
};
use rustix::fd::{FromRawFd, OwnedFd};
use rustix::fs::{
    AtFlags, FileType, FlockOperation, Mode, OFlags, RenameFlags, flock, fstat, fsync, open,
    openat, renameat, renameat_with, statat, unlinkat,
};
use sha2::{Digest, Sha256};

const MARKER_MAGIC: &[u8] = b"FE2O3-CARGO-WORKER-V2-RESUME-V1\0";
const MARKER_VERSION: u16 = 1;
const MARKER_CHECKSUM_DOMAIN: &[u8] = b"FE2O3/CARGO-WORKER-V2-RESUME-CHECKSUM/V1\0";
const ADMISSION_COMMITMENT_DOMAIN: &[u8] = b"FE2O3/CARGO-WORKER-V2-ADMISSION-COMMITMENT/V1\0";
const MARKER_PREFIX: &str = ".fe2o3-cargo-worker-v2-resume-v1-";
const LOCK_SUFFIX: &str = ".lock";
const RECORD_SUFFIX: &str = ".record";
const TEMP_SUFFIX: &str = ".tmp-";
const RECEIPT_FIELDS: usize = 7;
const MARKER_BYTES: usize =
    MARKER_MAGIC.len() + 2 + 1 + 32 + 8 + 16 + 32 + 32 + RECEIPT_FIELDS * 32 + 32;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReceiptRecordV1([[u8; 32]; RECEIPT_FIELDS]);

impl ReceiptRecordV1 {
    pub(crate) fn from_receipt(receipt: BackendPublicationReceiptV1) -> Self {
        Self([
            receipt.attempt_identity(),
            receipt.producer_identity(),
            receipt.scope_identity(),
            receipt.plan_commitment(),
            receipt.upstream_evidence_identity(),
            receipt.finalized_output_identity(),
            receipt.publication_identity(),
        ])
    }

    pub(crate) fn matches(self, receipt: BackendPublicationReceiptV1) -> bool {
        self == Self::from_receipt(receipt)
    }

    fn encode(self, bytes: &mut Vec<u8>) {
        for field in self.0 {
            bytes.extend_from_slice(&field);
        }
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, &'static str> {
        let mut fields = [[0_u8; 32]; RECEIPT_FIELDS];
        for field in &mut fields {
            *field = decoder.array()?;
        }
        Ok(Self(fields))
    }

    fn is_zero(self) -> bool {
        self.0.iter().all(|field| *field == [0; 32])
    }
}

#[allow(clippy::large_enum_variant)] // The fixed receipt is kept inline for exact marker equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResumeMarkerStateV1 {
    Pending {
        attempt: BuildAttempt,
        admission: [u8; 32],
    },
    Ready {
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV1,
    },
    Completed {
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV1,
        receipt: ReceiptRecordV1,
    },
}

#[derive(Debug)]
pub(crate) enum RestartIntentErrorV1 {
    Marker(ResumeMarkerErrorV1),
    Intent(WorkerV2PublicationIntentErrorV1),
    Preparation(WorkerV2HsacoPublicationError),
    IntentIdentityMismatch,
}

impl fmt::Display for RestartIntentErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Marker(error) => write!(formatter, "Worker V2 resume marker failed: {error}"),
            Self::Intent(error) => {
                write!(formatter, "Worker V2 publication intent failed: {error}")
            }
            Self::Preparation(error) => {
                write!(
                    formatter,
                    "Worker V2 publication preparation failed: {error}"
                )
            }
            Self::IntentIdentityMismatch => formatter.write_str(
                "recovered Worker V2 publication intent does not match its resume marker",
            ),
        }
    }
}

impl Error for RestartIntentErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Marker(error) => Some(error),
            Self::Intent(error) => Some(error),
            Self::Preparation(error) => Some(error),
            Self::IntentIdentityMismatch => None,
        }
    }
}

pub(crate) struct PersistedAdmittedWorkerV2IntentV1 {
    pub(crate) intent: RecoveredWorkerV2PublicationIntentV1,
    pub(crate) prepared: PreparedWorkerV2HsacoPublicationV1,
}

impl From<ResumeMarkerErrorV1> for RestartIntentErrorV1 {
    fn from(error: ResumeMarkerErrorV1) -> Self {
        Self::Marker(error)
    }
}

impl From<WorkerV2PublicationIntentErrorV1> for RestartIntentErrorV1 {
    fn from(error: WorkerV2PublicationIntentErrorV1) -> Self {
        Self::Intent(error)
    }
}

pub(crate) fn persist_admitted_worker_v2_intent_v1(
    store: &WorkerV2ResumeStoreV1,
    producer: &ProducerIdentity,
    inspected: InspectedRawWorkerV2HsacoV1,
) -> Result<PersistedAdmittedWorkerV2IntentV1, RestartIntentErrorV1> {
    let attempt = inspected.attempt();
    let (plan, upstream) = derive_publication_plan_v1(producer, &inspected);
    let admission = restart_admission_commitment_v1(plan, upstream, inspected.exact_bytes());
    let prepared = prepare_worker_v2_hsaco_publication_v1(producer, inspected)
        .map_err(RestartIntentErrorV1::Preparation)?;
    store.persist_pending(attempt, admission)?;
    store.verify_output_path()?;
    let persisted = persist_worker_v2_publication_intent_v1(
        &store.display_path,
        producer,
        attempt,
        plan,
        upstream,
        prepared.exact_bytes(),
    )?;
    store.verify_output_path()?;
    store.persist_ready(attempt, persisted.record().identity())?;
    Ok(PersistedAdmittedWorkerV2IntentV1 {
        intent: persisted,
        prepared,
    })
}

pub(crate) fn recover_worker_v2_intent_v1(
    store: &WorkerV2ResumeStoreV1,
    producer: &ProducerIdentity,
    state: ResumeMarkerStateV1,
) -> Result<RecoveredWorkerV2PublicationIntentV1, RestartIntentErrorV1> {
    let attempt = state.attempt();
    store.verify_output_path()?;
    let recovered =
        recover_worker_v2_publication_intent_v1(&store.display_path, producer, attempt)?;
    store.verify_output_path()?;
    if let Some(expected) = state.intent()
        && recovered.record().identity() != expected
    {
        return Err(RestartIntentErrorV1::IntentIdentityMismatch);
    }
    if let ResumeMarkerStateV1::Pending { admission, .. } = state
        && restart_admission_commitment_v1(
            recovered.record().plan(),
            recovered.record().upstream_evidence(),
            recovered.exact_output(),
        ) != admission
    {
        return Err(RestartIntentErrorV1::IntentIdentityMismatch);
    }
    if matches!(state, ResumeMarkerStateV1::Pending { .. }) {
        store.persist_ready(attempt, recovered.record().identity())?;
    }
    Ok(recovered)
}

// This derivation is intentionally byte-for-byte compatible with the raw-HSACO publication
// bridge. The upstream bridge currently keeps its plan private, so cargo must reconstruct the plan
// before the restart journal can retain it. Publication still requires the attempt protocol.
fn derive_publication_plan_v1(
    producer: &ProducerIdentity,
    inspected: &InspectedRawWorkerV2HsacoV1,
) -> (
    DurableLinkPublicationPlanV1,
    UpstreamCodeObjectEvidenceIdentityV1,
) {
    const KERNEL_SET_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-KERNEL-SET/V1\0";
    const TARGET_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-TARGET/V1\0";
    const REQUEST_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-REQUEST/V1\0";
    const WORKER_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-WORKER/V1\0";
    const RESPONSE_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-RESPONSE/V1\0";
    const INSPECTION_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-RAW-INSPECTION/V1\0";
    const PUBLICATION_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-ATOMIC-PUBLICATION/V1\0";

    let producer_package = producer_package_identity_v1(producer);
    let manifest = inspected.policy().symbol_manifest().identity();
    let compiler_envelope = inspected.compiler_envelope_identity();
    let kernel_set = KernelSetIdentityV1::from_bytes(hash_identity(KERNEL_SET_DOMAIN, |digest| {
        digest.update(manifest.sha256());
        digest.update(manifest.byte_len().to_le_bytes());
        digest.update(compiler_envelope.as_bytes());
    }));

    let launch = inspected.policy().launch();
    let target_text = inspected.target().to_string();
    let target = TargetIdentityV1::from_bytes(hash_identity(TARGET_DOMAIN, |digest| {
        update_length_prefixed(digest, target_text.as_bytes());
        digest.update([match inspected.code_object_version() {
            CodeObjectVersion::V4 => 4,
            CodeObjectVersion::V5 => 5,
            CodeObjectVersion::V6 => 6,
        }]);
        for axis in launch.required_workgroup_size() {
            digest.update(axis.to_le_bytes());
        }
        digest.update(launch.max_flat_workgroup_size().to_le_bytes());
        digest.update(launch.wavefront_size().to_le_bytes());
    }));
    let scope = LinkPublicationScopeV1::new(producer_package, kernel_set, target);

    let source = inspected.source_evidence_identity();
    let request =
        CanonicalLinkRequestIdentityV1::from_bytes(hash_identity(REQUEST_DOMAIN, |digest| {
            digest.update(inspected.sealed_request_id());
            digest.update(inspected.sealed_request_identity());
            digest.update(inspected.handoff_identity().as_bytes());
            digest.update(manifest.sha256());
            digest.update(manifest.byte_len().to_le_bytes());
            digest.update(inspected.link_plan_identity().as_bytes());
            digest.update(inspected.policy().compiler_envelope_identity().as_bytes());
            digest.update(inspected.policy().identity().as_bytes());
            digest.update(source.as_bytes());
            digest.update(inspected.worker_measurement().executable().sha256());
            digest.update(
                inspected
                    .worker_measurement()
                    .executable()
                    .byte_len()
                    .to_le_bytes(),
            );
        }));

    let measurement = inspected.worker_measurement();
    let executable = measurement.executable();
    let worker = PinnedWorkerIdentityV1::from_bytes(hash_identity(WORKER_DOMAIN, |digest| {
        digest.update(executable.sha256());
        digest.update(executable.byte_len().to_le_bytes());
        update_length_prefixed(digest, measurement.worker_build_identity().as_bytes());
        update_length_prefixed(digest, measurement.llvm_build_identity().as_bytes());
    }));
    let response =
        ValidatedResponseIdentityV1::from_bytes(hash_identity(RESPONSE_DOMAIN, |digest| {
            digest.update(inspected.response_identity().as_bytes())
        }));
    let output_digest: [u8; 32] = Sha256::digest(inspected.exact_bytes()).into();
    let linked_output = LinkedOutputIdentityV1::from_bytes(output_digest);
    let finalization =
        FinalizationIdentityV1::from_bytes(hash_identity(INSPECTION_DOMAIN, |digest| {
            digest.update(inspected.identity().as_bytes())
        }));
    let finalized_output = FinalizedOutputIdentityV1::from_bytes(output_digest);
    let attempt = inspected.attempt();
    let publication =
        AtomicPublicationIdentityV1::from_bytes(hash_identity(PUBLICATION_DOMAIN, |digest| {
            digest.update(attempt.generation().to_le_bytes());
            digest.update(attempt.session().as_bytes());
            digest.update(attempt.invocation().as_bytes());
            digest.update(producer_package.as_bytes());
            digest.update(kernel_set.as_bytes());
            digest.update(target.as_bytes());
            digest.update(request.as_bytes());
            digest.update(worker.as_bytes());
            digest.update(response.as_bytes());
            digest.update(linked_output.as_bytes());
            digest.update(finalization.as_bytes());
            digest.update(finalized_output.as_bytes());
            digest.update(inspected.identity().as_bytes());
        }));
    let plan = DurableLinkPublicationPlanV1::new(
        attempt,
        scope,
        request,
        worker,
        response,
        linked_output,
        finalization,
        finalized_output,
        publication,
    );
    let upstream =
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes(*inspected.identity().as_bytes());
    (plan, upstream)
}

pub(crate) fn restart_admission_commitment_v1(
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    output: &[u8],
) -> [u8; 32] {
    hash_identity(ADMISSION_COMMITMENT_DOMAIN, |digest| {
        let attempt = plan.attempt();
        digest.update(attempt.generation().to_le_bytes());
        digest.update(attempt.session().as_bytes());
        digest.update(attempt.invocation().as_bytes());
        digest.update(plan.scope().package().as_bytes());
        digest.update(plan.scope().kernel_set().as_bytes());
        digest.update(plan.scope().target().as_bytes());
        digest.update(plan.request().as_bytes());
        digest.update(plan.worker().as_bytes());
        digest.update(plan.response().as_bytes());
        digest.update(plan.linked_output().as_bytes());
        digest.update(plan.finalization().as_bytes());
        digest.update(plan.finalized_output().as_bytes());
        digest.update(plan.publication().as_bytes());
        digest.update(upstream.as_bytes());
        digest.update(Sha256::digest(output));
        digest.update((output.len() as u64).to_le_bytes());
    })
}

fn hash_identity(domain: &[u8], update: impl FnOnce(&mut Sha256)) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    update(&mut digest);
    digest.finalize().into()
}

fn update_length_prefixed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

impl ResumeMarkerStateV1 {
    pub(crate) const fn attempt(self) -> BuildAttempt {
        match self {
            Self::Pending { attempt, .. }
            | Self::Ready { attempt, .. }
            | Self::Completed { attempt, .. } => attempt,
        }
    }

    pub(crate) const fn intent(self) -> Option<WorkerV2PublicationIntentIdentityV1> {
        match self {
            Self::Pending { .. } => None,
            Self::Ready { intent, .. } | Self::Completed { intent, .. } => Some(intent),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ResumeMarkerErrorV1 {
    Io(std::io::Error),
    OutputDirectoryChanged(PathBuf),
    InvalidMarker { path: PathBuf, reason: String },
    ConflictingMarker,
    InvalidTransition,
    StaleInvocation,
}

impl fmt::Display for ResumeMarkerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::OutputDirectoryChanged(path) => write!(
                formatter,
                "Worker V2 resume output directory changed: {}",
                path.display()
            ),
            Self::InvalidMarker { path, reason } => write!(
                formatter,
                "invalid Worker V2 resume marker {}: {reason}",
                path.display()
            ),
            Self::ConflictingMarker => formatter
                .write_str("a different Worker V2 resume marker already exists for this producer"),
            Self::InvalidTransition => {
                formatter.write_str("invalid Worker V2 resume marker state transition")
            }
            Self::StaleInvocation => formatter.write_str(
                "Worker V2 resume marker belongs to a different build session or invocation",
            ),
        }
    }
}

impl Error for ResumeMarkerErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ResumeMarkerErrorV1 {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub(crate) struct WorkerV2ResumeStoreV1 {
    directory: OwnedFd,
    _lock: OwnedFd,
    display_path: PathBuf,
    device: u64,
    inode: u64,
    package: [u8; 32],
    marker_name: String,
}

impl WorkerV2ResumeStoreV1 {
    pub(crate) fn open(
        output_dir: &Path,
        producer: &ProducerIdentity,
    ) -> Result<Self, ResumeMarkerErrorV1> {
        let directory = open_output_directory(output_dir, true)?;
        let directory_stat = fstat(&directory).map_err(std::io::Error::from)?;
        if FileType::from_raw_mode(directory_stat.st_mode) != FileType::Directory {
            return Err(Self::invalid_at(
                output_dir,
                "output path is not a directory",
            ));
        }

        let package = *producer_package_identity_v1(producer).as_bytes();
        let stem = format!("{MARKER_PREFIX}{}", hex(&package));
        let lock_name = format!("{stem}{LOCK_SUFFIX}");
        let marker_name = format!("{stem}{RECORD_SUFFIX}");
        let lock = openat(
            &directory,
            &lock_name,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(std::io::Error::from)?;
        validate_private_file(&directory, &lock, &lock_name, output_dir, None)?;
        flock(&lock, FlockOperation::LockExclusive).map_err(std::io::Error::from)?;
        validate_private_file(&directory, &lock, &lock_name, output_dir, None)?;

        let store = Self {
            directory,
            _lock: lock,
            display_path: output_dir.to_path_buf(),
            device: directory_stat.st_dev,
            inode: directory_stat.st_ino,
            package,
            marker_name,
        };
        store.verify_output_path()?;
        Ok(store)
    }

    pub(crate) fn verify_output_path(&self) -> Result<(), ResumeMarkerErrorV1> {
        let reopened = open_output_directory(&self.display_path, false)?;
        let stat = fstat(&reopened).map_err(std::io::Error::from)?;
        if stat.st_dev != self.device || stat.st_ino != self.inode {
            return Err(ResumeMarkerErrorV1::OutputDirectoryChanged(
                self.display_path.clone(),
            ));
        }
        Ok(())
    }

    pub(crate) fn load(&self) -> Result<Option<ResumeMarkerStateV1>, ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        let descriptor = match openat(
            &self.directory,
            &self.marker_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        validate_private_file(
            &self.directory,
            &descriptor,
            &self.marker_name,
            &self.display_path,
            Some(MARKER_BYTES),
        )?;
        let mut file = File::from(descriptor);
        let mut bytes = Vec::with_capacity(MARKER_BYTES + 1);
        Read::by_ref(&mut file)
            .take((MARKER_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let final_stat = fstat(&file).map_err(std::io::Error::from)?;
        if final_stat.st_nlink != 1 || final_stat.st_size != MARKER_BYTES as i64 {
            return Err(self.invalid("marker changed while it was read"));
        }
        decode_marker(&bytes, self.package)
            .map(Some)
            .map_err(|reason| self.invalid(reason))
    }

    pub(crate) fn persist_pending(
        &self,
        attempt: BuildAttempt,
        admission: [u8; 32],
    ) -> Result<(), ResumeMarkerErrorV1> {
        if admission == [0; 32] {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        let pending = ResumeMarkerStateV1::Pending { attempt, admission };
        match self.load()? {
            None => self.write(pending, false),
            Some(existing) if existing == pending => Ok(()),
            Some(_) => Err(ResumeMarkerErrorV1::ConflictingMarker),
        }
    }

    pub(crate) fn persist_ready(
        &self,
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let ready = ResumeMarkerStateV1::Ready { attempt, intent };
        match self.load()? {
            Some(ResumeMarkerStateV1::Pending {
                attempt: current, ..
            }) if current == attempt => self.write(ready, true),
            Some(existing) if existing == ready => Ok(()),
            _ => Err(ResumeMarkerErrorV1::InvalidTransition),
        }
    }

    pub(crate) fn persist_completed(
        &self,
        attempt: BuildAttempt,
        intent: WorkerV2PublicationIntentIdentityV1,
        receipt: BackendPublicationReceiptV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        let completed = ResumeMarkerStateV1::Completed {
            attempt,
            intent,
            receipt: ReceiptRecordV1::from_receipt(receipt),
        };
        match self.load()? {
            Some(ResumeMarkerStateV1::Ready {
                attempt: current_attempt,
                intent: current_intent,
            }) if current_attempt == attempt && current_intent == intent => {
                self.write(completed, true)
            }
            Some(existing) if existing == completed => Ok(()),
            _ => Err(ResumeMarkerErrorV1::InvalidTransition),
        }
    }

    pub(crate) fn clear_completed(
        &self,
        expected: ResumeMarkerStateV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if !matches!(expected, ResumeMarkerStateV1::Completed { .. }) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        self.clear_exact(expected)
    }

    pub(crate) fn clear_exact(
        &self,
        expected: ResumeMarkerStateV1,
    ) -> Result<(), ResumeMarkerErrorV1> {
        if self.load()? != Some(expected) {
            return Err(ResumeMarkerErrorV1::InvalidTransition);
        }
        unlinkat(&self.directory, &self.marker_name, AtFlags::empty())
            .map_err(std::io::Error::from)?;
        fsync(&self.directory).map_err(std::io::Error::from)?;
        self.verify_output_path()?;
        Ok(())
    }

    fn write(&self, state: ResumeMarkerStateV1, replace: bool) -> Result<(), ResumeMarkerErrorV1> {
        self.verify_output_path()?;
        let bytes = encode_marker(self.package, state);
        let temp_name = format!(
            "{}{TEMP_SUFFIX}{}-{}",
            self.marker_name,
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        );
        let result = (|| {
            let descriptor = openat(
                &self.directory,
                &temp_name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(std::io::Error::from)?;
            let mut file = File::from(descriptor);
            file.write_all(&bytes)?;
            file.sync_all()?;
            validate_private_file(
                &self.directory,
                &file,
                &temp_name,
                &self.display_path,
                Some(MARKER_BYTES),
            )?;
            self.verify_output_path()?;
            if replace {
                renameat(
                    &self.directory,
                    &temp_name,
                    &self.directory,
                    &self.marker_name,
                )
                .map_err(std::io::Error::from)?;
            } else {
                renameat_with(
                    &self.directory,
                    &temp_name,
                    &self.directory,
                    &self.marker_name,
                    RenameFlags::NOREPLACE,
                )
                .map_err(std::io::Error::from)?;
            }
            fsync(&self.directory).map_err(std::io::Error::from)?;
            self.verify_output_path()?;
            if self.load()? != Some(state) {
                return Err(self.invalid("marker changed after atomic publication"));
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = unlinkat(&self.directory, &temp_name, AtFlags::empty());
        }
        result
    }

    fn invalid(&self, reason: impl Into<String>) -> ResumeMarkerErrorV1 {
        ResumeMarkerErrorV1::InvalidMarker {
            path: self.display_path.join(&self.marker_name),
            reason: reason.into(),
        }
    }

    fn invalid_at(path: &Path, reason: impl Into<String>) -> ResumeMarkerErrorV1 {
        ResumeMarkerErrorV1::InvalidMarker {
            path: path.to_path_buf(),
            reason: reason.into(),
        }
    }
}

fn open_output_directory(path: &Path, create: bool) -> Result<OwnedFd, ResumeMarkerErrorV1> {
    #[cfg(target_os = "linux")]
    if let Some(directory) = duplicate_proc_self_fd_directory(path) {
        return directory.map_err(Into::into);
    }

    if create {
        std::fs::create_dir_all(path)?;
    }
    open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)
    .map_err(Into::into)
}

#[cfg(target_os = "linux")]
fn duplicate_proc_self_fd_directory(path: &Path) -> Option<std::io::Result<OwnedFd>> {
    use std::os::unix::ffi::OsStrExt;

    const PREFIX: &[u8] = b"/proc/self/fd/";
    let descriptor = path.as_os_str().as_bytes().strip_prefix(PREFIX)?;
    let canonical = descriptor == b"0"
        || descriptor
            .first()
            .is_some_and(|byte| matches!(byte, b'1'..=b'9'))
            && descriptor.iter().all(u8::is_ascii_digit);
    if !canonical {
        return Some(Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "procfs descriptor path is not canonical",
        )));
    }
    let Some(raw_fd) = descriptor.iter().try_fold(0_i32, |value, digit| {
        value.checked_mul(10)?.checked_add(i32::from(*digit - b'0'))
    }) else {
        return Some(Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "procfs descriptor number is out of range",
        )));
    };

    // Raw fcntl reports EBADF for stale descriptor numbers without manufacturing an invalid
    // BorrowedFd. A successful return is a new descriptor owned by this process.
    let duplicated = unsafe { libc::fcntl(raw_fd, libc::F_DUPFD_CLOEXEC, 0) };
    if duplicated < 0 {
        return Some(Err(std::io::Error::last_os_error()));
    }
    let directory = unsafe { OwnedFd::from_raw_fd(duplicated) };
    let stat = match fstat(&directory) {
        Ok(stat) => stat,
        Err(error) => return Some(Err(std::io::Error::from(error))),
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return Some(Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "procfs descriptor does not reference a directory",
        )));
    }
    Some(Ok(directory))
}

fn validate_private_file(
    directory: &OwnedFd,
    descriptor: &impl rustix::fd::AsFd,
    name: &str,
    display_path: &Path,
    expected_size: Option<usize>,
) -> Result<(), ResumeMarkerErrorV1> {
    let descriptor_stat = fstat(descriptor).map_err(std::io::Error::from)?;
    let path_stat =
        statat(directory, name, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    let valid_kind = FileType::from_raw_mode(descriptor_stat.st_mode) == FileType::RegularFile
        && FileType::from_raw_mode(path_stat.st_mode) == FileType::RegularFile;
    let valid_identity =
        descriptor_stat.st_dev == path_stat.st_dev && descriptor_stat.st_ino == path_stat.st_ino;
    let valid_links = descriptor_stat.st_nlink == 1 && path_stat.st_nlink == 1;
    let valid_mode = descriptor_stat.st_mode & 0o077 == 0 && path_stat.st_mode & 0o077 == 0;
    let valid_size = expected_size.is_none_or(|size| {
        descriptor_stat.st_size == size as i64 && path_stat.st_size == size as i64
    });
    if !valid_kind || !valid_identity || !valid_links || !valid_mode || !valid_size {
        return Err(ResumeMarkerErrorV1::InvalidMarker {
            path: display_path.join(name),
            reason: "entry must be one private, single-link regular file with exact size".into(),
        });
    }
    Ok(())
}

fn encode_marker(package: [u8; 32], state: ResumeMarkerStateV1) -> Vec<u8> {
    let attempt = state.attempt();
    let (stage, intent, receipt) = match state {
        ResumeMarkerStateV1::Pending { admission, .. } => {
            (1, admission, ReceiptRecordV1([[0; 32]; 7]))
        }
        ResumeMarkerStateV1::Ready { intent, .. } => {
            (2, intent.as_bytes(), ReceiptRecordV1([[0; 32]; 7]))
        }
        ResumeMarkerStateV1::Completed {
            intent, receipt, ..
        } => (3, intent.as_bytes(), receipt),
    };
    let mut bytes = Vec::with_capacity(MARKER_BYTES);
    bytes.extend_from_slice(MARKER_MAGIC);
    bytes.extend_from_slice(&MARKER_VERSION.to_le_bytes());
    bytes.push(stage);
    bytes.extend_from_slice(&package);
    bytes.extend_from_slice(&attempt.generation().to_le_bytes());
    bytes.extend_from_slice(attempt.session().as_bytes());
    bytes.extend_from_slice(attempt.invocation().as_bytes());
    bytes.extend_from_slice(&intent);
    receipt.encode(&mut bytes);
    let checksum = checksum(&bytes);
    bytes.extend_from_slice(&checksum);
    debug_assert_eq!(bytes.len(), MARKER_BYTES);
    bytes
}

fn decode_marker(
    bytes: &[u8],
    expected_package: [u8; 32],
) -> Result<ResumeMarkerStateV1, &'static str> {
    if bytes.len() != MARKER_BYTES {
        return Err("marker has a noncanonical length");
    }
    let (body, encoded_checksum) = bytes.split_at(bytes.len() - 32);
    if checksum(body).as_slice() != encoded_checksum {
        return Err("marker checksum mismatch");
    }
    let mut decoder = Decoder::new(body);
    if decoder.take(MARKER_MAGIC.len())? != MARKER_MAGIC {
        return Err("marker magic mismatch");
    }
    if decoder.u16()? != MARKER_VERSION {
        return Err("unsupported marker version");
    }
    let stage = decoder.byte()?;
    if decoder.array()? != expected_package {
        return Err("marker producer package mismatch");
    }
    let generation = decoder.u64()?;
    let session = BuildSession::from_bytes(decoder.array()?);
    let invocation = fe2o3_artifact_transaction::BuildInvocation::from_bytes(decoder.array()?);
    let attempt = BuildAttempt::from_env_value(&format!(
        "{generation}:{}:{}",
        session.to_hex(),
        invocation.to_hex()
    ))
    .map_err(|_| "marker contains an invalid attempt")?;
    let intent = WorkerV2PublicationIntentIdentityV1::from_bytes(decoder.array()?);
    let receipt = ReceiptRecordV1::decode(&mut decoder)?;
    if !decoder.finished() {
        return Err("marker has trailing body bytes");
    }
    match stage {
        1 if intent.as_bytes() != [0; 32] && receipt.is_zero() => {
            Ok(ResumeMarkerStateV1::Pending {
                attempt,
                admission: intent.as_bytes(),
            })
        }
        2 if intent.as_bytes() != [0; 32] && receipt.is_zero() => {
            Ok(ResumeMarkerStateV1::Ready { attempt, intent })
        }
        3 if intent.as_bytes() != [0; 32] && !receipt.is_zero() => {
            Ok(ResumeMarkerStateV1::Completed {
                attempt,
                intent,
                receipt,
            })
        }
        _ => Err("marker stage fields are noncanonical"),
    }
}

fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(MARKER_CHECKSUM_DOMAIN);
    digest.update(bytes);
    digest.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        write!(text, "{byte:02x}").expect("writing to String cannot fail");
    }
    text
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], &'static str> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or("marker length overflow")?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or("marker is truncated")?;
        self.offset = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], &'static str> {
        Ok(self.take(N)?.try_into().unwrap())
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};

    use fe2o3_artifact_transaction::{
        AtomicPublicationIdentityV1, BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1,
        DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
        KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
        PinnedWorkerIdentityV1, TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1,
        ValidatedResponseIdentityV1, begin_build_attempt, clear_worker_v2_publication_intent_v1,
        publish_exact_hsaco_evidence_for_attempt_v1,
    };

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "cargo-fe2o3-worker-v2-resume-marker-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn producer(seed: u8) -> ProducerIdentity {
        ProducerIdentity::from_codegen(
            &format!("resume_{seed}"),
            Some(Path::new(&format!("/src/resume-{seed}.rs"))),
        )
        .unwrap()
    }

    fn attempt(path: &Path, producer: &ProducerIdentity, seed: u8) -> BuildAttempt {
        begin_build_attempt(
            path,
            producer,
            BuildInvocation::from_bytes([seed; 32]),
            BuildSession::from_bytes([seed.wrapping_add(1); 16]),
        )
        .unwrap()
    }

    fn publication_inputs(
        attempt: BuildAttempt,
        seed: u8,
    ) -> (
        Vec<u8>,
        DurableLinkPublicationPlanV1,
        UpstreamCodeObjectEvidenceIdentityV1,
    ) {
        let output = vec![seed; 19];
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            LinkPublicationScopeV1::new(
                PackageIdentityV1::from_bytes([seed; 32]),
                KernelSetIdentityV1::from_bytes([seed.wrapping_add(1); 32]),
                TargetIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
            ),
            CanonicalLinkRequestIdentityV1::from_bytes([seed.wrapping_add(3); 32]),
            PinnedWorkerIdentityV1::from_bytes([seed.wrapping_add(4); 32]),
            ValidatedResponseIdentityV1::from_bytes([seed.wrapping_add(5); 32]),
            LinkedOutputIdentityV1::from_bytes(Sha256::digest(&output).into()),
            FinalizationIdentityV1::from_bytes([seed.wrapping_add(6); 32]),
            FinalizedOutputIdentityV1::from_bytes(Sha256::digest(&output).into()),
            AtomicPublicationIdentityV1::from_bytes([seed.wrapping_add(7); 32]),
        );
        (
            output,
            plan,
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes([seed.wrapping_add(8); 32]),
        )
    }

    fn receipt(
        path: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        seed: u8,
    ) -> BackendPublicationReceiptV1 {
        let (output, plan, upstream) = publication_inputs(attempt, seed);
        publish_exact_hsaco_evidence_for_attempt_v1(
            path, producer, attempt, plan, upstream, &output,
        )
        .unwrap()
        .receipt()
    }

    #[test]
    fn canonical_state_machine_round_trips_exactly() {
        let directory = TestDirectory::new();
        let producer = producer(1);
        let attempt = attempt(&directory.0, &producer, 1);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        assert_eq!(store.load().unwrap(), None);

        let admission = [0x31; 32];
        store.persist_pending(attempt, admission).unwrap();
        assert_eq!(
            store.load().unwrap(),
            Some(ResumeMarkerStateV1::Pending { attempt, admission })
        );
        store.persist_pending(attempt, admission).unwrap();

        let intent = WorkerV2PublicationIntentIdentityV1::from_bytes([0x41; 32]);
        store.persist_ready(attempt, intent).unwrap();
        assert_eq!(
            store.load().unwrap(),
            Some(ResumeMarkerStateV1::Ready { attempt, intent })
        );
        store.persist_ready(attempt, intent).unwrap();

        let receipt = receipt(&directory.0, &producer, attempt, 7);
        store.persist_completed(attempt, intent, receipt).unwrap();
        let completed = ResumeMarkerStateV1::Completed {
            attempt,
            intent,
            receipt: ReceiptRecordV1::from_receipt(receipt),
        };
        assert_eq!(store.load().unwrap(), Some(completed));
        store.persist_completed(attempt, intent, receipt).unwrap();
        store.clear_completed(completed).unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn rejects_out_of_order_and_substituted_transitions() {
        let directory = TestDirectory::new();
        let producer = producer(2);
        let attempt = attempt(&directory.0, &producer, 2);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        let intent = WorkerV2PublicationIntentIdentityV1::from_bytes([0x42; 32]);

        assert!(matches!(
            store.persist_ready(attempt, intent),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
        store.persist_pending(attempt, [0x32; 32]).unwrap();
        assert!(matches!(
            store.persist_pending(
                BuildAttempt::from_env_value(&format!(
                    "{}:{}:{}",
                    attempt.generation() + 1,
                    attempt.session().to_hex(),
                    attempt.invocation().to_hex()
                ))
                .unwrap(),
                [0x32; 32]
            ),
            Err(ResumeMarkerErrorV1::ConflictingMarker)
        ));
        assert!(matches!(
            store.persist_ready(
                attempt,
                WorkerV2PublicationIntentIdentityV1::from_bytes([0x43; 32])
            ),
            Ok(())
        ));
        assert!(matches!(
            store.persist_completed(
                attempt,
                intent,
                receipt(&directory.0, &producer, attempt, 8)
            ),
            Err(ResumeMarkerErrorV1::InvalidTransition)
        ));
    }

    #[test]
    fn rejects_checksum_mode_symlink_and_hardlink_tamper() {
        for case in 0..4 {
            let directory = TestDirectory::new();
            let producer = producer(20 + case);
            let attempt = attempt(&directory.0, &producer, 20 + case);
            let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
            store.persist_pending(attempt, [0x33; 32]).unwrap();
            let marker = directory.0.join(&store.marker_name);
            drop(store);

            match case {
                0 => {
                    let mut bytes = fs::read(&marker).unwrap();
                    bytes[MARKER_MAGIC.len() + 3] ^= 1;
                    fs::write(&marker, bytes).unwrap();
                }
                1 => {
                    fs::set_permissions(&marker, fs::Permissions::from_mode(0o644)).unwrap();
                }
                2 => {
                    let replacement = directory.0.join("replacement");
                    fs::write(&replacement, vec![0; MARKER_BYTES]).unwrap();
                    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
                    fs::remove_file(&marker).unwrap();
                    symlink(&replacement, &marker).unwrap();
                }
                3 => {
                    fs::hard_link(&marker, directory.0.join("alias")).unwrap();
                }
                _ => unreachable!(),
            }

            let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
            assert!(store.load().is_err(), "tamper case {case} was accepted");
        }
    }

    #[test]
    fn rejects_output_directory_substitution() {
        let parent = TestDirectory::new();
        let output = parent.0.join("output");
        fs::create_dir(&output).unwrap();
        let producer = producer(40);
        let store = WorkerV2ResumeStoreV1::open(&output, &producer).unwrap();
        fs::rename(&output, parent.0.join("moved")).unwrap();
        fs::create_dir(&output).unwrap();
        assert!(matches!(
            store.verify_output_path(),
            Err(ResumeMarkerErrorV1::OutputDirectoryChanged(_))
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_self_fd_resume_store_stays_bound_to_retained_directory() {
        let parent = TestDirectory::new();
        let output = parent.0.join("output");
        let moved = parent.0.join("moved");
        fs::create_dir(&output).unwrap();
        let producer = producer(41);
        let attempt = attempt(&output, &producer, 41);
        let retained = fs::File::open(&output).unwrap();
        let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", retained.as_raw_fd()));
        let store = WorkerV2ResumeStoreV1::open(&descriptor_path, &producer).unwrap();
        let marker_name = store.marker_name.clone();
        store.persist_pending(attempt, [0x35; 32]).unwrap();

        fs::rename(&output, &moved).unwrap();
        fs::create_dir(&output).unwrap();
        let intent = WorkerV2PublicationIntentIdentityV1::from_bytes([0x45; 32]);
        store.persist_ready(attempt, intent).unwrap();
        drop(store);

        let reopened = WorkerV2ResumeStoreV1::open(&descriptor_path, &producer).unwrap();
        assert_eq!(
            reopened.load().unwrap(),
            Some(ResumeMarkerStateV1::Ready { attempt, intent })
        );
        assert!(moved.join(marker_name).is_file());
        assert!(fs::read_dir(&output).unwrap().next().is_none());
        assert!(open_output_directory(Path::new("/proc/self/fd/01"), false).is_err());
    }

    #[test]
    fn pending_marker_promotes_and_ready_marker_replays_after_restart() {
        let directory = TestDirectory::new();
        let producer = producer(50);
        let attempt = attempt(&directory.0, &producer, 50);
        let (output, plan, upstream) = publication_inputs(attempt, 50);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        store
            .persist_pending(
                attempt,
                restart_admission_commitment_v1(plan, upstream, &output),
            )
            .unwrap();
        let persisted = persist_worker_v2_publication_intent_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            &output,
        )
        .unwrap();
        let intent_identity = persisted.record().identity();
        drop(persisted);
        drop(store);

        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        let pending = store.load().unwrap().unwrap();
        assert!(matches!(pending, ResumeMarkerStateV1::Pending { .. }));
        let recovered = recover_worker_v2_intent_v1(&store, &producer, pending).unwrap();
        assert_eq!(recovered.record().identity(), intent_identity);
        assert_eq!(recovered.exact_output(), output);
        assert_eq!(
            store.load().unwrap(),
            Some(ResumeMarkerStateV1::Ready {
                attempt,
                intent: intent_identity,
            })
        );
        drop(recovered);
        drop(store);

        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        let ready = store.load().unwrap().unwrap();
        let recovered = recover_worker_v2_intent_v1(&store, &producer, ready).unwrap();
        let published = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory.0,
            &producer,
            attempt,
            recovered.record().plan(),
            recovered.record().upstream_evidence(),
            recovered.exact_output(),
        )
        .unwrap();
        fe2o3_artifact_transaction::finish_build_attempt(&directory.0, &producer, attempt).unwrap();
        store
            .persist_completed(attempt, intent_identity, published.receipt())
            .unwrap();
        let completed = store.load().unwrap().unwrap();
        clear_worker_v2_publication_intent_v1(&directory.0, &producer, attempt, intent_identity)
            .unwrap();
        store.clear_completed(completed).unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn ready_marker_rejects_mutated_retained_output() {
        let directory = TestDirectory::new();
        let producer = producer(60);
        let attempt = attempt(&directory.0, &producer, 60);
        let (output, plan, upstream) = publication_inputs(attempt, 60);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        store
            .persist_pending(
                attempt,
                restart_admission_commitment_v1(plan, upstream, &output),
            )
            .unwrap();
        let persisted = persist_worker_v2_publication_intent_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            &output,
        )
        .unwrap();
        store
            .persist_ready(attempt, persisted.record().identity())
            .unwrap();
        drop(persisted);
        drop(store);

        let retained_output = fs::read_dir(&directory.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                let name = path.file_name().unwrap().to_string_lossy();
                name.starts_with(".fe2o3-worker-v2-publication-intent-v1-")
                    && name.ends_with(".output")
            })
            .unwrap();
        let mut substituted = fs::read(&retained_output).unwrap();
        substituted[0] ^= 1;
        fs::write(&retained_output, substituted).unwrap();

        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        let state = store.load().unwrap().unwrap();
        assert!(matches!(
            recover_worker_v2_intent_v1(&store, &producer, state),
            Err(RestartIntentErrorV1::Intent(_))
        ));
        assert_eq!(store.load().unwrap(), Some(state));
    }

    #[test]
    fn pending_marker_rejects_substituted_journal_commitment() {
        let directory = TestDirectory::new();
        let producer = producer(70);
        let attempt = attempt(&directory.0, &producer, 70);
        let (output, plan, upstream) = publication_inputs(attempt, 70);
        let store = WorkerV2ResumeStoreV1::open(&directory.0, &producer).unwrap();
        store.persist_pending(attempt, [0xa5; 32]).unwrap();
        persist_worker_v2_publication_intent_v1(
            &directory.0,
            &producer,
            attempt,
            plan,
            upstream,
            &output,
        )
        .unwrap();

        let state = store.load().unwrap().unwrap();
        assert!(matches!(
            recover_worker_v2_intent_v1(&store, &producer, state),
            Err(RestartIntentErrorV1::IntentIdentityMismatch)
        ));
        assert_eq!(store.load().unwrap(), Some(state));
    }
}
