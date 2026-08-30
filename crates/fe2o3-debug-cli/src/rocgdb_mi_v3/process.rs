use super::*;

use std::collections::{BTreeSet, VecDeque};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::rocgdb_mi_parser_v3::{MiListV3, MiRecordV3, MiValueV3, parse_mi_record_v3};

const MAX_COMMAND_BYTES_V3: usize = 64 * 1024;

pub(crate) struct RocgdbMiNativeSpawnProvisionV4<'a> {
    pub(crate) target_endpoint: &'a OwnedFd,
    pub(crate) nonce: fe2o3_kfd::KfdTargetDebugSessionNonceV1,
    pub(crate) debugger_pid: u32,
}

enum ReaderItemV3 {
    Line(Vec<u8>),
    TooLarge,
    Io,
    Eof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InferiorOwnershipV3 {
    Unknown,
    LaunchOwned,
    AttachBorrowed,
}

/// Exact-argv ROCgdb subprocess with one outstanding bounded MI command.
pub struct RocgdbMiProcessV3 {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: Receiver<ReaderItemV3>,
    next_token: u64,
    pending_token: Option<u64>,
    last_response_evidence: Vec<u8>,
    defer_observations: bool,
    deferred_records: VecDeque<(MiRecordV3, Vec<u8>)>,
    adapter: RocgdbMiObservationAdapterV3,
    pending_events: VecDeque<RocgdbMiExecutionEventV3>,
    inferior_process: Option<OwnedFd>,
    inferior_pid: Option<u32>,
    inferior_ownership: InferiorOwnershipV3,
}

impl fmt::Debug for RocgdbMiProcessV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RocgdbMiProcessV3")
            .field("adapter", &self.adapter)
            .field("next_token", &self.next_token)
            .field("pending", &self.pending_token.is_some())
            .field("pending_events", &self.pending_events.len())
            .field("deferred_records", &self.deferred_records.len())
            .field("process_authority", &"REDACTED")
            .field("descriptor_authority", &"REDACTED")
            .field("inferior_process_authority", &"REDACTED")
            .field("inferior_ownership", &self.inferior_ownership)
            .finish()
    }
}

impl RocgdbMiProcessV3 {
    pub fn spawn(
        rocgdb: &Path,
        session_identity: OpaqueIdentityV1,
        authorization_identity: OpaqueIdentityV1,
        wave_width: u16,
        limits: RocgdbMiAdapterLimitsV3,
    ) -> Result<Self, RocgdbMiAdapterErrorV3> {
        limits.validate()?;
        let mut child = Command::new(rocgdb)
            .args(["--interpreter=mi3", "--nx", "--quiet"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProcessSpawn)?;
        let input = child
            .stdin
            .take()
            .ok_or(RocgdbMiAdapterErrorV3::ProcessSpawn)?;
        let output = child
            .stdout
            .take()
            .ok_or(RocgdbMiAdapterErrorV3::ProcessSpawn)?;
        let (sender, receiver) = mpsc::sync_channel(64);
        thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let item = read_bounded_line(&mut reader, limits.max_line_bytes);
                let terminal = !matches!(item, ReaderItemV3::Line(_));
                if sender.send(item).is_err() || terminal {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input: BufWriter::new(input),
            output: receiver,
            next_token: 1,
            pending_token: None,
            last_response_evidence: Vec::new(),
            defer_observations: false,
            deferred_records: VecDeque::new(),
            adapter: RocgdbMiObservationAdapterV3::new(
                session_identity,
                authorization_identity,
                wave_width,
                limits,
            )?,
            pending_events: VecDeque::new(),
            inferior_process: None,
            inferior_pid: None,
            inferior_ownership: InferiorOwnershipV3::Unknown,
        })
    }

    /// V4-only launcher substrate that provisions one inherited cooperative
    /// telemetry descriptor without changing the V3 spawn path.
    pub(crate) fn spawn_native_v4(
        rocgdb: &Path,
        session_identity: OpaqueIdentityV1,
        authorization_identity: OpaqueIdentityV1,
        wave_width: u16,
        limits: RocgdbMiAdapterLimitsV3,
        provision: RocgdbMiNativeSpawnProvisionV4<'_>,
    ) -> Result<Self, RocgdbMiAdapterErrorV3> {
        limits.validate()?;
        let inherited = provision.target_endpoint.as_raw_fd();
        let mut command = Command::new(rocgdb);
        command
            .args(["--interpreter=mi3", "--nx", "--quiet"])
            .env(
                fe2o3_kfd::KFD_TARGET_DEBUG_TELEMETRY_FD_ENV_V2,
                inherited.to_string(),
            )
            .env(
                fe2o3_kfd::KFD_TARGET_DEBUG_TELEMETRY_NONCE_ENV_V2,
                lower_hex_v4(provision.nonce.as_bytes()),
            )
            .env(
                fe2o3_kfd::KFD_TARGET_DEBUG_TELEMETRY_DEBUGGER_PID_ENV_V2,
                provision.debugger_pid.to_string(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // SAFETY: the hook performs two scalar fcntl operations. The retained
        // descriptor is provisioned only into ROCgdb and its launched target.
        unsafe {
            command.pre_exec(move || {
                let flags = libc::fcntl(inherited, libc::F_GETFD);
                if flags < 0 || libc::fcntl(inherited, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command
            .spawn()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProcessSpawn)?;
        let input = child
            .stdin
            .take()
            .ok_or(RocgdbMiAdapterErrorV3::ProcessSpawn)?;
        let output = child
            .stdout
            .take()
            .ok_or(RocgdbMiAdapterErrorV3::ProcessSpawn)?;
        let (sender, receiver) = mpsc::sync_channel(64);
        thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let item = read_bounded_line(&mut reader, limits.max_line_bytes);
                let terminal = !matches!(item, ReaderItemV3::Line(_));
                if sender.send(item).is_err() || terminal {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input: BufWriter::new(input),
            output: receiver,
            next_token: 1,
            pending_token: None,
            last_response_evidence: Vec::new(),
            defer_observations: false,
            deferred_records: VecDeque::new(),
            adapter: RocgdbMiObservationAdapterV3::new(
                session_identity,
                authorization_identity,
                wave_width,
                limits,
            )?,
            pending_events: VecDeque::new(),
            inferior_process: None,
            inferior_pid: None,
            inferior_ownership: InferiorOwnershipV3::Unknown,
        })
    }

    pub(crate) fn native_v4_commands_available(
        &mut self,
        timeout: Duration,
    ) -> Result<bool, RocgdbMiAdapterErrorV3> {
        let deadline = deadline(timeout)?;
        for command in [
            "-agent-info",
            "-queue-info",
            "-dispatch-info",
            "-thread-info",
            "-lane-info",
        ] {
            let mut request = b"-info-gdb-mi-command ".to_vec();
            request.extend_from_slice(command.as_bytes());
            let (class, result) = self.send_command(&request, deadline)?;
            let present = class == "done"
                && result
                    .get("command")
                    .and_then(MiValueV3::as_tuple)
                    .and_then(|value| optional_const(value, "exists"))
                    == Some(b"true");
            if !present {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(crate) fn launch_native_v4(
        &mut self,
        target: &Path,
        arguments: &[OsString],
        kernel_breakpoint: &[u8],
        timeout: Duration,
    ) -> Result<u32, RocgdbMiAdapterErrorV3> {
        if kernel_breakpoint.is_empty()
            || kernel_breakpoint.len() > 4_096
            || kernel_breakpoint.contains(&b'\n')
            || kernel_breakpoint.contains(&b'\r')
        {
            return Err(RocgdbMiAdapterErrorV3::InvalidField("kernel breakpoint"));
        }
        self.configure_launch(target, arguments, timeout)?;
        let mut breakpoint = b"-break-insert -f ".to_vec();
        append_quoted(&mut breakpoint, kernel_breakpoint)?;
        expect_class(self.send_command(&breakpoint, deadline(timeout)?)?, "done")?;
        self.inferior_ownership = InferiorOwnershipV3::LaunchOwned;
        self.launch(timeout)?;
        self.adapter.bump_revision()?;
        self.adapter.state = ExecutionStateV3::Running;
        let deadline = deadline(timeout)?;
        while self.inferior_pid.is_none() {
            let line = self.receive_line(deadline)?;
            let record = parse_mi_record_v3(&line, self.adapter.limits.parser())?;
            self.observe_process_authority(&record)?;
            self.deferred_records.push_back((record, line));
        }
        Ok(self.inferior_pid.expect("loop establishes inferior PID"))
    }

    pub const fn adapter(&self) -> &RocgdbMiObservationAdapterV3 {
        &self.adapter
    }

    pub fn adapter_mut(&mut self) -> &mut RocgdbMiObservationAdapterV3 {
        &mut self.adapter
    }

    pub fn discover_capabilities(
        &mut self,
        timeout: Duration,
    ) -> Result<RocgdbMiCapabilitiesV3, RocgdbMiAdapterErrorV3> {
        let deadline = deadline(timeout)?;
        let asynchronous = self
            .send_command(b"-gdb-set mi-async on", deadline)
            .is_ok_and(|(class, _)| class == "done");
        let (class, feature_results) = self.send_command(b"-list-features", deadline)?;
        if class != "done" {
            return Err(RocgdbMiAdapterErrorV3::BackendRejected);
        }
        let features = const_list(feature_results.get("features"))?;
        let command_names = [
            "-file-exec-and-symbols",
            "-exec-arguments",
            "-exec-run",
            "-target-attach",
            "-thread-info",
            "-stack-info-frame",
            "-data-list-register-names",
            "-data-list-register-values",
            "-stack-list-variables",
            "-data-read-memory-bytes",
            "-break-insert",
            "-break-delete",
            "-exec-continue",
            "-exec-interrupt",
            "-exec-step",
            "-exec-next",
            "-exec-finish",
            "-exec-step-instruction",
        ];
        let mut commands = BTreeSet::new();
        for command in command_names {
            let mut request = b"-info-gdb-mi-command ".to_vec();
            request.extend_from_slice(command.as_bytes());
            let (class, result) = self.send_command(&request, deadline)?;
            if class == "done"
                && result
                    .get("command")
                    .and_then(MiValueV3::as_tuple)
                    .and_then(|value| optional_const(value, "exists"))
                    == Some(b"true")
            {
                commands.insert(command);
            }
        }
        let capability = |name, present, unavailable_reason, authorization| RocgdbMiCapabilityV3 {
            name,
            availability: if present {
                LiveGpuCapabilityAvailabilityV3::Available
            } else {
                LiveGpuCapabilityAvailabilityV3::Unavailable
            },
            unavailable_reason: (!present).then_some(unavailable_reason),
            authorization,
        };
        let command_capability = |name, present, authorization| {
            capability(
                name,
                present,
                LiveGpuUnavailableReasonV3::Unsupported,
                authorization,
            )
        };
        let no_auth = RocgdbMiAuthorizationRequirementV3::NotRequired;
        let auth = RocgdbMiAuthorizationRequirementV3::Required;
        let thread_info = features.iter().any(|value| value == b"thread-info")
            && commands.contains("-thread-info");
        let stack = commands.contains("-stack-info-frame");
        let registers = commands.contains("-data-list-register-names")
            && commands.contains("-data-list-register-values");
        let values = commands.contains("-stack-list-variables");
        let memory = features
            .iter()
            .any(|value| value == b"data-read-memory-bytes")
            && commands.contains("-data-read-memory-bytes");
        let breakpoints = features.iter().any(|value| value == b"pending-breakpoints")
            && commands.contains("-break-insert")
            && commands.contains("-break-delete");
        let continue_control = commands.contains("-exec-continue");
        let pause = commands.contains("-exec-interrupt");
        let step = [
            "-exec-step",
            "-exec-next",
            "-exec-finish",
            "-exec-step-instruction",
        ]
        .into_iter()
        .all(|name| commands.contains(name));
        let gpu_admitted = !self.adapter.authenticated_gpu_threads.is_empty();
        let admitted_capability = |name, command_present, bound, reason, authorization| {
            capability(
                name,
                command_present && gpu_admitted && bound,
                if command_present {
                    reason
                } else {
                    LiveGpuUnavailableReasonV3::Unsupported
                },
                authorization,
            )
        };
        let capabilities = RocgdbMiCapabilitiesV3 {
            capabilities: vec![
                command_capability(
                    RocgdbMiCapabilityNameV3::Launch,
                    commands.contains("-file-exec-and-symbols")
                        && commands.contains("-exec-arguments")
                        && commands.contains("-exec-run"),
                    auth,
                ),
                command_capability(
                    RocgdbMiCapabilityNameV3::Attach,
                    commands.contains("-target-attach"),
                    auth,
                ),
                command_capability(
                    RocgdbMiCapabilityNameV3::AsyncExecution,
                    asynchronous,
                    no_auth,
                ),
                command_capability(
                    RocgdbMiCapabilityNameV3::StructuredThreads,
                    thread_info,
                    no_auth,
                ),
                admitted_capability(
                    RocgdbMiCapabilityNameV3::StoppedWave,
                    thread_info,
                    true,
                    LiveGpuUnavailableReasonV3::Unsupported,
                    no_auth,
                ),
                admitted_capability(
                    RocgdbMiCapabilityNameV3::LogicalLanes,
                    thread_info,
                    true,
                    LiveGpuUnavailableReasonV3::Unsupported,
                    no_auth,
                ),
                admitted_capability(
                    RocgdbMiCapabilityNameV3::RelativeProgramCounter,
                    stack,
                    !self.adapter.code_objects.is_empty(),
                    LiveGpuUnavailableReasonV3::Unsupported,
                    no_auth,
                ),
                admitted_capability(
                    RocgdbMiCapabilityNameV3::SourceSite,
                    stack,
                    !self.adapter.sources.is_empty(),
                    LiveGpuUnavailableReasonV3::Unsupported,
                    no_auth,
                ),
                admitted_capability(
                    RocgdbMiCapabilityNameV3::RegisterValues,
                    registers,
                    true,
                    LiveGpuUnavailableReasonV3::Unsupported,
                    no_auth,
                ),
                admitted_capability(
                    RocgdbMiCapabilityNameV3::SemanticValues,
                    values,
                    true,
                    LiveGpuUnavailableReasonV3::Unsupported,
                    no_auth,
                ),
                admitted_capability(
                    RocgdbMiCapabilityNameV3::AllocationRelativeMemory,
                    memory,
                    !self.adapter.allocations.is_empty(),
                    LiveGpuUnavailableReasonV3::Unsupported,
                    no_auth,
                ),
                command_capability(RocgdbMiCapabilityNameV3::Breakpoints, breakpoints, auth),
                command_capability(RocgdbMiCapabilityNameV3::Continue, continue_control, auth),
                command_capability(RocgdbMiCapabilityNameV3::Pause, pause, auth),
                command_capability(RocgdbMiCapabilityNameV3::Step, step, auth),
            ],
        };
        capabilities
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        Ok(capabilities)
    }

    fn configure_launch(
        &mut self,
        target: &Path,
        arguments: &[OsString],
        timeout: Duration,
    ) -> Result<(), RocgdbMiAdapterErrorV3> {
        let deadline = deadline(timeout)?;
        expect_class(
            self.send_command(b"-gdb-set mi-async on", deadline)?,
            "done",
        )?;
        let mut executable = b"-file-exec-and-symbols ".to_vec();
        append_quoted(&mut executable, target.as_os_str().as_bytes())?;
        expect_class(self.send_command(&executable, deadline)?, "done")?;
        let mut command = b"-exec-arguments".to_vec();
        for argument in arguments {
            command.push(b' ');
            append_quoted(&mut command, argument.as_os_str().as_bytes())?;
        }
        expect_class(self.send_command(&command, deadline)?, "done")
    }

    fn launch(&mut self, timeout: Duration) -> Result<(), RocgdbMiAdapterErrorV3> {
        let deadline = deadline(timeout)?;
        let (class, _) = self.send_control_command(b"-exec-run", deadline)?;
        if !matches!(class.as_str(), "running" | "done") {
            return Err(RocgdbMiAdapterErrorV3::BackendRejected);
        }
        Ok(())
    }

    fn attach(&mut self, process: u32, timeout: Duration) -> Result<(), RocgdbMiAdapterErrorV3> {
        if process == 0 || i32::try_from(process).is_err() {
            return Err(RocgdbMiAdapterErrorV3::InvalidField("process"));
        }
        let deadline = deadline(timeout)?;
        expect_class(
            self.send_command(b"-gdb-set mi-async on", deadline)?,
            "done",
        )?;
        let command = format!("-target-attach {process}");
        expect_class(
            self.send_control_command(command.as_bytes(), deadline)?,
            "done",
        )
    }

    pub fn launch_target(
        &mut self,
        request: RocgdbMiControlRequestV3,
        target: &Path,
        arguments: &[OsString],
        timeout: Duration,
    ) -> Result<RocgdbMiControlResultV3, RocgdbMiAdapterErrorV3> {
        if !matches!(request, RocgdbMiControlRequestV3::Launch { .. }) {
            return Err(RocgdbMiAdapterErrorV3::InvalidCommand);
        }
        request
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        let before = self.adapter.revision;
        if request.authorization().authorization_identity != self.adapter.authorization_identity {
            return self.control_unavailable(
                request,
                RocgdbMiControlOperationV3::Launch,
                before,
                RocgdbMiControlUnavailableReasonV3::AuthorizationMismatch,
            );
        }
        if request.authorization().expected_revision != before {
            return self.control_unavailable(
                request,
                RocgdbMiControlOperationV3::Launch,
                before,
                RocgdbMiControlUnavailableReasonV3::StaleRevision,
            );
        }
        if self.adapter.state != ExecutionStateV3::Unknown {
            return self.control_unavailable(
                request,
                RocgdbMiControlOperationV3::Launch,
                before,
                RocgdbMiControlUnavailableReasonV3::BackendRejected,
            );
        }
        if let Err(error) = self.configure_launch(target, arguments, timeout) {
            return self.launch_attach_failure(
                request,
                RocgdbMiControlOperationV3::Launch,
                before,
                error,
            );
        }
        self.inferior_ownership = InferiorOwnershipV3::LaunchOwned;
        if let Err(error) = self.launch(timeout) {
            return self.launch_attach_failure(
                request,
                RocgdbMiControlOperationV3::Launch,
                before,
                error,
            );
        }
        self.adapter.bump_revision()?;
        self.adapter.state = ExecutionStateV3::Running;
        self.control_result(
            request,
            RocgdbMiControlOperationV3::Launch,
            before,
            self.adapter.revision,
            RocgdbMiControlOutcomeV3::Applied {
                effect: RocgdbMiControlEffectV3::Committed,
                breakpoint: None,
            },
        )
    }

    pub fn attach_target(
        &mut self,
        request: RocgdbMiControlRequestV3,
        process: u32,
        timeout: Duration,
    ) -> Result<RocgdbMiControlResultV3, RocgdbMiAdapterErrorV3> {
        if !matches!(request, RocgdbMiControlRequestV3::Attach { .. }) {
            return Err(RocgdbMiAdapterErrorV3::InvalidCommand);
        }
        request
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        let before = self.adapter.revision;
        if request.authorization().authorization_identity != self.adapter.authorization_identity {
            return self.control_unavailable(
                request,
                RocgdbMiControlOperationV3::Attach,
                before,
                RocgdbMiControlUnavailableReasonV3::AuthorizationMismatch,
            );
        }
        if request.authorization().expected_revision != before {
            return self.control_unavailable(
                request,
                RocgdbMiControlOperationV3::Attach,
                before,
                RocgdbMiControlUnavailableReasonV3::StaleRevision,
            );
        }
        if self.adapter.state != ExecutionStateV3::Unknown {
            return self.control_unavailable(
                request,
                RocgdbMiControlOperationV3::Attach,
                before,
                RocgdbMiControlUnavailableReasonV3::BackendRejected,
            );
        }
        self.inferior_ownership = InferiorOwnershipV3::AttachBorrowed;
        if let Err(error) = self.attach(process, timeout) {
            return self.launch_attach_failure(
                request,
                RocgdbMiControlOperationV3::Attach,
                before,
                error,
            );
        }
        self.adapter.bump_revision()?;
        self.control_result(
            request,
            RocgdbMiControlOperationV3::Attach,
            before,
            self.adapter.revision,
            RocgdbMiControlOutcomeV3::Applied {
                effect: RocgdbMiControlEffectV3::Committed,
                breakpoint: None,
            },
        )
    }

    fn launch_attach_failure(
        &self,
        request: RocgdbMiControlRequestV3,
        operation: RocgdbMiControlOperationV3,
        revision: u64,
        error: RocgdbMiAdapterErrorV3,
    ) -> Result<RocgdbMiControlResultV3, RocgdbMiAdapterErrorV3> {
        let (reason, effect) = match error {
            RocgdbMiAdapterErrorV3::BackendRejected => (
                RocgdbMiControlUnavailableReasonV3::BackendRejected,
                RocgdbMiControlEffectV3::None,
            ),
            RocgdbMiAdapterErrorV3::Timeout
            | RocgdbMiAdapterErrorV3::ProcessExited
            | RocgdbMiAdapterErrorV3::ProcessIo => (
                RocgdbMiControlUnavailableReasonV3::BackendDisconnected,
                RocgdbMiControlEffectV3::Indeterminate,
            ),
            error => return Err(error),
        };
        self.control_failed(request, operation, revision, reason, effect)
    }

    pub fn next_event(
        &mut self,
        timeout: Duration,
    ) -> Result<RocgdbMiExecutionEventV3, RocgdbMiAdapterErrorV3> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(event);
        }
        let deadline = deadline(timeout)?;
        loop {
            let (record, line, authority_observed) =
                if let Some((record, line)) = self.deferred_records.pop_front() {
                    (record, line, true)
                } else {
                    let line = self.receive_line(deadline)?;
                    let record = parse_mi_record_v3(&line, self.adapter.limits.parser())?;
                    (record, line, false)
                };
            if !authority_observed {
                self.observe_process_authority(&record)?;
            }
            if let Some(event) = self.adapter.ingest_record(record, &line)? {
                return Ok(event);
            }
        }
    }

    pub fn admit_threads(
        &mut self,
        ordinals: &[u16],
        timeout: Duration,
    ) -> Result<Vec<RocgdbMiThreadAdmissionV3>, RocgdbMiAdapterErrorV3> {
        let (class, results) = self.send_command(b"-thread-info", deadline(timeout)?)?;
        if class != "done" {
            return Err(RocgdbMiAdapterErrorV3::BackendRejected);
        }
        let evidence = self.last_response_evidence.clone();
        self.adapter
            .admit_thread_results(&results, ordinals, &evidence)
    }

    /// Collects only the five documented structured MI3 hierarchy records for V4.
    /// Stream records may be processed for ordinary V3 async state, but are
    /// never forwarded to or interpreted by the V4 correlation adapter.
    pub fn collect_native_hierarchy_v4(
        &mut self,
        adapter: &mut crate::rocgdb_mi_v4::RocgdbMiNativeCorrelationAdapterV4,
        timeout: Duration,
    ) -> Result<(), crate::rocgdb_mi_v4::RocgdbMiNativeQueryErrorV4> {
        let deadline = deadline(timeout)?;
        macro_rules! collect {
            ($command:literal, $admit:ident) => {{
                let (class, _) = self.send_command($command, deadline)?;
                if class != "done" {
                    return Err(RocgdbMiAdapterErrorV3::BackendRejected.into());
                }
                adapter.$admit(&self.last_response_evidence)?;
            }};
        }
        collect!(b"-agent-info", admit_agent_info);
        collect!(b"-queue-info", admit_queue_info);
        collect!(b"-dispatch-info", admit_dispatch_info);
        collect!(b"-thread-info", admit_thread_info);
        collect!(b"-lane-info", admit_lane_info);
        Ok(())
    }

    pub fn inspect_registers(
        &mut self,
        scope: RocgdbMiStoppedScopeV3,
        timeout: Duration,
    ) -> Result<RocgdbMiRegisterSnapshotV3, RocgdbMiAdapterErrorV3> {
        self.require_scope(scope)?;
        let raw_thread = self.raw_thread(scope.thread)?.to_vec();
        let deadline = deadline(timeout)?;
        let names_command = command_with_thread(b"-data-list-register-names", &raw_thread)?;
        let mut values_command = command_with_thread(b"-data-list-register-values", &raw_thread)?;
        values_command.extend_from_slice(b" x");
        let (names_class, names) = self.send_command(&names_command, deadline)?;
        let names_evidence = self.last_response_evidence.clone();
        let (values_class, values) = self.send_command(&values_command, deadline)?;
        if names_class != "done" || values_class != "done" {
            return Err(RocgdbMiAdapterErrorV3::BackendRejected);
        }
        let names = const_list(names.get("register-names"))?;
        if names.len() > MAX_ROCGDB_MI_REGISTERS_V3 {
            return Err(RocgdbMiAdapterErrorV3::CountOutOfRange("registers"));
        }
        let tuples = tuple_list(values.get("register-values"))?;
        let mut truth_seed = names_command;
        truth_seed.extend_from_slice(&names_evidence);
        truth_seed.extend_from_slice(&values_command);
        truth_seed.extend_from_slice(&self.last_response_evidence);
        let truth = observed_truth(self.adapter.derive_identity(b"registers", &truth_seed)?);
        let mut registers = Vec::new();
        for tuple in tuples {
            let number = required_const(tuple, "number")?;
            let Some(index) =
                parse_decimal_u64(number).and_then(|value| usize::try_from(value).ok())
            else {
                return Err(RocgdbMiAdapterErrorV3::InvalidField("register number"));
            };
            let Some(name) = names.get(index) else {
                return Err(RocgdbMiAdapterErrorV3::InvalidField("register number"));
            };
            if name.is_empty() {
                continue;
            }
            let name = bounded_text(name, "register name")?;
            let value = required_const(tuple, "value")?;
            let mut identity_material = number.to_vec();
            identity_material.extend_from_slice(name.as_bytes());
            registers.push(LiveGpuRegisterValueV3 {
                register_identity: self
                    .adapter
                    .derive_identity(b"register", &identity_material)?,
                name: name.clone(),
                class: register_class(&name),
                kind: LiveGpuValueKindV3::UnsignedInteger,
                lane: scope.lane.map(|lane| lane.lane),
                value: map_scalar(value, &truth),
            });
        }
        let snapshot = RocgdbMiRegisterSnapshotV3 { scope, registers };
        snapshot
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        Ok(snapshot)
    }

    pub fn inspect_values(
        &mut self,
        scope: RocgdbMiStoppedScopeV3,
        timeout: Duration,
    ) -> Result<RocgdbMiValueSnapshotV3, RocgdbMiAdapterErrorV3> {
        self.require_scope(scope)?;
        let raw_thread = self.raw_thread(scope.thread)?.to_vec();
        let deadline = deadline(timeout)?;
        let mut command = command_with_thread(b"-stack-list-variables", &raw_thread)?;
        command.extend_from_slice(b" --simple-values");
        let (class, results) = self.send_command(&command, deadline)?;
        if class != "done" {
            return Err(RocgdbMiAdapterErrorV3::BackendRejected);
        }
        let variables = tuple_list(results.get("variables"))?;
        if variables.len() > MAX_ROCGDB_MI_VALUES_V3 {
            return Err(RocgdbMiAdapterErrorV3::CountOutOfRange("values"));
        }
        let mut truth_seed = command;
        truth_seed.extend_from_slice(&self.last_response_evidence);
        let truth = observed_truth(self.adapter.derive_identity(b"values", &truth_seed)?);
        let mut values = Vec::new();
        for (ordinal, variable) in variables.iter().enumerate() {
            let raw_name = required_const(variable, "name")?;
            let name = bounded_text(raw_name, "value name")?;
            let raw_value = optional_const(variable, "value").unwrap_or(b"<unavailable>");
            let mut material = u64::try_from(ordinal)
                .unwrap_or(u64::MAX)
                .to_le_bytes()
                .to_vec();
            material.extend_from_slice(raw_name);
            values.push(LiveGpuSemanticValueV3 {
                value_identity: self.adapter.derive_identity(b"value", &material)?,
                name,
                kind: LiveGpuValueKindV3::UnsignedInteger,
                value: map_scalar(raw_value, &truth),
            });
        }
        let snapshot = RocgdbMiValueSnapshotV3 { scope, values };
        snapshot
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        Ok(snapshot)
    }

    pub fn evaluate_expression(
        &mut self,
        scope: RocgdbMiStoppedScopeV3,
        value_identity: OpaqueIdentityV1,
        name: &str,
        expression: &str,
        timeout: Duration,
    ) -> Result<LiveGpuSemanticValueV3, RocgdbMiAdapterErrorV3> {
        self.require_scope(scope)?;
        let name = bounded_text(name.as_bytes(), "value name")?;
        if expression.is_empty()
            || expression.len() > MAX_COMMAND_BYTES_V3 / 2
            || expression.chars().any(|value| value == '\0')
        {
            return Err(RocgdbMiAdapterErrorV3::InvalidField("expression"));
        }
        let raw_thread = self.raw_thread(scope.thread)?.to_vec();
        let mut command = command_with_thread(b"-data-evaluate-expression", &raw_thread)?;
        command.push(b' ');
        append_quoted(&mut command, expression.as_bytes())?;
        let (class, results) = self.send_command(&command, deadline(timeout)?)?;
        let value = if class == "done" {
            let raw = required_const(&results, "value")?;
            let mut evidence = command;
            evidence.extend_from_slice(&self.last_response_evidence);
            let truth = observed_truth(self.adapter.derive_identity(b"expression", &evidence)?);
            map_scalar(raw, &truth)
        } else if class == "error" {
            unavailable(LiveGpuUnavailableReasonV3::NotCaptured)
        } else {
            return Err(RocgdbMiAdapterErrorV3::UnexpectedMiRecord);
        };
        Ok(LiveGpuSemanticValueV3 {
            value_identity,
            name,
            kind: LiveGpuValueKindV3::UnsignedInteger,
            value,
        })
    }

    pub fn read_memory(
        &mut self,
        request: RocgdbMiMemoryReadRequestV3,
        timeout: Duration,
    ) -> Result<RocgdbMiMemoryReadResultV3, RocgdbMiAdapterErrorV3> {
        request
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        self.require_scope(request.scope)?;
        if request.expected_revision != self.adapter.revision {
            return Err(RocgdbMiAdapterErrorV3::StaleRevision);
        }
        let authority = self
            .adapter
            .allocations
            .iter()
            .find(|(identity, _)| *identity == request.allocation)
            .map(|(_, authority)| *authority)
            .ok_or(RocgdbMiAdapterErrorV3::UnknownAllocation)?;
        let end = request
            .byte_offset
            .checked_add(request.byte_len)
            .ok_or(RocgdbMiAdapterErrorV3::InvalidField("memory range"))?;
        if end > authority.byte_len {
            return validated_memory_result(RocgdbMiMemoryReadResultV3 {
                request_id: request.request_id,
                revision: self.adapter.revision,
                memory: LiveGpuMemoryReadV3 {
                    allocation: request.allocation,
                    byte_offset: request.byte_offset,
                    requested_bytes: request.byte_len,
                    returned_bytes: 0,
                    value: unavailable(LiveGpuUnavailableReasonV3::OutsideCaptureScope),
                },
            });
        }
        let address = authority
            .base
            .checked_add(request.byte_offset)
            .ok_or(RocgdbMiAdapterErrorV3::InvalidField("memory range"))?;
        let command = format!("-data-read-memory-bytes 0x{address:x} {}", request.byte_len);
        let (class, results) = self.send_command(command.as_bytes(), deadline(timeout)?)?;
        if class == "error" {
            return validated_memory_result(RocgdbMiMemoryReadResultV3 {
                request_id: request.request_id,
                revision: self.adapter.revision,
                memory: LiveGpuMemoryReadV3 {
                    allocation: request.allocation,
                    byte_offset: request.byte_offset,
                    requested_bytes: request.byte_len,
                    returned_bytes: 0,
                    value: unavailable(LiveGpuUnavailableReasonV3::NotCaptured),
                },
            });
        }
        if class != "done" {
            return Err(RocgdbMiAdapterErrorV3::UnexpectedMiRecord);
        }
        let memory = tuple_list(results.get("memory"))?;
        if memory.len() != 1 {
            return Err(RocgdbMiAdapterErrorV3::InvalidField("memory"));
        }
        let contents = required_const(memory[0], "contents")?;
        let expected_hex = usize::try_from(request.byte_len)
            .ok()
            .and_then(|value| value.checked_mul(2))
            .ok_or(RocgdbMiAdapterErrorV3::CountOutOfRange("memory"))?;
        if contents.len() != expected_hex
            || !contents
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(RocgdbMiAdapterErrorV3::InvalidField("memory contents"));
        }
        let contents = String::from_utf8(contents.to_vec())
            .map_err(|_| RocgdbMiAdapterErrorV3::InvalidField("memory contents"))?;
        let mut truth_seed = command.into_bytes();
        truth_seed.extend_from_slice(&self.last_response_evidence);
        validated_memory_result(RocgdbMiMemoryReadResultV3 {
            request_id: request.request_id,
            revision: self.adapter.revision,
            memory: LiveGpuMemoryReadV3 {
                allocation: request.allocation,
                byte_offset: request.byte_offset,
                requested_bytes: request.byte_len,
                returned_bytes: request.byte_len,
                value: LiveGpuAvailabilityV3::Available {
                    value: LiveGpuMemoryBytesV3 {
                        space: authority.space,
                        bytes: contents,
                    },
                    truth: observed_truth(self.adapter.derive_identity(b"memory", &truth_seed)?),
                },
            },
        })
    }

    pub fn control(
        &mut self,
        request: RocgdbMiControlRequestV3,
        timeout: Duration,
    ) -> Result<RocgdbMiControlResultV3, RocgdbMiAdapterErrorV3> {
        if matches!(
            request,
            RocgdbMiControlRequestV3::Launch { .. } | RocgdbMiControlRequestV3::Attach { .. }
        ) {
            return Err(RocgdbMiAdapterErrorV3::InvalidCommand);
        }
        request
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        let before = self.adapter.revision;
        let authorization = request.authorization();
        let operation = control_operation(request);
        if authorization.authorization_identity != self.adapter.authorization_identity {
            return self.control_unavailable(
                request,
                operation,
                before,
                RocgdbMiControlUnavailableReasonV3::AuthorizationMismatch,
            );
        }
        if authorization.expected_revision != before {
            return self.control_unavailable(
                request,
                operation,
                before,
                RocgdbMiControlUnavailableReasonV3::StaleRevision,
            );
        }
        let expected_state = if matches!(request, RocgdbMiControlRequestV3::Pause { .. }) {
            ExecutionStateV3::Running
        } else {
            ExecutionStateV3::Stopped
        };
        if self.adapter.state != expected_state {
            return self.control_unavailable(
                request,
                operation,
                before,
                RocgdbMiControlUnavailableReasonV3::SessionNotStopped,
            );
        }
        let command = match self.control_command(request) {
            Ok(command) => command,
            Err(RocgdbMiAdapterErrorV3::UnknownThread) => {
                return self.control_unavailable(
                    request,
                    operation,
                    before,
                    RocgdbMiControlUnavailableReasonV3::SessionNotStopped,
                );
            }
            Err(RocgdbMiAdapterErrorV3::UnknownCodeObject)
            | Err(RocgdbMiAdapterErrorV3::UnknownSource)
            | Err(RocgdbMiAdapterErrorV3::UnknownAllocation) => {
                return self.control_unavailable(
                    request,
                    operation,
                    before,
                    RocgdbMiControlUnavailableReasonV3::BackendRejected,
                );
            }
            Err(error) => return Err(error),
        };
        let deadline = deadline(timeout)?;
        let response = self.send_control_command(&command, deadline);
        let (class, results) = match response {
            Ok(response) => response,
            Err(
                RocgdbMiAdapterErrorV3::Timeout
                | RocgdbMiAdapterErrorV3::ProcessIo
                | RocgdbMiAdapterErrorV3::ProcessExited,
            ) => {
                return self.control_failed(
                    request,
                    operation,
                    before,
                    RocgdbMiControlUnavailableReasonV3::BackendDisconnected,
                    RocgdbMiControlEffectV3::Indeterminate,
                );
            }
            Err(error) => return Err(error),
        };
        if class == "error" {
            return self.control_failed(
                request,
                operation,
                before,
                RocgdbMiControlUnavailableReasonV3::BackendRejected,
                RocgdbMiControlEffectV3::None,
            );
        }
        if !matches!(class.as_str(), "done" | "running") {
            return Err(RocgdbMiAdapterErrorV3::UnexpectedMiRecord);
        }
        let breakpoint = if let RocgdbMiControlRequestV3::InsertBreakpoint { .. } = request {
            let raw = results
                .get("bkpt")
                .and_then(MiValueV3::as_tuple)
                .ok_or(RocgdbMiAdapterErrorV3::MissingField("bkpt"))
                .and_then(|bkpt| required_const(bkpt, "number"))?;
            Some(self.adapter.bind_breakpoint(raw)?)
        } else {
            None
        };
        if let RocgdbMiControlRequestV3::RemoveBreakpoint { breakpoint, .. } = request
            && let Some(raw) = self.adapter.logical_breakpoints.remove(&breakpoint)
        {
            self.adapter.raw_breakpoints.remove(&raw);
        }
        commit_control_transition(&mut self.adapter, request)?;
        let after = self.adapter.revision;
        let outcome = RocgdbMiControlOutcomeV3::Applied {
            effect: RocgdbMiControlEffectV3::Committed,
            breakpoint,
        };
        self.control_result(request, operation, before, after, outcome)
    }

    /// Requests bounded debugger shutdown. `Drop` remains the final cleanup
    /// authority if ROCgdb disconnects or does not acknowledge `-gdb-exit`.
    pub fn shutdown(&mut self, timeout: Duration) -> Result<u64, RocgdbMiAdapterErrorV3> {
        if self.adapter.state == ExecutionStateV3::Exited {
            return Ok(self.adapter.revision);
        }
        let (class, _) = self.send_control_command(b"-gdb-exit", deadline(timeout)?)?;
        if !matches!(class.as_str(), "exit" | "done") {
            return Err(RocgdbMiAdapterErrorV3::BackendRejected);
        }
        self.adapter.bump_revision()?;
        self.adapter.state = ExecutionStateV3::Exited;
        self.adapter.current_stop = None;
        Ok(self.adapter.revision)
    }

    fn control_command(
        &self,
        request: RocgdbMiControlRequestV3,
    ) -> Result<Vec<u8>, RocgdbMiAdapterErrorV3> {
        match request {
            RocgdbMiControlRequestV3::Launch { .. } | RocgdbMiControlRequestV3::Attach { .. } => {
                Err(RocgdbMiAdapterErrorV3::InvalidCommand)
            }
            RocgdbMiControlRequestV3::InsertBreakpoint { site, .. } => {
                self.breakpoint_insert_command(site)
            }
            RocgdbMiControlRequestV3::RemoveBreakpoint { breakpoint, .. } => {
                let raw = self
                    .adapter
                    .logical_breakpoints
                    .get(&breakpoint)
                    .ok_or(RocgdbMiAdapterErrorV3::UnknownCodeObject)?;
                let mut command = b"-break-delete ".to_vec();
                append_quoted(&mut command, raw)?;
                Ok(command)
            }
            RocgdbMiControlRequestV3::Continue { focus, .. } => {
                command_with_thread(b"-exec-continue", self.raw_thread(focus)?)
            }
            RocgdbMiControlRequestV3::Pause { .. } => Ok(b"-exec-interrupt --all".to_vec()),
            RocgdbMiControlRequestV3::Step { focus, kind, .. } => {
                let command = match kind {
                    RocgdbMiStepKindV3::Instruction => b"-exec-step-instruction".as_slice(),
                    RocgdbMiStepKindV3::Into => b"-exec-step".as_slice(),
                    RocgdbMiStepKindV3::Over => b"-exec-next".as_slice(),
                    RocgdbMiStepKindV3::Out => b"-exec-finish".as_slice(),
                };
                command_with_thread(command, self.raw_thread(focus)?)
            }
        }
    }

    fn breakpoint_insert_command(
        &self,
        site: RocgdbMiBreakpointSiteV3,
    ) -> Result<Vec<u8>, RocgdbMiAdapterErrorV3> {
        let mut command = b"-break-insert ".to_vec();
        match site {
            RocgdbMiBreakpointSiteV3::CodeObjectRelative {
                code_object,
                kernel_entry_byte_offset,
            } => {
                let binding = self
                    .adapter
                    .code_objects
                    .iter()
                    .find(|binding| binding.content == code_object)
                    .ok_or(RocgdbMiAdapterErrorV3::UnknownCodeObject)?;
                let address = binding
                    .kernel_entry
                    .checked_add(kernel_entry_byte_offset)
                    .filter(|address| *address < binding.load_base.saturating_add(binding.byte_len))
                    .ok_or(RocgdbMiAdapterErrorV3::UnknownCodeObject)?;
                command.extend_from_slice(format!("*0x{address:x}").as_bytes());
            }
            RocgdbMiBreakpointSiteV3::Source { source } => {
                let binding = self
                    .adapter
                    .sources
                    .iter()
                    .find(|binding| binding.span == source)
                    .ok_or(RocgdbMiAdapterErrorV3::UnknownSource)?;
                let mut location = binding.path.clone();
                location.push(b':');
                location.extend_from_slice(binding.line.to_string().as_bytes());
                append_quoted(&mut command, &location)?;
            }
        }
        Ok(command)
    }

    fn raw_thread(
        &self,
        logical: RocgdbMiThreadIdentityV3,
    ) -> Result<&[u8], RocgdbMiAdapterErrorV3> {
        self.adapter
            .logical_threads
            .get(&logical)
            .map(Vec::as_slice)
            .ok_or(RocgdbMiAdapterErrorV3::UnknownThread)
    }

    fn require_scope(&self, scope: RocgdbMiStoppedScopeV3) -> Result<(), RocgdbMiAdapterErrorV3> {
        scope
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        if self.adapter.authenticated_gpu_threads.is_empty() {
            return Err(RocgdbMiAdapterErrorV3::GpuClassificationUnavailable);
        }
        if self.adapter.state != ExecutionStateV3::Stopped
            || self.adapter.current_stop != Some(scope.stop_identity)
        {
            return Err(RocgdbMiAdapterErrorV3::SessionNotStopped);
        }
        if !self.adapter.logical_threads.contains_key(&scope.thread) {
            return Err(RocgdbMiAdapterErrorV3::UnknownThread);
        }
        Ok(())
    }

    fn control_unavailable(
        &self,
        request: RocgdbMiControlRequestV3,
        operation: RocgdbMiControlOperationV3,
        revision: u64,
        reason: RocgdbMiControlUnavailableReasonV3,
    ) -> Result<RocgdbMiControlResultV3, RocgdbMiAdapterErrorV3> {
        self.control_result(
            request,
            operation,
            revision,
            revision,
            RocgdbMiControlOutcomeV3::Unavailable {
                reason,
                effect: RocgdbMiControlEffectV3::None,
            },
        )
    }

    fn control_failed(
        &self,
        request: RocgdbMiControlRequestV3,
        operation: RocgdbMiControlOperationV3,
        revision: u64,
        reason: RocgdbMiControlUnavailableReasonV3,
        effect: RocgdbMiControlEffectV3,
    ) -> Result<RocgdbMiControlResultV3, RocgdbMiAdapterErrorV3> {
        self.control_result(
            request,
            operation,
            revision,
            revision,
            RocgdbMiControlOutcomeV3::Failed { reason, effect },
        )
    }

    fn control_result(
        &self,
        request: RocgdbMiControlRequestV3,
        operation: RocgdbMiControlOperationV3,
        before: u64,
        after: u64,
        outcome: RocgdbMiControlOutcomeV3,
    ) -> Result<RocgdbMiControlResultV3, RocgdbMiAdapterErrorV3> {
        let effect = match outcome {
            RocgdbMiControlOutcomeV3::Applied { effect, .. }
            | RocgdbMiControlOutcomeV3::Unavailable { effect, .. }
            | RocgdbMiControlOutcomeV3::Failed { effect, .. } => effect,
        };
        let mut audit_material = serde_json::to_vec(&request)
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        audit_material.extend_from_slice(&before.to_le_bytes());
        audit_material.extend_from_slice(&after.to_le_bytes());
        audit_material.push(effect as u8);
        let result = RocgdbMiControlResultV3 {
            request_id: request.request_id(),
            operation,
            revision: after,
            outcome,
            audit: RocgdbMiControlAuditV3 {
                audit_identity: self
                    .adapter
                    .derive_identity(b"control-audit", &audit_material)?,
                authorization_identity: request.authorization().authorization_identity,
                before_revision: before,
                after_revision: after,
                effect,
            },
        };
        result
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        Ok(result)
    }

    fn send_command(
        &mut self,
        command: &[u8],
        deadline: Instant,
    ) -> Result<(String, MiResultsV3), RocgdbMiAdapterErrorV3> {
        if command.is_empty()
            || command.len() > MAX_COMMAND_BYTES_V3
            || command.contains(&b'\n')
            || command.contains(&b'\r')
            || !command.starts_with(b"-")
        {
            return Err(RocgdbMiAdapterErrorV3::InvalidCommand);
        }
        let token = self.next_token;
        if self.pending_token.is_some() {
            return Err(RocgdbMiAdapterErrorV3::UnexpectedMiRecord);
        }
        self.next_token = self
            .next_token
            .checked_add(1)
            .ok_or(RocgdbMiAdapterErrorV3::CountOutOfRange("token"))?;
        self.pending_token = Some(token);
        if write!(&mut self.input, "{token}")
            .and_then(|()| self.input.write_all(command))
            .and_then(|()| self.input.write_all(b"\n"))
            .and_then(|()| self.input.flush())
            .is_err()
        {
            self.pending_token = None;
            return Err(RocgdbMiAdapterErrorV3::ProcessIo);
        }
        for _ in 0..self.adapter.limits.max_records_per_command {
            let line = match self.receive_line(deadline) {
                Ok(line) => line,
                Err(error) => {
                    self.pending_token = None;
                    return Err(error);
                }
            };
            let record = match parse_mi_record_v3(&line, self.adapter.limits.parser()) {
                Ok(record) => record,
                Err(error) => {
                    self.pending_token = None;
                    return Err(error.into());
                }
            };
            if let Err(error) = self.observe_process_authority(&record) {
                self.pending_token = None;
                return Err(error);
            }
            match record {
                MiRecordV3::Result {
                    token: Some(actual),
                    class,
                    results,
                } if actual == token => {
                    self.pending_token = None;
                    self.last_response_evidence = line;
                    return Ok((class, results));
                }
                MiRecordV3::Result { token: Some(_), .. } => {
                    self.pending_token = None;
                    return Err(RocgdbMiAdapterErrorV3::UnexpectedMiRecord);
                }
                record => {
                    if self.defer_observations {
                        if self.deferred_records.len() >= self.adapter.limits.max_pending_events {
                            self.pending_token = None;
                            return Err(RocgdbMiAdapterErrorV3::EventBudgetExhausted);
                        }
                        self.deferred_records.push_back((record, line));
                        continue;
                    }
                    let event = match self.adapter.ingest_record(record, &line) {
                        Ok(event) => event,
                        Err(error) => {
                            self.pending_token = None;
                            return Err(error);
                        }
                    };
                    if let Some(event) = event {
                        if self.pending_events.len() >= self.adapter.limits.max_pending_events {
                            self.pending_token = None;
                            return Err(RocgdbMiAdapterErrorV3::EventBudgetExhausted);
                        }
                        self.pending_events.push_back(event);
                    }
                }
            }
        }
        self.pending_token = None;
        Err(RocgdbMiAdapterErrorV3::ResponseBudgetExhausted)
    }

    fn send_control_command(
        &mut self,
        command: &[u8],
        deadline: Instant,
    ) -> Result<(String, MiResultsV3), RocgdbMiAdapterErrorV3> {
        if self.defer_observations {
            return Err(RocgdbMiAdapterErrorV3::UnexpectedMiRecord);
        }
        self.defer_observations = true;
        let result = self.send_command(command, deadline);
        self.defer_observations = false;
        result
    }

    fn receive_line(&self, deadline: Instant) -> Result<Vec<u8>, RocgdbMiAdapterErrorV3> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(RocgdbMiAdapterErrorV3::Timeout);
        }
        match self.output.recv_timeout(remaining) {
            Ok(ReaderItemV3::Line(line)) => Ok(line),
            Ok(ReaderItemV3::TooLarge) => Err(RocgdbMiAdapterErrorV3::InvalidMiRecord),
            Ok(ReaderItemV3::Io) => Err(RocgdbMiAdapterErrorV3::ProcessIo),
            Ok(ReaderItemV3::Eof) => Err(RocgdbMiAdapterErrorV3::ProcessExited),
            Err(RecvTimeoutError::Timeout) => Err(RocgdbMiAdapterErrorV3::Timeout),
            Err(RecvTimeoutError::Disconnected) => Err(RocgdbMiAdapterErrorV3::ProcessExited),
        }
    }

    fn observe_process_authority(
        &mut self,
        record: &MiRecordV3,
    ) -> Result<(), RocgdbMiAdapterErrorV3> {
        let MiRecordV3::Async {
            kind: crate::rocgdb_mi_parser_v3::MiAsyncKindV3::Notify,
            class,
            results,
            ..
        } = record
        else {
            return Ok(());
        };
        if class == "thread-group-exited" {
            self.inferior_process = None;
            self.inferior_pid = None;
            return Ok(());
        }
        if class != "thread-group-started" {
            return Ok(());
        }
        if self.inferior_process.is_some() {
            return Err(RocgdbMiAdapterErrorV3::UnexpectedMiRecord);
        }
        let raw = required_const(results, "pid")?;
        let pid = parse_decimal_u64(raw)
            .and_then(|value| i32::try_from(value).ok())
            .filter(|value| *value > 0)
            .ok_or(RocgdbMiAdapterErrorV3::InvalidField("inferior process"))?;
        // SAFETY: `pidfd_open` returns a new descriptor on success. The
        // positive pid and zero flags were checked above, and ownership is
        // transferred exactly once to `OwnedFd`.
        let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0_u32) };
        if descriptor < 0 {
            return Err(RocgdbMiAdapterErrorV3::ProcessIo);
        }
        // SAFETY: successful `pidfd_open` returned one owned descriptor.
        self.inferior_process = Some(unsafe { OwnedFd::from_raw_fd(descriptor as i32) });
        self.inferior_pid = Some(pid as u32);
        Ok(())
    }
}

fn lower_hex_v4(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl Drop for RocgdbMiProcessV3 {
    fn drop(&mut self) {
        if let Some(process) = self.inferior_process.take()
            && self.inferior_ownership == InferiorOwnershipV3::LaunchOwned
        {
            // SAFETY: the first argument is an owned pidfd, the signal has no
            // payload, and flags are required to be zero.
            let _ = unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    process.as_raw_fd(),
                    libc::SIGKILL,
                    std::ptr::null::<libc::siginfo_t>(),
                    0_u32,
                )
            };
            let mut descriptor = libc::pollfd {
                fd: process.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: `descriptor` points to one initialized pollfd for the
            // duration of this bounded call.
            let _ = unsafe { libc::poll(&mut descriptor, 1, 5_000) };
        }
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5));
                }
                Ok(None) => break,
            }
        }
    }
}

fn read_bounded_line(reader: &mut impl BufRead, max: usize) -> ReaderItemV3 {
    let mut line = Vec::new();
    let limit = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
    match reader.take(limit).read_until(b'\n', &mut line) {
        Ok(0) => ReaderItemV3::Eof,
        Ok(_) if line.len() > max => ReaderItemV3::TooLarge,
        Ok(_) if line.last() != Some(&b'\n') => ReaderItemV3::TooLarge,
        Ok(_) => ReaderItemV3::Line(line),
        Err(_) => ReaderItemV3::Io,
    }
}

fn deadline(timeout: Duration) -> Result<Instant, RocgdbMiAdapterErrorV3> {
    if timeout.is_zero() || timeout > Duration::from_secs(60) {
        return Err(RocgdbMiAdapterErrorV3::InvalidTimeout);
    }
    Instant::now()
        .checked_add(timeout)
        .ok_or(RocgdbMiAdapterErrorV3::InvalidTimeout)
}

fn expect_class(
    response: (String, MiResultsV3),
    expected: &str,
) -> Result<(), RocgdbMiAdapterErrorV3> {
    if response.0 == expected {
        Ok(())
    } else {
        Err(RocgdbMiAdapterErrorV3::BackendRejected)
    }
}

fn validated_memory_result(
    result: RocgdbMiMemoryReadResultV3,
) -> Result<RocgdbMiMemoryReadResultV3, RocgdbMiAdapterErrorV3> {
    result
        .validate()
        .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
    Ok(result)
}

fn const_list(value: Option<&MiValueV3>) -> Result<Vec<Vec<u8>>, RocgdbMiAdapterErrorV3> {
    let values = value
        .and_then(MiValueV3::as_values)
        .ok_or(RocgdbMiAdapterErrorV3::InvalidField("list"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_const()
                .map(<[u8]>::to_vec)
                .ok_or(RocgdbMiAdapterErrorV3::InvalidField("list item"))
        })
        .collect()
}

fn tuple_list(value: Option<&MiValueV3>) -> Result<Vec<&MiResultsV3>, RocgdbMiAdapterErrorV3> {
    let Some(value) = value else {
        return Err(RocgdbMiAdapterErrorV3::InvalidField("tuple list"));
    };
    match value {
        MiValueV3::List(MiListV3::Values(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_tuple()
                    .ok_or(RocgdbMiAdapterErrorV3::InvalidField("tuple list item"))
            })
            .collect(),
        MiValueV3::List(MiListV3::Results(results)) => results
            .iter()
            .map(|(_, value)| {
                value
                    .as_tuple()
                    .ok_or(RocgdbMiAdapterErrorV3::InvalidField("tuple list item"))
            })
            .collect(),
        MiValueV3::Const(_) | MiValueV3::Tuple(_) => {
            Err(RocgdbMiAdapterErrorV3::InvalidField("tuple list"))
        }
    }
}

fn append_quoted(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), RocgdbMiAdapterErrorV3> {
    output.push(b'"');
    for byte in bytes {
        match byte {
            b'"' | b'\\' => {
                output.push(b'\\');
                output.push(*byte);
            }
            b' '..=b'~' => output.push(*byte),
            byte => output.extend_from_slice(format!("\\{byte:03o}").as_bytes()),
        }
        if output.len() > MAX_COMMAND_BYTES_V3 {
            return Err(RocgdbMiAdapterErrorV3::InvalidCommand);
        }
    }
    output.push(b'"');
    Ok(())
}

fn command_with_thread(command: &[u8], thread: &[u8]) -> Result<Vec<u8>, RocgdbMiAdapterErrorV3> {
    validate_native_token(thread, "thread")?;
    let mut output = command.to_vec();
    output.extend_from_slice(b" --thread ");
    append_quoted(&mut output, thread)?;
    Ok(output)
}

fn bounded_text(bytes: &[u8], field: &'static str) -> Result<String, RocgdbMiAdapterErrorV3> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| RocgdbMiAdapterErrorV3::InvalidField(field))?;
    if text.is_empty()
        || text.len() > MAX_NATIVE_TOKEN_BYTES_V3
        || text.chars().any(char::is_control)
    {
        return Err(RocgdbMiAdapterErrorV3::InvalidField(field));
    }
    Ok(text.to_owned())
}

fn register_class(name: &str) -> LiveGpuRegisterClassV3 {
    if name.starts_with('s') && name[1..].bytes().all(|byte| byte.is_ascii_digit()) {
        LiveGpuRegisterClassV3::Scalar
    } else if name.starts_with('v') && name[1..].bytes().all(|byte| byte.is_ascii_digit()) {
        LiveGpuRegisterClassV3::Vector
    } else if matches!(name, "exec" | "vcc" | "scc") {
        LiveGpuRegisterClassV3::Predicate
    } else {
        LiveGpuRegisterClassV3::Special
    }
}

fn map_scalar(raw: &[u8], truth: &LiveGpuTruthV3) -> LiveGpuAvailabilityV3<LiveGpuValueEncodingV3> {
    if raw == b"<optimized out>" {
        return unavailable(LiveGpuUnavailableReasonV3::OptimizedOut);
    }
    if raw == b"<unavailable>" {
        return unavailable(LiveGpuUnavailableReasonV3::NotCaptured);
    }
    let Some(digits) = raw.strip_prefix(b"0x") else {
        return unavailable(LiveGpuUnavailableReasonV3::NotCaptured);
    };
    if digits.is_empty()
        || digits.len() > usize::from(MAX_LIVE_GPU_VALUE_BITS_V3 / 4)
        || !digits.iter().all(u8::is_ascii_hexdigit)
    {
        return unavailable(LiveGpuUnavailableReasonV3::NotCaptured);
    }
    let bits = digits
        .iter()
        .map(u8::to_ascii_lowercase)
        .map(char::from)
        .collect();
    LiveGpuAvailabilityV3::Available {
        value: LiveGpuValueEncodingV3::Bits {
            bit_width: u16::try_from(digits.len() * 4).unwrap_or(u16::MAX),
            bits,
        },
        truth: truth.clone(),
    }
}

fn control_operation(request: RocgdbMiControlRequestV3) -> RocgdbMiControlOperationV3 {
    match request {
        RocgdbMiControlRequestV3::Launch { .. } => RocgdbMiControlOperationV3::Launch,
        RocgdbMiControlRequestV3::Attach { .. } => RocgdbMiControlOperationV3::Attach,
        RocgdbMiControlRequestV3::InsertBreakpoint { .. } => {
            RocgdbMiControlOperationV3::InsertBreakpoint
        }
        RocgdbMiControlRequestV3::RemoveBreakpoint { .. } => {
            RocgdbMiControlOperationV3::RemoveBreakpoint
        }
        RocgdbMiControlRequestV3::Continue { .. } => RocgdbMiControlOperationV3::Continue,
        RocgdbMiControlRequestV3::Pause { .. } => RocgdbMiControlOperationV3::Pause,
        RocgdbMiControlRequestV3::Step { .. } => RocgdbMiControlOperationV3::Step,
    }
}

fn commit_control_transition(
    adapter: &mut RocgdbMiObservationAdapterV3,
    request: RocgdbMiControlRequestV3,
) -> Result<(), RocgdbMiAdapterErrorV3> {
    adapter.bump_revision()?;
    if matches!(
        request,
        RocgdbMiControlRequestV3::Continue { .. } | RocgdbMiControlRequestV3::Step { .. }
    ) {
        adapter.state = ExecutionStateV3::Running;
        adapter.current_stop = None;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(byte: u8) -> OpaqueIdentityV1 {
        OpaqueIdentityV1::new([byte; 32]).expect("nonzero identity")
    }

    fn stopped_adapter() -> (RocgdbMiObservationAdapterV3, RocgdbMiThreadIdentityV3) {
        let mut adapter = RocgdbMiObservationAdapterV3::new(
            identity(1),
            identity(2),
            64,
            RocgdbMiAdapterLimitsV3::default(),
        )
        .expect("adapter");
        let admitted = adapter
            .admit_threads_from_thread_info(b"1^done,threads=[{id=\"9\"}]\n", &[0])
            .expect("admission");
        adapter
            .ingest_line(b"*stopped,reason=\"signal-received\",thread-id=\"9\"\n")
            .expect("stop");
        (adapter, admitted[0].thread)
    }

    fn continue_request(
        thread: RocgdbMiThreadIdentityV3,
        revision: u64,
    ) -> RocgdbMiControlRequestV3 {
        RocgdbMiControlRequestV3::Continue {
            request_id: 1,
            authorization: RocgdbMiControlAuthorizationV3 {
                authorization_identity: identity(2),
                expected_revision: revision,
            },
            focus: thread,
        }
    }

    #[test]
    fn async_before_result_is_one_committed_revision() {
        let (mut adapter, thread) = stopped_adapter();
        let before = adapter.revision();
        let record = parse_mi_record_v3(b"*running,thread-id=\"9\"\n", adapter.limits.parser())
            .expect("deferred record");
        commit_control_transition(&mut adapter, continue_request(thread, before)).expect("commit");
        let event = adapter
            .ingest_record(record, b"*running,thread-id=\"9\"\n")
            .expect("event");
        assert_eq!(adapter.revision(), before + 1);
        assert_eq!(
            event,
            Some(RocgdbMiExecutionEventV3::Running {
                revision: before + 1
            })
        );
    }

    #[test]
    fn result_before_async_is_one_committed_revision() {
        let (mut adapter, thread) = stopped_adapter();
        let before = adapter.revision();
        commit_control_transition(&mut adapter, continue_request(thread, before)).expect("commit");
        let event = adapter
            .ingest_line(b"*running,thread-id=\"9\"\n")
            .expect("event");
        assert_eq!(adapter.revision(), before + 1);
        assert_eq!(
            event,
            Some(RocgdbMiExecutionEventV3::Running {
                revision: before + 1
            })
        );
    }
}
