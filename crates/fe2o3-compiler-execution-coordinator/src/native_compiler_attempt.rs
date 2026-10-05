//! Original compiler custody behind an unopened exec gate. No resume API.
use super::*;
use crate::{
    compiler_child_channel::CompilerTrace,
    native_launch::{self as native, Channels, CompilerExecutionLaunchErrorV2 as NativeError},
    native_runtime_inventory::NativeCompilerExecutableInventory as Executables,
    proof_helper_backing::ProofHelperBackingError,
    proof_helper_launch::{ManagedProofHelper as Helper, ProofHelperLaunchError as Failure},
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1 as Credentials, observations,
};
use fe2o3_protected_service_spawn::{
    ProtectedServiceCleanupServiceV2 as Cleanup, ProtectedServiceDescriptorBindingV1 as Binding,
    cleanup_bridge::CleanupPollV1,
    launch_io,
    native_spawn::{
        RootOwnedRetainedServiceChildV2 as Child, StagedProtectedServiceExecV2 as Stage,
    },
};
use std::{mem::size_of_val, os::fd::BorrowedFd};

type AttemptResult<T> = std::result::Result<T, Failure>;
type Trace<'work> = CompilerTrace<'work, Helper>;
const INVOCATION_FD: i32 = fe2o3_compiler_closure_capability::RUSTC_INVOCATION_CHILD_FD_V1;
const BACKEND_FD: i32 = fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_CHILD_FD_V1;
const OUTPUT_FD: i32 = fe2o3_artifact_transaction::BROKERED_ARTIFACT_DIRECTORY_CHILD_FD_V1;
pub(super) const LOCAL_WORK: usize = 8 + 256 * 1088;
// The retained invocation already enforces these byte/count bounds. Pay the
// bounded comparison traversal before inspecting the staged argv/environment.
pub(super) const INPUT_COMPARE_WORK: usize =
    crate::compiler_invocation_staging::StagedRustcInvocationV1::STAGING_WORK;
pub(super) const FRAME: usize = 4 * size_of::<Attempt<'static>>()
    + 4 * size_of::<Stage>()
    + 8 * size_of::<fs::Stat>()
    + launch_io::ATTEMPT_SCRATCH
    + 8192;

pub(super) struct Attempt<'work> {
    // Foreground trace cancellation precedes stage/gate descriptor retirement.
    trace: Trace<'work>,
    executables: Executables,
    stage: Stage,
    _gate_reader: OwnedFd,
    _gate_writer: OwnedFd,
    _exec_reader: OwnedFd,
    retained: usize,
}

impl Attempt<'_> {
    const ENVELOPE: usize = size_of::<Self>()
        - size_of::<Trace<'static>>()
        - size_of::<Executables>()
        - size_of::<Stage>()
        - 3 * size_of::<OwnedFd>();

    pub(super) fn revalidate(&self, received: &Receiver, b: &mut Budget<'_>) -> AttemptResult<()> {
        b.with_prepaid_scope(self.retained, 8, LOCAL_WORK, FRAME, |b| {
            self.trace.with_backing(b, |helper, b| {
                helper.with_compiler(b, |compiler, b| {
                    self.executables.revalidate(compiler, b)?;
                    validate(compiler, received, &self.stage, b)
                })
            })
        })
    }

    pub(super) fn cancel(&mut self) -> CleanupPollV1 {
        // The original compiler slot retains Helper until aggregate retirement;
        // its Drop then cancels the helper in the SAME independently funded pool.
        self.trace.cancel()
    }

    #[cfg(test)]
    pub(super) fn pid(&self) -> rustix::process::Pid {
        self.trace.pid()
    }
}

/// Caller supplies the same dedicated creator and outside-domain custodian as
/// helper launch. The returned trace remains pre-exec: no gate writer is exposed,
/// and neither release_child, confirm_exec nor resume is called here.
#[allow(unsafe_code)]
pub(super) unsafe fn launch<'work>(
    helper: Helper,
    received: &Receiver,
    credentials: Credentials,
    deadline: Instant,
    cleanup: &mut Cleanup,
    b: &mut Budget<'work>,
) -> AttemptResult<(Attempt<'work>, usize)> {
    let input = helper.retained_storage();
    b.with_prepaid_scope(input, 8, LOCAL_WORK, FRAME, |b| {
        let executables = helper.with_compiler(b, |compiler, b| -> AttemptResult<_> {
            Ok(Executables::capture(compiler, b)?)
        })?;
        b.reserve_storage(executables.retained_storage())?;
        let channels = Channels::new()?;
        b.reserve_storage(Channels::STORAGE)?;
        let (stage, storage) =
            helper.with_compiler(b, |compiler, b| stage(compiler, received, &channels, b))?;
        b.reserve_storage(storage)?;
        require_deadline(deadline)?;
        // SAFETY: final stage derives exclusively from the retained compiler and
        // original received objects. The complete helper (including those objects)
        // enters the original pool BEFORE clone. The caller retains the dedicated
        // creator, outside custodian and exclusive wait/mutation contract.
        let (child, growth) = unsafe {
            stage.spawn_retaining_in_fresh_domain(credentials, helper, input, cleanup, b)
        }?;
        // child already owns cancellation before this or any later operation fails.
        b.reserve_storage(growth.additional_storage())?;
        launch_io::await_profile_ready(
            channels.profile_reader.as_fd(),
            channels.exec_reader.as_fd(),
            &mut Observer {
                child: &child,
                budget: b,
            },
            deadline,
        )?;
        b.with_prepaid_scope(
            child.retained_storage(),
            0,
            observations::PROCESS_VALIDATE_WORK,
            observations::PROCESS_VALIDATE_SCRATCH,
            |_| {
                observations::validate_process(credentials, child.pid())
                    .map_err(NativeError::Observation)
            },
        )?;
        #[cfg(test)]
        RootCompilerRequest::postclone_checkpoint_for_test("compiler-profile", child.pid(), b)?;
        child.with_resources(b, |helper, b| {
            helper.with_compiler(b, |compiler, b| validate(compiler, received, &stage, b))
        })?;
        let previous = native::sum(&[child.retained_storage(), native::FILE_STORAGE])?;
        let Channels {
            root,
            child: sender,
            exec_reader,
            exec_writer,
            profile_reader,
            profile_writer,
            gate_reader,
            gate_writer,
        } = channels;
        let (trace, full) = CompilerTrace::receive(child, root, credentials, deadline, b)?;
        b.reserve_storage(full.checked_sub(previous).ok_or(Resource::Accounting)?)?;
        #[cfg(test)]
        RootCompilerRequest::postclone_checkpoint_for_test("compiler-trace", trace.pid(), b)?;
        drop((sender, exec_writer, profile_reader, profile_writer));
        b.release_storage(4 * native::FILE_STORAGE)?;
        let retained = native::sum(&[
            full,
            executables.retained_storage(),
            storage,
            3 * native::FILE_STORAGE,
            Attempt::ENVELOPE,
        ])?;
        b.reserve_storage(Attempt::ENVELOPE)?;
        let attempt = Attempt {
            trace,
            executables,
            stage,
            _gate_reader: gate_reader,
            _gate_writer: gate_writer,
            _exec_reader: exec_reader,
            retained,
        };
        attempt.revalidate(received, b)?;
        require_deadline(deadline)?;
        Ok((
            attempt,
            retained.checked_sub(input).ok_or(Resource::Accounting)?,
        ))
    })
}

fn require_deadline(deadline: Instant) -> AttemptResult<()> {
    if Instant::now() >= deadline {
        return Err(Failure::Invalid("compiler pre-exec deadline exceeded"));
    }
    Ok(())
}

fn received_fd(received: &Receiver, role: Role) -> AttemptResult<BorrowedFd<'_>> {
    let index = received
        .challenge
        .as_ref()
        .and_then(|record| record.roles().position(|candidate| candidate == role))
        .ok_or(Failure::Invalid("missing received compiler role"))?;
    received.files[index]
        .as_ref()
        .map(AsFd::as_fd)
        .ok_or(Failure::Invalid("missing original received compiler FD"))
}

fn same_object(source: BorrowedFd<'_>, staged: &std::fs::File) -> AttemptResult<()> {
    let expected = fs::fstat(source).map_err(|e| native::io("inspect original compiler FD", e))?;
    let actual = fs::fstat(staged).map_err(|e| native::io("inspect staged compiler FD", e))?;
    let expected_flags =
        fs::fcntl_getfl(source).map_err(|e| native::io("inspect original FD flags", e))?;
    let actual_flags =
        fs::fcntl_getfl(staged).map_err(|e| native::io("inspect staged FD flags", e))?;
    if (
        expected.st_dev,
        expected.st_ino,
        expected.st_mode,
        expected_flags,
    ) != (actual.st_dev, actual.st_ino, actual.st_mode, actual_flags)
    {
        return Err(Failure::Invalid("staged compiler object or flags differ"));
    }
    Ok(())
}

fn validate(
    compiler: &Backing,
    received: &Receiver,
    stage: &Stage,
    b: &mut Budget<'_>,
) -> AttemptResult<()> {
    compiler
        .revalidate(b)
        .map_err(ProofHelperBackingError::Compiler)?;
    let invalid = || Failure::Invalid("incomplete original compiler stage");
    if !stage.has_runtime_checkpoints() {
        return Err(Failure::Invalid("compiler stage lacks runtime checkpoints"));
    }
    compiler
        .runtime()
        .validate_rustc_exec_transfer(stage.executable(), b)
        .map_err(ProofHelperBackingError::Runtime)?;
    compiler
        .runtime()
        .validate_codegen_backend_load_transfer(stage.binding(BACKEND_FD).ok_or_else(invalid)?, b)
        .map_err(ProofHelperBackingError::Runtime)?;
    b.charge_work(INPUT_COMPARE_WORK)?;
    if stage.compiler_arguments() != Some(compiler.invocation().arguments())
        || stage.compiler_environment() != Some(compiler.invocation().environment())
    {
        return Err(Failure::Invalid(
            "staged compiler bytes differ from original invocation",
        ));
    }
    same_object(
        received_fd(received, Role::WorkingDirectory)?,
        stage.compiler_working_directory().ok_or_else(invalid)?,
    )?;
    same_object(
        received_fd(received, Role::Invocation)?,
        stage.binding(INVOCATION_FD).ok_or_else(invalid)?,
    )?;
    same_object(
        received_fd(received, Role::OutputDirectory)?,
        stage.binding(OUTPUT_FD).ok_or_else(invalid)?,
    )?;
    for (index, role) in [Role::Stdin, Role::Stdout, Role::Stderr]
        .into_iter()
        .enumerate()
    {
        let selected = received
            .challenge
            .as_ref()
            .ok_or_else(invalid)?
            .stdio_mask()
            & (1 << index)
            != 0;
        match (selected, stage.binding(index as i32)) {
            (true, Some(file)) => {
                same_object(received_fd(received, role)?, file)?;
                crate::native_runtime_descriptors::inspect(file.as_fd(), b)?;
            }
            (false, None) => {}
            _ => return Err(invalid()),
        }
    }
    Ok(())
}

#[allow(unsafe_code)]
fn stage(
    compiler: &Backing,
    received: &Receiver,
    channels: &Channels,
    b: &mut Budget<'_>,
) -> AttemptResult<(Stage, usize)> {
    b.with_prepaid_scope(compiler.retained_storage(), 8, LOCAL_WORK, FRAME, |b| {
        compiler
            .revalidate(b)
            .map_err(ProofHelperBackingError::Compiler)?;
        let bindings = [
            Binding::new(received_fd(received, Role::Invocation)?, INVOCATION_FD)
                .map_err(|_| Failure::Invalid("invalid invocation binding"))?,
            Binding::new(compiler.codegen_backend_source().as_fd(), BACKEND_FD)
                .map_err(|_| Failure::Invalid("invalid backend binding"))?,
            compiler
                .output_binding(b)
                .map_err(ProofHelperBackingError::Compiler)?,
        ];
        let mask = received
            .challenge
            .as_ref()
            .ok_or(Failure::Invalid("missing original challenge"))?
            .stdio_mask();
        let mut stdio = [None; 3];
        for (index, role) in [Role::Stdin, Role::Stdout, Role::Stderr]
            .into_iter()
            .enumerate()
        {
            if mask & (1 << index) != 0 {
                stdio[index] = Some(received_fd(received, role)?);
            }
        }
        let bytes = |file: &std::fs::File| -> AttemptResult<usize> {
            let stat =
                fs::fstat(file).map_err(|e| native::io("measure retained compiler image", e))?;
            usize::try_from(stat.st_size).map_err(|_| Resource::Arithmetic.into())
        };
        let wire = usize::try_from(received.challenge.as_ref().unwrap().invocation_bytes())
            .map_err(|_| Resource::Arithmetic)?;
        let sources = native::sum(&[
            bytes(compiler.rustc_source())?,
            bytes(compiler.codegen_backend_source())?,
            wire,
            compiler.invocation().retained_storage(),
            Channels::STORAGE,
            10 * native::FILE_STORAGE,
            size_of_val(&bindings),
        ])?;
        b.reserve_storage(size_of_val(&bindings))?;
        // SAFETY: borrowed bytes/objects are the original capture and approved
        // sources, fully prepaid through the helper. No cwd pathname is reopened.
        let (stage, charge) = unsafe {
            Stage::stage_compiler_with_child_channel(
                compiler.rustc_source(),
                compiler.invocation().arguments(),
                compiler.invocation().environment(),
                received_fd(received, Role::WorkingDirectory)?,
                stdio,
                &bindings,
                channels.profile_writer.as_fd(),
                channels.gate_reader.as_fd(),
                channels.exec_writer.as_fd(),
                channels.child.as_fd(),
                sources,
                b,
            )
        }?;
        b.reserve_storage(charge.additional_storage())?;
        // This only selects child setup after the still-closed gate. The gate
        // remains inaccessible until the original trace has a complete guard.
        let stage = stage.require_runtime_checkpoints(b)?;
        same_object(
            channels.child.as_fd(),
            stage
                .compiler_child_channel_transfer()
                .ok_or(Failure::Invalid("missing original channel transfer"))?,
        )?;
        validate(compiler, received, &stage, b)?;
        Ok((stage, charge.additional_storage()))
    })
}

struct Observer<'a, 'work> {
    child: &'a Child<Helper>,
    budget: &'a mut Budget<'work>,
}
impl launch_io::Observer for Observer<'_, '_> {
    type Error = Failure;
    fn before_attempt(&mut self, boundary: launch_io::Boundary) -> AttemptResult<()> {
        Ok(self.budget.charge_work(boundary.work())?)
    }
    fn is_live(&mut self) -> AttemptResult<bool> {
        Ok(self.child.is_live(self.budget)?)
    }
}

#[cfg(test)]
#[path = "native_compiler_attempt_tests.rs"]
mod tests;
