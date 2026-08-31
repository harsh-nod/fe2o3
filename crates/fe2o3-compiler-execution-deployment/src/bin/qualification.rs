use std::fs::File;
use std::io::Write as _;
use std::os::unix::fs::FileExt as _;
use std::os::unix::process::CommandExt as _;
use std::os::unix::process::ExitStatusExt as _;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use fe2o3_compiler_execution_deployment::{
    COMPILER_EXECUTION_SYSTEMD_PREFLIGHT_PARENT_PID_ENV_V1,
    COMPILER_EXECUTION_SYSTEMD_PREFLIGHT_TOOL_COMMAND_V1, CompilerExecutionInstallRecoveryV1,
    CompilerExecutionQualificationRecoveryV1, CompilerExecutionQualificationRequestV1,
    CompilerExecutionQualificationSupervisorLeaseV1, QualificationFaultPointV1,
    QualificationWorkerTerminationV1, acquire_compiler_execution_qualification_supervisor_lease_v1,
    execute_compiler_execution_systemd_preflight_tool_v1,
    probe_compiler_execution_qualification_host_v1, recover_compiler_execution_install_parent_v1,
    recover_compiler_execution_qualification_parent_v1,
    run_compiler_execution_qualification_campaign_v1,
    run_compiler_execution_qualification_fault_v1, run_compiler_execution_qualification_request_v1,
    wait_for_compiler_execution_qualification_supervisor_lease_v1,
    wait_for_qualification_worker_v1,
};
use rustix::fs::{MemfdFlags, memfd_create};
use rustix::process::{Signal, getppid, set_parent_process_death_signal};
use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};

const WORKER_RUN_COMMAND_V1: &str = "__worker-run-v1";
const WORKER_FAULT_COMMAND_V1: &str = "__worker-fault-v1";
const WORKER_CAMPAIGN_COMMAND_V1: &str = "__worker-campaign-v1";
const WORKER_PARENT_PID_ENV_V1: &str = "FE2O3_QUALIFICATION_WORKER_PARENT_PID_V1";
const RUN_TIMEOUT_V1: Duration = Duration::from_secs(120);
const FAULT_TIMEOUT_V1: Duration = Duration::from_secs(120);
const CAMPAIGN_TIMEOUT_V1: Duration = Duration::from_secs(20 * 60);
const MAX_WORKER_OUTPUT_BYTES_V1: u64 = 1024 * 1024;

const USAGE: &str = "usage: fe2o3-compiler-execution-qualification probe\n       fe2o3-compiler-execution-qualification fault-points\n       fe2o3-compiler-execution-qualification recover QUALIFICATION_PARENT\n       fe2o3-compiler-execution-qualification recover-install EXPECTED_MANIFEST_SHA256 INSTALL_PARENT\n       fe2o3-compiler-execution-qualification run BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT INSTALL_PARENT BASE_IMAGE EXPECTED_BASE_IMAGE_SHA256 QUALIFICATION_PARENT\n       fe2o3-compiler-execution-qualification fault POINT BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT INSTALL_PARENT BASE_IMAGE EXPECTED_BASE_IMAGE_SHA256 QUALIFICATION_PARENT\n       fe2o3-compiler-execution-qualification campaign BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT EMPTY_INSTALL_PARENT BASE_IMAGE EXPECTED_BASE_IMAGE_SHA256 QUALIFICATION_PARENT";

fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
    let Some(command) = arguments.get(1).and_then(|argument| argument.to_str()) else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    match command {
        "probe" if arguments.len() == 2 => run_probe(),
        "fault-points" if arguments.len() == 2 => print_fault_points(),
        "recover" if arguments.len() == 3 => run_recovery(&arguments),
        "recover-install" if arguments.len() == 4 => run_install_recovery(&arguments),
        "run" if arguments.len() == 9 => {
            supervise_qualification(&arguments, 2, WORKER_RUN_COMMAND_V1, RUN_TIMEOUT_V1)
        }
        "fault" if arguments.len() == 10 => {
            supervise_qualification(&arguments, 3, WORKER_FAULT_COMMAND_V1, FAULT_TIMEOUT_V1)
        }
        "campaign" if arguments.len() == 9 => supervise_qualification(
            &arguments,
            2,
            WORKER_CAMPAIGN_COMMAND_V1,
            CAMPAIGN_TIMEOUT_V1,
        ),
        WORKER_RUN_COMMAND_V1 if arguments.len() == 9 => {
            let _lease = prepare_qualification_worker(&arguments, 2);
            run_qualification(&arguments);
        }
        WORKER_FAULT_COMMAND_V1 if arguments.len() == 10 => {
            let _lease = prepare_qualification_worker(&arguments, 3);
            run_fault(&arguments);
        }
        WORKER_CAMPAIGN_COMMAND_V1 if arguments.len() == 9 => {
            let _lease = prepare_qualification_worker(&arguments, 2);
            run_campaign(&arguments);
        }
        COMPILER_EXECUTION_SYSTEMD_PREFLIGHT_TOOL_COMMAND_V1 if arguments.len() == 3 => {
            run_systemd_preflight_tool(&arguments)
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

fn supervise_qualification(
    arguments: &[std::ffi::OsString],
    request_start: usize,
    worker_command: &str,
    timeout: Duration,
) {
    if rustix::process::geteuid().as_raw() != 0 {
        eprintln!("compiler-execution qualification supervision requires effective UID 0");
        std::process::exit(1);
    }
    let Some(manifest_sha256) = arguments[request_start + 1].to_str() else {
        eprintln!("expected manifest SHA-256 must be UTF-8");
        std::process::exit(2);
    };
    let install_parent = Path::new(&arguments[request_start + 3]);
    let qualification_parent = Path::new(&arguments[request_start + 6]);
    let lease = match acquire_compiler_execution_qualification_supervisor_lease_v1(
        install_parent,
        qualification_parent,
    ) {
        Ok(lease) => lease,
        Err(error) => {
            eprintln!("compiler-execution qualification supervisor lease failed: {error}");
            std::process::exit(1);
        }
    };
    let registered_signal = match register_supervisor_signals() {
        Ok(signal) => signal,
        Err(error) => {
            eprintln!("compiler-execution qualification signal registration failed: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) =
        recover_supervised_state(install_parent, manifest_sha256, qualification_parent)
    {
        eprintln!("compiler-execution qualification initial recovery failed: {error}");
        std::process::exit(1);
    }
    if let Some(signal) = observed_signal(&registered_signal) {
        eprintln!("compiler-execution qualification interrupted by signal {signal} before launch");
        exit_for_signal(signal);
    }

    let output = match WorkerOutputCaptureV1::new() {
        Ok(output) => output,
        Err(error) => {
            eprintln!("compiler-execution qualification output preparation failed: {error}");
            std::process::exit(1);
        }
    };
    let mut child = match spawn_qualification_worker(arguments, worker_command, &output) {
        Ok(child) => child,
        Err(error) => {
            let recovery =
                recover_supervised_state(install_parent, manifest_sha256, qualification_parent);
            eprintln!("compiler-execution qualification worker launch failed: {error}");
            if let Err(recovery) = recovery {
                eprintln!(
                    "compiler-execution qualification post-launch recovery failed: {recovery}"
                );
            }
            std::process::exit(1);
        }
    };
    drop(lease);

    let outcome = match wait_for_qualification_worker_v1(&mut child, timeout, &registered_signal) {
        Ok(outcome) => Ok(outcome),
        Err(error) => match force_terminate_and_reap(&mut child) {
            Ok(()) => Err(error.to_string()),
            Err(termination) => {
                eprintln!("compiler-execution qualification supervision failed: {error}");
                eprintln!(
                    "compiler-execution qualification worker remains unconfirmed: {termination}"
                );
                std::process::exit(1);
            }
        },
    };
    let post_worker_lease = match acquire_compiler_execution_qualification_supervisor_lease_v1(
        install_parent,
        qualification_parent,
    ) {
        Ok(lease) => lease,
        Err(error) => {
            eprintln!("compiler-execution qualification post-worker lease failed: {error}");
            std::process::exit(1);
        }
    };
    let recovery = recover_supervised_state(install_parent, manifest_sha256, qualification_parent);
    drop(post_worker_lease);
    let captured = output.read();
    let recovery = match recovery {
        Ok(recovery) => recovery,
        Err(error) => {
            if let Ok(captured) = &captured {
                emit_failure_output(captured);
            }
            eprintln!("compiler-execution qualification post-worker recovery failed: {error}");
            std::process::exit(1);
        }
    };
    let captured = match captured {
        Ok(captured) => captured,
        Err(error) => {
            eprintln!("compiler-execution qualification output admission failed: {error}");
            std::process::exit(1);
        }
    };

    let outcome = match outcome {
        Ok(completed @ QualificationWorkerTerminationV1::Completed(_)) => {
            observed_signal(&registered_signal)
                .map(QualificationWorkerTerminationV1::Signaled)
                .unwrap_or(completed)
        }
        Ok(outcome) => outcome,
        Err(error) => {
            emit_failure_output(&captured);
            eprintln!("compiler-execution qualification supervision failed: {error}");
            std::process::exit(1);
        }
    };
    finish_supervised_qualification(outcome, recovery, captured);
}

fn register_supervisor_signals() -> Result<Arc<AtomicUsize>, String> {
    let observed = Arc::new(AtomicUsize::new(0));
    for signal in [SIGTERM, SIGINT, SIGHUP, SIGQUIT] {
        signal_hook::flag::register_usize(signal, Arc::clone(&observed), signal as usize)
            .map_err(|error| format!("cannot register signal {signal}: {error}"))?;
    }
    Ok(observed)
}

fn observed_signal(signal: &AtomicUsize) -> Option<i32> {
    let signal = signal.load(Ordering::Acquire);
    (signal != 0).then(|| i32::try_from(signal).expect("registered signal fits i32"))
}

fn recover_supervised_state(
    install_parent: &Path,
    manifest_sha256: &str,
    qualification_parent: &Path,
) -> Result<SupervisedRecoveryV1, String> {
    let install = recover_compiler_execution_install_parent_v1(install_parent, manifest_sha256);
    let qualification = recover_compiler_execution_qualification_parent_v1(qualification_parent);
    match (install, qualification) {
        (Ok(install), Ok(qualification)) => Ok(SupervisedRecoveryV1 {
            install,
            qualification,
        }),
        (Err(install), Ok(_)) => Err(format!("installer recovery failed: {install}")),
        (Ok(_), Err(qualification)) => {
            Err(format!("qualification recovery failed: {qualification}"))
        }
        (Err(install), Err(qualification)) => Err(format!(
            "installer recovery failed ({install}); qualification recovery failed ({qualification})"
        )),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SupervisedRecoveryV1 {
    install: CompilerExecutionInstallRecoveryV1,
    qualification: CompilerExecutionQualificationRecoveryV1,
}

impl SupervisedRecoveryV1 {
    fn is_already_clean(self) -> bool {
        self.install == CompilerExecutionInstallRecoveryV1::AlreadyClean
            && self.qualification == CompilerExecutionQualificationRecoveryV1::AlreadyEmpty
    }
}

struct WorkerOutputCaptureV1 {
    stdout: File,
    stderr: File,
}

struct CapturedWorkerOutputV1 {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl WorkerOutputCaptureV1 {
    fn new() -> Result<Self, String> {
        let stdout = memfd_create(c"fe2o3-qualification-worker-stdout-v1", MemfdFlags::CLOEXEC)
            .map(File::from)
            .map_err(|error| format!("cannot create worker stdout memfd: {error}"))?;
        let stderr = memfd_create(c"fe2o3-qualification-worker-stderr-v1", MemfdFlags::CLOEXEC)
            .map(File::from)
            .map_err(|error| format!("cannot create worker stderr memfd: {error}"))?;
        Ok(Self { stdout, stderr })
    }

    fn stdout_stdio(&self) -> Result<Stdio, String> {
        self.stdout
            .try_clone()
            .map(Stdio::from)
            .map_err(|error| format!("cannot clone worker stdout custody: {error}"))
    }

    fn stderr_stdio(&self) -> Result<Stdio, String> {
        self.stderr
            .try_clone()
            .map(Stdio::from)
            .map_err(|error| format!("cannot clone worker stderr custody: {error}"))
    }

    fn read(&self) -> Result<CapturedWorkerOutputV1, String> {
        Ok(CapturedWorkerOutputV1 {
            stdout: read_bounded_output(&self.stdout, "stdout")?,
            stderr: read_bounded_output(&self.stderr, "stderr")?,
        })
    }
}

fn read_bounded_output(file: &File, role: &str) -> Result<Vec<u8>, String> {
    let byte_len = file
        .metadata()
        .map_err(|error| format!("cannot inspect worker {role}: {error}"))?
        .len();
    if byte_len > MAX_WORKER_OUTPUT_BYTES_V1 {
        return Err(format!(
            "worker {role} exceeds the {MAX_WORKER_OUTPUT_BYTES_V1}-byte bound"
        ));
    }
    let mut bytes = vec![0_u8; usize::try_from(byte_len).expect("bounded output fits usize")];
    file.read_exact_at(&mut bytes, 0)
        .map_err(|error| format!("cannot read worker {role}: {error}"))?;
    if file
        .metadata()
        .map_err(|error| format!("cannot reinspect worker {role}: {error}"))?
        .len()
        != byte_len
    {
        return Err(format!("worker {role} changed during output admission"));
    }
    Ok(bytes)
}

fn spawn_qualification_worker(
    arguments: &[std::ffi::OsString],
    worker_command: &str,
    output: &WorkerOutputCaptureV1,
) -> Result<Child, String> {
    let mut command = Command::new("/proc/self/exe");
    command
        .arg(worker_command)
        .args(&arguments[2..])
        .env_clear()
        .env(WORKER_PARENT_PID_ENV_V1, std::process::id().to_string())
        .stdin(Stdio::null())
        .stdout(output.stdout_stdio()?)
        .stderr(output.stderr_stdio()?)
        .process_group(0);
    command
        .spawn()
        .map_err(|error| format!("cannot execute /proc/self/exe: {error}"))
}

fn force_terminate_and_reap(child: &mut Child) -> Result<(), String> {
    if let Ok(Some(_)) = child.try_wait() {
        return Ok(());
    }
    match child.kill() {
        Ok(()) => child
            .wait()
            .map(|_| ())
            .map_err(|error| format!("cannot reap killed worker: {error}")),
        Err(kill) => match child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(format!("cannot kill worker: {kill}")),
            Err(wait) => Err(format!(
                "cannot kill worker ({kill}) or confirm its exit ({wait})"
            )),
        },
    }
}

fn finish_supervised_qualification(
    outcome: QualificationWorkerTerminationV1,
    recovery: SupervisedRecoveryV1,
    captured: CapturedWorkerOutputV1,
) {
    match outcome {
        QualificationWorkerTerminationV1::Completed(status) if status.success() => {
            if !recovery.is_already_clean() {
                emit_failure_output(&captured);
                eprintln!(
                    "compiler-execution qualification worker reported success with residual staging: {recovery:?}"
                );
                std::process::exit(1);
            }
            if captured.stdout.is_empty() || !captured.stderr.is_empty() {
                emit_failure_output(&captured);
                eprintln!("compiler-execution qualification worker success output is noncanonical");
                std::process::exit(1);
            }
            if let Err(error) = std::io::stdout()
                .write_all(&captured.stdout)
                .and_then(|()| std::io::stdout().flush())
            {
                eprintln!("compiler-execution qualification report publication failed: {error}");
                std::process::exit(1);
            }
        }
        QualificationWorkerTerminationV1::Completed(status) => {
            emit_failure_output(&captured);
            if !captured.stdout.is_empty() {
                eprintln!(
                    "compiler-execution qualification discarded {} worker stdout bytes after failure",
                    captured.stdout.len()
                );
            }
            std::process::exit(exit_code_for_status(status));
        }
        QualificationWorkerTerminationV1::TimedOut => {
            emit_failure_output(&captured);
            eprintln!("compiler-execution qualification worker timed out after complete recovery");
            std::process::exit(124);
        }
        QualificationWorkerTerminationV1::Signaled(signal) => {
            emit_failure_output(&captured);
            eprintln!(
                "compiler-execution qualification interrupted by signal {signal} after complete recovery"
            );
            exit_for_signal(signal);
        }
    }
}

fn emit_failure_output(captured: &CapturedWorkerOutputV1) {
    let _ = std::io::stderr().write_all(&captured.stderr);
}

fn exit_code_for_status(status: ExitStatus) -> i32 {
    status
        .code()
        .or_else(|| status.signal().map(|signal| (128 + signal).min(255)))
        .unwrap_or(1)
}

fn exit_for_signal(signal: i32) -> ! {
    std::process::exit((128 + signal).min(255))
}

fn establish_worker_parent_boundary() {
    if let Err(error) = establish_worker_parent_boundary_inner() {
        eprintln!("compiler-execution qualification worker boundary failed: {error}");
        std::process::exit(1);
    }
}

fn prepare_qualification_worker(
    arguments: &[std::ffi::OsString],
    request_start: usize,
) -> CompilerExecutionQualificationSupervisorLeaseV1 {
    establish_worker_parent_boundary();
    let Some(manifest_sha256) = arguments[request_start + 1].to_str() else {
        eprintln!("expected manifest SHA-256 must be UTF-8");
        std::process::exit(2);
    };
    let install_parent = Path::new(&arguments[request_start + 3]);
    let qualification_parent = Path::new(&arguments[request_start + 6]);
    let lease = match wait_for_compiler_execution_qualification_supervisor_lease_v1(
        install_parent,
        qualification_parent,
    ) {
        Ok(lease) => lease,
        Err(error) => {
            eprintln!("compiler-execution qualification worker lease failed: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) =
        recover_supervised_state(install_parent, manifest_sha256, qualification_parent)
    {
        eprintln!("compiler-execution qualification worker recovery failed: {error}");
        std::process::exit(1);
    }
    lease
}

fn establish_worker_parent_boundary_inner() -> Result<(), String> {
    establish_exact_parent_boundary(WORKER_PARENT_PID_ENV_V1)
}

fn establish_exact_parent_boundary(environment_name: &str) -> Result<(), String> {
    let expected = std::env::var_os(environment_name)
        .ok_or_else(|| "expected parent PID is missing".to_owned())?;
    let expected = expected
        .to_str()
        .ok_or_else(|| "expected parent PID is not UTF-8".to_owned())?;
    if expected.is_empty()
        || !expected.bytes().all(|byte| byte.is_ascii_digit())
        || (expected.len() > 1 && expected.starts_with('0'))
    {
        return Err("expected parent PID is not canonical decimal".to_owned());
    }
    let expected = expected
        .parse::<i32>()
        .map_err(|_| "expected parent PID overflows".to_owned())?;
    if expected < 2 || observed_parent_pid() != expected {
        return Err("qualification worker parent identity differs before binding".to_owned());
    }
    set_parent_process_death_signal(Some(Signal::KILL))
        .map_err(|error| format!("cannot set parent-death SIGKILL: {error}"))?;
    if observed_parent_pid() != expected {
        return Err("qualification worker parent identity changed while binding".to_owned());
    }
    Ok(())
}

fn run_systemd_preflight_tool(arguments: &[std::ffi::OsString]) {
    if let Err(error) =
        establish_exact_parent_boundary(COMPILER_EXECUTION_SYSTEMD_PREFLIGHT_PARENT_PID_ENV_V1)
    {
        eprintln!("compiler-execution systemd preflight boundary failed: {error}");
        std::process::exit(1);
    }
    let Some(stage) = arguments[2].to_str() else {
        eprintln!("compiler-execution systemd preflight stage must be UTF-8");
        std::process::exit(1);
    };
    match execute_compiler_execution_systemd_preflight_tool_v1(stage) {
        Ok(never) => match never {},
        Err(error) => {
            eprintln!("compiler-execution systemd preflight helper failed: {error}");
            std::process::exit(1);
        }
    }
}

fn observed_parent_pid() -> i32 {
    getppid().map_or(1, |pid| pid.as_raw_pid())
}

fn run_install_recovery(arguments: &[std::ffi::OsString]) {
    let Some(manifest_sha256) = arguments[2].to_str() else {
        eprintln!("expected manifest SHA-256 must be UTF-8");
        std::process::exit(2);
    };
    match recover_compiler_execution_install_parent_v1(Path::new(&arguments[3]), manifest_sha256) {
        Ok(recovery) => {
            let recovery = match recovery {
                CompilerExecutionInstallRecoveryV1::AlreadyClean => "already-clean",
                CompilerExecutionInstallRecoveryV1::Recovered => "recovered",
            };
            println!("recovery_schema=fe2o3-compiler-execution-install-recovery-v1");
            println!("recovery={recovery}");
            println!("cleanup=complete");
        }
        Err(error) => {
            eprintln!("compiler-execution installer recovery failed: {error}");
            std::process::exit(1);
        }
    }
}

fn print_fault_points() {
    for point in QualificationFaultPointV1::all() {
        println!("{}", point.canonical_name());
    }
}

fn run_probe() {
    match probe_compiler_execution_qualification_host_v1() {
        Ok(probe) => print!("{}", probe.canonical_report()),
        Err(error) => {
            eprintln!("compiler-execution qualification host probe failed: {error}");
            std::process::exit(1);
        }
    }
}

fn run_recovery(arguments: &[std::ffi::OsString]) {
    match recover_compiler_execution_qualification_parent_v1(Path::new(&arguments[2])) {
        Ok(recovery) => {
            let recovery = match recovery {
                CompilerExecutionQualificationRecoveryV1::AlreadyEmpty => "already-empty",
                CompilerExecutionQualificationRecoveryV1::Recovered => "recovered",
            };
            println!("recovery_schema=fe2o3-compiler-execution-qualification-recovery-v1");
            println!("recovery={recovery}");
            println!("cleanup=complete");
        }
        Err(error) => {
            eprintln!("compiler-execution qualification recovery failed: {error}");
            std::process::exit(1);
        }
    }
}

fn run_qualification(arguments: &[std::ffi::OsString]) {
    let request = parse_request(arguments, 2);
    match run_compiler_execution_qualification_request_v1(request) {
        Ok(report) => print!("{}", report.canonical_report()),
        Err(error) => {
            eprintln!("compiler-execution qualification failed: {error}");
            std::process::exit(1);
        }
    }
}

fn run_fault(arguments: &[std::ffi::OsString]) {
    let Some(point_name) = arguments[2].to_str() else {
        eprintln!("fault point must be UTF-8");
        std::process::exit(2);
    };
    let Some(point) = QualificationFaultPointV1::from_canonical_name(point_name) else {
        eprintln!("fault point is not one canonical V1 point");
        std::process::exit(2);
    };
    let request = parse_request(arguments, 3);
    match run_compiler_execution_qualification_fault_v1(point, request) {
        Ok(report) => print!("{}", report.canonical_report()),
        Err(error) => {
            eprintln!("compiler-execution fault qualification failed: {error}");
            std::process::exit(1);
        }
    }
}

fn run_campaign(arguments: &[std::ffi::OsString]) {
    let request = parse_request(arguments, 2);
    match run_compiler_execution_qualification_campaign_v1(request) {
        Ok(report) => print!("{}", report.canonical_report()),
        Err(error) => {
            eprintln!("compiler-execution qualification campaign failed: {error}");
            std::process::exit(1);
        }
    }
}

fn parse_request(
    arguments: &[std::ffi::OsString],
    start: usize,
) -> CompilerExecutionQualificationRequestV1<'_> {
    let Some(manifest_sha256) = arguments[start + 1].to_str() else {
        eprintln!("expected manifest SHA-256 must be UTF-8");
        std::process::exit(2);
    };
    let Some(commit) = arguments[start + 2].to_str() else {
        eprintln!("expected git commit must be UTF-8");
        std::process::exit(2);
    };
    let Some(base_sha256) = arguments[start + 5].to_str() else {
        eprintln!("expected base-image SHA-256 must be UTF-8");
        std::process::exit(2);
    };
    CompilerExecutionQualificationRequestV1::new(
        Path::new(&arguments[start]),
        manifest_sha256,
        commit,
        Path::new(&arguments[start + 3]),
        Path::new(&arguments[start + 4]),
        base_sha256,
        Path::new(&arguments[start + 6]),
    )
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::FileExt as _;

    use super::*;

    #[test]
    fn worker_output_capture_round_trips_exact_independent_streams() {
        let output = WorkerOutputCaptureV1::new().unwrap();
        output
            .stdout
            .write_all_at(b"canonical report\n", 0)
            .unwrap();
        output.stderr.write_all_at(b"diagnostic\n", 0).unwrap();

        let captured = output.read().unwrap();
        assert_eq!(captured.stdout, b"canonical report\n");
        assert_eq!(captured.stderr, b"diagnostic\n");
    }

    #[test]
    fn worker_output_capture_rejects_oversized_stream_before_allocation() {
        let output = WorkerOutputCaptureV1::new().unwrap();
        output
            .stdout
            .set_len(MAX_WORKER_OUTPUT_BYTES_V1 + 1)
            .unwrap();

        assert!(output.read().err().unwrap().contains("exceeds"));
    }

    #[test]
    fn only_already_clean_recovery_can_admit_worker_success() {
        let clean = SupervisedRecoveryV1 {
            install: CompilerExecutionInstallRecoveryV1::AlreadyClean,
            qualification: CompilerExecutionQualificationRecoveryV1::AlreadyEmpty,
        };
        let recovered = SupervisedRecoveryV1 {
            install: CompilerExecutionInstallRecoveryV1::Recovered,
            qualification: CompilerExecutionQualificationRecoveryV1::AlreadyEmpty,
        };

        assert!(clean.is_already_clean());
        assert!(!recovered.is_already_clean());
    }

    #[test]
    fn observed_signal_distinguishes_zero_from_registered_signal() {
        let signal = AtomicUsize::new(0);
        assert_eq!(observed_signal(&signal), None);
        signal.store(SIGTERM as usize, Ordering::Release);
        assert_eq!(observed_signal(&signal), Some(SIGTERM));
    }
}
