//! Explicit native V5/V3 application route; never retries through a legacy envelope.
use super::*;
use fe2o3_artifact_transaction::{
    DurablePublishedHsacoClaimV3 as Claim, NativeCurrentPublicationLimitsV1 as Limits,
    NativeCurrentPublicationV1 as Publication, RetainedDurableDirectoryV1 as Directory,
    WorkerV3PublicationIntentRecordV1 as Record,
};
use fe2o3_compiler_closure_capability::ProductionCompilerExecutionDeploymentV3 as Deployment;
use fe2o3_compiler_execution_client::{
    PendingCompilerExecutionChildChannelV1 as Channel, RetainedApplicationServiceLaunchV1 as Launch,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionReceiptCarriageV3 as Carriage,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
    CompilerExecutionSupervisorHandoffV3 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_runtime_protocol::{
    InertConditionalWorkerReadinessWireV5 as Wire,
    MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 as MAX_READINESS,
    NativeApplicationRegistrationBindingV1 as Binding,
    NativeApplicationRegistrationInputsV1 as Inputs,
    NativeConditionalApplicationBindingV1 as Association,
};
use std::{
    ffi::{OsStr, OsString},
    mem::size_of,
    os::fd::OwnedFd,
};

pub(crate) const NATIVE_HANDOFF_ENV: &str = "FE2O3_NATIVE_CONDITIONAL_APPLICATION_V1";
const ADMISSION_WORK: usize = 16 * 1024 * 1024 * 1024;
const ADMISSION_STORAGE: usize = 16 * 1024 * 1024 * 1024;

fn error(value: impl std::fmt::Display) -> String {
    value.to_string()
}

pub(crate) struct Pinned<'directory, 'work> {
    directory: &'directory PinnedDirectory,
    name: String,
    envelope: File,
    snapshot: FileSnapshot,
    inherited_directory: OwnedFd,
    admitted_directory: Directory,
    publication: Publication<'work>,
    carriage: Carriage,
    retained: usize,
}

impl<'directory, 'work> Pinned<'directory, 'work> {
    pub(crate) fn discover(
        directory: &'directory PinnedDirectory,
        budget: &mut Budget<'work>,
    ) -> Result<Self, String> {
        directory.validate_path("native Cargo application artifact directory")?;
        // Closed scan bound includes all visible entries and every retained
        // candidate name. It stays charged on refusal or while candidates live.
        budget
            .charge_work(MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1 * 4096)
            .map_err(error)?;
        budget
            .reserve_storage(MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1 * 512)
            .map_err(error)?;
        let names = envelope_names(directory)?;
        reject_retired_worker_v2_envelopes(&names)?;
        let mut selected = None;
        for name in names {
            // A candidate is admitted only if its exact V5 bytes name the current
            // durable publication. No older schema is decoded as a fallback.
            let candidate = Self::open(directory, name, budget)?;
            if selected.replace(candidate).is_some() {
                return Err("multiple current native application readiness files".into());
            }
        }
        selected.ok_or_else(|| "native custodian run requires exact current V5 readiness".into())
    }

    fn open(
        directory: &'directory PinnedDirectory,
        name: String,
        budget: &mut Budget<'work>,
    ) -> Result<Self, String> {
        let floor = budget.storage();
        budget.charge_work(4096).map_err(error)?;
        budget
            .reserve_storage(128 * 1024 + name.len())
            .map_err(error)?;
        if !is_canonical_v3_envelope_name(name.as_bytes()) {
            return Err("native application readiness has an invalid filename".into());
        }
        let envelope = File::from(
            openat2(
                directory.file(),
                Path::new(&name),
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
                ResolveFlags::BENEATH
                    | ResolveFlags::NO_SYMLINKS
                    | ResolveFlags::NO_MAGICLINKS
                    | ResolveFlags::NO_XDEV,
            )
            .map_err(error)?,
        );
        let stat = fstat(&envelope).map_err(error)?;
        validate_envelope_stat(directory, &name, &stat)?;
        let snapshot = FileSnapshot::from_stat(&stat);
        let length = usize::try_from(stat.st_size).map_err(error)?;
        if length == 0 || length > MAX_READINESS {
            return Err("native readiness exceeds its closed byte bound".into());
        }
        let bytes = read_exact_finite(&envelope, length, budget)?;
        if FileSnapshot::from_stat(&fstat(&envelope).map_err(error)?) != snapshot {
            return Err("native readiness changed during finite read".into());
        }
        let wire = Wire::decode(&bytes, MAX_READINESS).map_err(error)?;
        let record = Record::decode_canonical(wire.record_bytes()).map_err(error)?;
        let claim = Claim::decode_canonical(wire.claim_bytes()).map_err(error)?;
        if record.plan() != claim.plan() {
            return Err("native readiness record and claim differ".into());
        }
        let inherited_directory = directory.try_clone_for_transfer()?.into();
        let admitted_directory =
            Directory::admit_service_owned(directory.try_clone_for_transfer()?.into())
                .map_err(error)?;
        budget
            .reserve_storage(Publication::DIRECTORY_STORAGE)
            .map_err(error)?;
        let (publication, charge) = Publication::recover(
            &admitted_directory,
            record.attempt(),
            Limits::new(length, record.output_length()).map_err(error)?,
            budget,
        )
        .map_err(error)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(error)?;
        if publication.readiness().exact_envelope_bytes() != bytes
            || publication.readiness().published_claim() != &claim
        {
            return Err("native readiness is not the exact retained current publication".into());
        }
        let (carriage, charge) =
            Carriage::decode_in_original_account_v3(wire.compiler_execution_bytes(), budget)
                .map_err(error)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(error)?;
        drop(bytes);
        budget.release_storage(length).map_err(error)?;
        let value = Self {
            directory,
            name,
            envelope,
            snapshot,
            inherited_directory,
            admitted_directory,
            publication,
            carriage,
            retained: budget.storage() - floor,
        };
        value.revalidate(budget)?;
        Ok(value)
    }

    pub(crate) fn revalidate(&self, budget: &mut Budget<'work>) -> Result<(), String> {
        if budget.storage() < self.retained {
            return Err("unpaid native Cargo publication".into());
        }
        budget.charge_work(4096).map_err(error)?;
        self.directory
            .validate_path("native Cargo application artifact directory")?;
        let stat = fstat(&self.envelope).map_err(error)?;
        validate_envelope_stat(self.directory, &self.name, &stat)?;
        if FileSnapshot::from_stat(&stat) != self.snapshot {
            return Err("native readiness descriptor changed".into());
        }
        if !self
            .admitted_directory
            .matches_descriptor(&self.inherited_directory)
            .map_err(error)?
        {
            return Err("native inherited directory changed".into());
        }
        self.publication.revalidate(budget).map_err(error)?;
        let length = self.publication.readiness().exact_envelope_bytes().len();
        let bytes = read_exact_finite(&self.envelope, length, budget)?;
        if bytes != self.publication.readiness().exact_envelope_bytes()
            || FileSnapshot::from_stat(&fstat(&self.envelope).map_err(error)?) != self.snapshot
        {
            return Err("native exact readiness bytes changed".into());
        }
        drop(bytes);
        budget.release_storage(length).map_err(error)
    }
    pub(crate) fn exact_readiness(&self) -> &[u8] {
        self.publication.readiness().exact_envelope_bytes()
    }
    pub(crate) fn carriage(&self) -> &Carriage {
        &self.carriage
    }
}

fn read_exact_finite(
    file: &File,
    length: usize,
    budget: &mut Budget<'_>,
) -> Result<Vec<u8>, String> {
    budget
        .charge_work(
            length
                .checked_add(4096)
                .ok_or("native read work overflow")?,
        )
        .map_err(error)?;
    budget.reserve_storage(length).map_err(error)?;
    let mut bytes = vec![0; length];
    for (index, chunk) in bytes.chunks_mut(64 * 1024).enumerate() {
        let offset = u64::try_from(index * 64 * 1024).map_err(error)?;
        if rustix::io::pread(file, &mut *chunk, offset).map_err(error)? != chunk.len() {
            return Err("native readiness finite read was short".into());
        }
    }
    if rustix::io::pread(file, &mut [0; 1], length as u64).map_err(error)? != 0 {
        return Err("native readiness grew during finite read".into());
    }
    Ok(bytes)
}

fn nonce() -> Result<[u8; 32], String> {
    let mut bytes = [0; 32];
    // SAFETY: fixed writable output lives through this single getrandom attempt.
    let count =
        unsafe { libc::getrandom(bytes.as_mut_ptr().cast(), bytes.len(), libc::GRND_NONBLOCK) };
    if count != bytes.len() as isize || bytes == [0; 32] {
        return Err("native startup entropy unavailable in one attempt".into());
    }
    Ok(bytes)
}

pub(crate) fn run(
    directory: &PinnedDirectory,
    application: &OsStr,
    args: &[OsString],
) -> Result<ExitStatus, String> {
    let mut account = Account::new(Work::new(ADMISSION_WORK), ADMISSION_STORAGE);
    account.with_budget(|budget| run_in_account(directory, application, args, budget))
}

fn run_in_account<'work>(
    directory: &PinnedDirectory,
    application: &OsStr,
    args: &[OsString],
    budget: &mut Budget<'work>,
) -> Result<ExitStatus, String> {
    let (deployment, charge) = Deployment::open(budget).map_err(error)?;
    budget
        .reserve_storage(charge.additional_storage())
        .map_err(error)?;
    let pinned = Pinned::discover(directory, budget)?;
    if pinned.carriage.policy().canonical_bytes() != deployment.policy().canonical_bytes() {
        return Err("native application carriage differs from installed V3 policy".into());
    }
    let cwd = std::env::current_dir().map_err(error)?;
    let path =
        crate::binding_wrapper::resolve_command_executable(application, &cwd).map_err(error)?;
    let executable = crate::pinned_executable::PinnedExecutable::open(&path).map_err(error)?;
    let sealed = executable.seal_static_application().map_err(error)?;
    let mut command = sealed.command().map_err(error)?;
    command.args(args);
    crate::scrub_application_environment(command.as_command_mut());
    let channel = Channel::prepare(command.as_command_mut()).map_err(error)?;
    let reaper = application_reaper().reserve()?;
    ensure_child_subreaper()?;
    let (reader, writer) = cloexec_pipe()?;
    let proof = PreparedApplicationProofChannelV1::prepare().map_err(error)?;
    let setup = proof.child_setup();
    let fds = [
        pinned.envelope.as_raw_fd(),
        pinned.inherited_directory.as_raw_fd(),
        writer.as_raw_fd(),
        setup.descriptor(),
    ];
    let snapshots = [
        fstat(&pinned.envelope).map_err(error)?,
        fstat(&pinned.inherited_directory).map_err(error)?,
        fstat(&writer).map_err(error)?,
    ];
    let proof_identity = setup.descriptor_identity();
    let occurrences = [
        descriptor_occurrence(1, &snapshots[0])?,
        descriptor_occurrence(2, &snapshots[1])?,
        descriptor_occurrence(3, &snapshots[2])?,
        WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
            4,
            proof_identity.0,
            proof_identity.1,
            proof_identity.2,
        )
        .map_err(error)?,
    ];
    budget
        .reserve_storage(
            128 * 1024
                + size_of::<WorkerV3ApplicationOccurrenceV1>()
                + 4 * size_of::<WorkerV3ApplicationInputOccurrenceV1>(),
        )
        .map_err(error)?;
    let occurrence =
        WorkerV3ApplicationOccurrenceV1::new(sealed.identity_v3(), nonce()?, &occurrences)
            .map_err(error)?;
    let challenge = WorkerV3ApplicationHandoffChallengeV1::from_bytes(nonce()?).map_err(error)?;
    command
        .as_command_mut()
        .env(NATIVE_HANDOFF_ENV, "1")
        .env(WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1, fds[0].to_string())
        .env(
            WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
            fds[1].to_string(),
        )
        .env(
            WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
            fds[2].to_string(),
        )
        .env(WORKER_V3_APPLICATION_PROOF_FD_ENV_V1, fds[3].to_string())
        .env(
            WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
            encode_lower_hex(&occurrence.encode_canonical().map_err(error)?),
        )
        .env(
            WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
            encode_lower_hex(&challenge.encode_canonical().map_err(error)?),
        );
    let descriptors =
        WorkerV3ApplicationRegistrationDescriptorsV1::new(fds[0], fds[1], fds[2], fds[3])
            .map_err(error)?;
    let (inputs, charge) = Inputs::new(
        pinned.publication.readiness().exact_envelope_bytes(),
        occurrence,
        descriptors,
        challenge,
        budget,
    )
    .map_err(error)?;
    budget
        .reserve_storage(charge.additional_storage())
        .map_err(error)?;
    let sandbox = PendingApplicationSandbox::start()?;
    let socket = sandbox.child_socket_fd();
    let filter = no_fork_application_filter();
    let snapshots = snapshots.map(|stat| FileSnapshot::from_stat(&stat));
    // SAFETY: all original descriptors stay owned until spawn completes. This
    // callback uses only the existing async-signal-safe descriptor/sandbox path.
    unsafe {
        command.as_command_mut().pre_exec(move || {
            establish_fresh_application_session()?;
            crate::application_exec::protect_all_nonstdio_descriptors()?;
            for (index, fd) in fds[..3].iter().enumerate() {
                let borrowed = BorrowedFd::borrow_raw(*fd);
                if FileSnapshot::from_stat(&fstat(borrowed).map_err(io::Error::from)?)
                    != snapshots[index]
                    || !rustix::io::fcntl_getfd(borrowed)
                        .map_err(io::Error::from)?
                        .contains(rustix::io::FdFlags::CLOEXEC)
                {
                    return Err(io::Error::from_raw_os_error(libc::ESTALE));
                }
                crate::application_exec::expose_descriptor(*fd)?;
            }
            crate::application_exec::validate_and_expose_connected_seqpacket_descriptor(
                COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
            )?;
            setup.expose_before_exec()?;
            install_application_profile(&filter, socket)
        });
    }
    pinned.revalidate(budget)?;
    deployment.revalidate(budget).map_err(error)?;
    let process = crate::process_execution::spawn(command.as_command_mut()).map_err(error)?;
    drop(writer);
    let proof = proof.after_spawn();
    let mut cleanup = ApplicationCleanup {
        reaper,
        sandbox: None,
        application: None,
        native_child: None,
        timeout: Duration::from_secs(2),
        #[cfg(test)]
        test_hold: None,
    };
    let result = (|| {
        let captured = RetainedCompilerExecutionChildV1::capture(&process).map_err(error);
        match sandbox.complete(process.id()) {
            Ok(guard) => cleanup.sandbox = Some(guard),
            Err(failure) => {
                let (message, guard) = failure.into_parts();
                cleanup.sandbox = Some(guard);
                return Err(message);
            }
        }
        cleanup.native_child = Some(captured?);
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(120))
            .ok_or("native application startup deadline overflow")?;
        let child = cleanup
            .native_child
            .as_ref()
            .ok_or("native child custody missing")?;
        budget.charge_work(64 * 1024).map_err(error)?;
        let launch = channel
            .finish_native_application_until(child, deadline)
            .map_err(error)?;
        budget
            .reserve_storage(Launch::NATIVE_APPLICATION_INPUT_STORAGE)
            .map_err(error)?;
        let (manifest, charge) = Manifest::new(
            launch.client(),
            deployment.profile().external_anchor_service(),
            deployment.policy(),
            budget,
        )
        .map_err(error)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(error)?;
        let (handoff, charge) =
            Handoff::new(launch.submitter(), manifest, budget).map_err(error)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(error)?;
        let association = Association::bind(
            pinned.publication.readiness().exact_envelope_bytes(),
            &handoff,
            &pinned.carriage,
            budget,
        )
        .map_err(error)?;
        budget
            .reserve_storage(size_of::<Association>())
            .map_err(error)?;
        let (binding, charge) =
            Binding::new(handoff, association, inputs, budget).map_err(error)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(error)?;
        let (ready, charge) = launch
            .handoff_native_application_until(
                proof,
                OwnedFd::from(reader),
                binding,
                deployment.profile(),
                deadline,
                budget,
            )
            .map_err(error)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(error)?;
        child.validate_custody().map_err(error)?;
        deployment.revalidate(budget).map_err(error)?;
        pinned.revalidate(budget)?;
        wait_for_application_exit_without_reaping(&process)?;
        deployment.revalidate(budget).map_err(error)?;
        pinned.revalidate(budget)?;
        drop(ready);
        Ok(())
    })();
    match result {
        Ok(()) => wait_and_contain_application_group(process, cleanup),
        Err(message) => crate::terminate_application_with_error(process, cleanup, message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_readiness_read_exact_quota_and_terminal_length_refusals() {
        let dir = crate::pinned_executable_test_directory::TestDirectory::new();
        let path = dir.path().join("readiness");
        let content = vec![19; 64 * 1024 + 17];
        std::fs::write(&path, &content).unwrap();
        let file = File::open(path).unwrap();
        for (work, storage, length, success) in [
            (content.len() + 4096, content.len(), content.len(), true),
            (content.len() + 4095, content.len(), content.len(), false),
            (
                content.len() + 4096,
                content.len() - 1,
                content.len(),
                false,
            ),
            (
                content.len() + 4096,
                content.len(),
                content.len() - 1,
                false,
            ),
            (
                content.len() + 4097,
                content.len() + 1,
                content.len() + 1,
                false,
            ),
        ] {
            let mut account = Account::new(Work::new(work), storage);
            account.with_budget(|budget| {
                let ledger = budget.work_ledger_identity_v1();
                let account = budget.storage_account_identity_v1();
                let result = read_exact_finite(&file, length, budget);
                assert_eq!(result.is_ok(), success);
                if success {
                    assert_eq!(result.unwrap(), content);
                }
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(budget.storage_account_identity_v1(), account);
            });
        }
    }
}
