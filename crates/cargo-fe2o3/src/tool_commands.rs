#[allow(dead_code)]
use crate::pinned_executable::PinnedExecutable;
use sha2::{Digest, Sha256};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs::{File, Metadata};
use std::io::{self, Read, Write as _};
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_DISCOVERY_DIRECTORIES: usize = 64;
const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MAX_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_OUTPUT_LIMIT: usize = 1024 * 1024;
const MAX_OUTPUT_LIMIT: usize = 16 * 1024 * 1024;
const PIPE_DRAIN_GRACE: Duration = Duration::from_millis(250);
const POLL_INTERVAL: Duration = Duration::from_millis(5);
const ALLOWED_ENVIRONMENT: &[&str] = &[
    "AMD_SERIALIZE_COPY",
    "AMD_SERIALIZE_KERNEL",
    "GPU_DEVICE_ORDINAL",
    "HIP_VISIBLE_DEVICES",
    "HOME",
    "HSA_ENABLE_DXG_DETECTION",
    "HSA_ENABLE_INTERRUPT",
    "HSA_ENABLE_SDMA",
    "HSA_ENABLE_SCRATCH_ASYNC_RECLAIM",
    "LANG",
    "LC_ALL",
    "ROCR_VISIBLE_DEVICES",
    "TERM",
    "TMPDIR",
    "XDG_CACHE_HOME",
];

#[cfg(unix)]
const SIG_DFL: usize = 0;
#[cfg(unix)]
const SIG_IGN: usize = 1;
#[cfg(unix)]
const SIG_ERR: usize = usize::MAX;
#[cfg(unix)]
const SIGINT: i32 = 2;
#[cfg(unix)]
const SIGQUIT: i32 = 3;
#[cfg(unix)]
const SIGKILL: i32 = 9;

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
    fn signal(signal: i32, handler: usize) -> usize;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode {
    Sanitize,
    Debug,
}

impl Mode {
    const fn name(self) -> &'static str {
        match self {
            Self::Sanitize => "sanitize",
            Self::Debug => "debug",
        }
    }

    fn usage(self) -> String {
        format!(
            "usage: cargo fe2o3 {} [--tool /absolute/path/to/rocgdb] [--execute] [--timeout-ms N] [--stdout-limit N] [--stderr-limit N] [--cwd /absolute/path]{} -- <program> [arguments...]",
            self.name(),
            match self {
                Self::Sanitize => " [--coverage precise-memory|race|api]",
                Self::Debug => " [--batch|--interactive]",
            }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Plan,
    Execute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DebugSession {
    Batch,
    Interactive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Coverage {
    PreciseMemory,
    Race,
    Api,
}

impl Coverage {
    const fn name(self) -> &'static str {
        match self {
            Self::PreciseMemory => "precise-memory",
            Self::Race => "race",
            Self::Api => "api",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    action: Action,
    action_explicit: bool,
    tool_override: Option<PathBuf>,
    program: String,
    program_arguments: Vec<String>,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    working_directory: Option<PathBuf>,
    debug_session: DebugSession,
    debug_session_explicit: bool,
    coverage: Coverage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiscoveryInput {
    rocm_roots: Vec<PathBuf>,
    path_directories: Vec<PathBuf>,
}

impl DiscoveryInput {
    fn capture() -> Self {
        let mut rocm_roots = Vec::new();
        for variable in ["ROCM_PATH", "HIP_PATH"] {
            if let Some(path) = env::var_os(variable).map(PathBuf::from)
                && path.is_absolute()
            {
                push_unique(&mut rocm_roots, path);
            }
        }
        for path in ["/opt/rocm", "/opt/rocm-7.2.0", "/opt/rocm-7.1.0"] {
            push_unique(&mut rocm_roots, PathBuf::from(path));
        }

        let mut path_directories = Vec::new();
        if let Some(value) = env::var_os("PATH") {
            for path in env::split_paths(&value)
                .filter(|path| path.is_absolute())
                .take(MAX_DISCOVERY_DIRECTORIES)
            {
                push_unique(&mut path_directories, path);
            }
        }
        Self {
            rocm_roots,
            path_directories,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InvocationPlan {
    mode: Mode,
    session: DebugSession,
    coverage: Coverage,
    executable: PathBuf,
    program: String,
    program_arguments: Vec<String>,
    arguments: Vec<String>,
}

impl InvocationPlan {
    fn render(&self, options: &Options, working_directory: &Path) -> String {
        let mut output = String::new();
        writeln!(output, "schema: fe2o3-tool-plan-v2").expect("write to String");
        writeln!(output, "mode: {}", self.mode.name()).expect("write to String");
        writeln!(output, "authority: plan-only").expect("write to String");
        match self.mode {
            Mode::Sanitize => {
                writeln!(output, "backend: rocgdb-precise-memory").expect("write to String");
                writeln!(output, "coverage-requested: {}", self.coverage.name())
                    .expect("write to String");
                writeln!(output, "coverage: gpu-memory-fault-location-only")
                    .expect("write to String");
                writeln!(output, "coverage-precise-memory: diagnostic").expect("write to String");
                writeln!(output, "coverage-race: unsupported").expect("write to String");
                writeln!(output, "coverage-api: unsupported").expect("write to String");
                writeln!(
                    output,
                    "not-covered: data-races,uninitialized-memory,synchronization-errors,host-api-misuse"
                )
                .expect("write to String");
                writeln!(
                    output,
                    "coverage-interpretation: successful-execution-is-not-proof-of-memory-safety"
                )
                .expect("write to String");
            }
            Mode::Debug => {
                let backend = match self.session {
                    DebugSession::Batch => "rocgdb-batch",
                    DebugSession::Interactive => "rocgdb-interactive",
                };
                writeln!(output, "backend: {backend}").expect("write to String");
                writeln!(output, "coverage: debugger-launch-only").expect("write to String");
                writeln!(
                    output,
                    "not-covered: source-map-generation,local-layout-validation"
                )
                .expect("write to String");
            }
        }
        writeln!(output, "executable: {:?}", self.executable).expect("write to String");
        writeln!(output, "program: {:?}", self.program).expect("write to String");
        for (index, argument) in self.program_arguments.iter().enumerate() {
            writeln!(output, "program-arg[{index}]: {argument:?}").expect("write to String");
        }
        writeln!(output, "working-directory: {working_directory:?}").expect("write to String");
        writeln!(output, "timeout-ms: {}", options.timeout.as_millis()).expect("write to String");
        writeln!(output, "stdout-limit: {}", options.stdout_limit).expect("write to String");
        writeln!(output, "stderr-limit: {}", options.stderr_limit).expect("write to String");
        writeln!(output, "environment-policy: clear-then-fixed-allowlist")
            .expect("write to String");
        for (index, argument) in self.arguments.iter().enumerate() {
            writeln!(output, "arg[{index}]: {argument:?}").expect("write to String");
        }
        output.pop();
        output
    }
}

#[derive(Debug)]
pub(crate) struct CommandReport {
    output: String,
    succeeded: bool,
}

impl CommandReport {
    pub(crate) fn output(&self) -> &str {
        &self.output
    }

    pub(crate) const fn succeeded(&self) -> bool {
        self.succeeded
    }
}

pub(crate) fn command(mode: Mode, args: &[String]) -> Result<CommandReport, String> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        return Ok(CommandReport {
            output: mode.usage(),
            succeeded: true,
        });
    }
    let options = parse_options(mode, args)?;
    validate_requested_capability(mode, &options)?;
    let selected = match options.tool_override.as_deref() {
        Some(path) => validate_override(mode, path)?,
        None => discover_rocgdb(&DiscoveryInput::capture()).ok_or_else(|| unavailable(mode))?,
    };
    let working_directory = resolve_working_directory(options.working_directory.as_deref())?;
    let plan = build_plan(
        mode,
        options.debug_session,
        options.coverage,
        selected,
        options.program.clone(),
        options.program_arguments.clone(),
    );
    if options.action == Action::Plan {
        return Ok(CommandReport {
            output: plan.render(&options, &working_directory),
            succeeded: true,
        });
    }
    execute(plan, &options, working_directory)
}

fn parse_options(mode: Mode, args: &[String]) -> Result<Options, String> {
    let separator = args
        .iter()
        .position(|argument| argument == "--")
        .ok_or_else(|| {
            format!(
                "{} requires `--` before the program\n{}",
                mode.name(),
                mode.usage()
            )
        })?;
    let mut options = Options {
        action: Action::Plan,
        action_explicit: false,
        tool_override: None,
        program: String::new(),
        program_arguments: Vec::new(),
        timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
        stdout_limit: DEFAULT_OUTPUT_LIMIT,
        stderr_limit: DEFAULT_OUTPUT_LIMIT,
        working_directory: None,
        debug_session: if mode == Mode::Sanitize {
            DebugSession::Batch
        } else {
            DebugSession::Interactive
        },
        debug_session_explicit: false,
        coverage: Coverage::PreciseMemory,
    };

    let mut index = 0;
    while index < separator {
        let argument = &args[index];
        match argument.as_str() {
            "--execute" => set_action(&mut options, Action::Execute)?,
            "--print-plan" => set_action(&mut options, Action::Plan)?,
            "--batch" => set_debug_session(mode, &mut options, DebugSession::Batch)?,
            "--interactive" => {
                set_debug_session(mode, &mut options, DebugSession::Interactive)?;
            }
            _ => {
                if let Some(value) = option_value(args, &mut index, separator, "--tool")? {
                    if value.is_empty() {
                        return Err("--tool path must not be empty".to_string());
                    }
                    if options
                        .tool_override
                        .replace(PathBuf::from(value))
                        .is_some()
                    {
                        return Err("--tool was specified more than once".to_string());
                    }
                } else if let Some(value) =
                    option_value(args, &mut index, separator, "--timeout-ms")?
                {
                    let milliseconds = parse_bounded_u64("--timeout-ms", value, 1, MAX_TIMEOUT_MS)?;
                    options.timeout = Duration::from_millis(milliseconds);
                } else if let Some(value) =
                    option_value(args, &mut index, separator, "--stdout-limit")?
                {
                    options.stdout_limit =
                        parse_bounded_usize("--stdout-limit", value, 1, MAX_OUTPUT_LIMIT)?;
                } else if let Some(value) =
                    option_value(args, &mut index, separator, "--stderr-limit")?
                {
                    options.stderr_limit =
                        parse_bounded_usize("--stderr-limit", value, 1, MAX_OUTPUT_LIMIT)?;
                } else if let Some(value) = option_value(args, &mut index, separator, "--cwd")? {
                    if options
                        .working_directory
                        .replace(PathBuf::from(value))
                        .is_some()
                    {
                        return Err("--cwd was specified more than once".to_string());
                    }
                } else if let Some(value) = option_value(args, &mut index, separator, "--coverage")?
                {
                    if mode != Mode::Sanitize {
                        return Err("--coverage is valid only for sanitize".to_string());
                    }
                    options.coverage = match value {
                        "precise-memory" => Coverage::PreciseMemory,
                        "race" => Coverage::Race,
                        "api" => Coverage::Api,
                        other => {
                            return Err(format!(
                                "unsupported sanitizer coverage `{other}`; supported request names are precise-memory, race, and api"
                            ));
                        }
                    };
                } else {
                    return Err(format!(
                        "unknown {} option `{argument}`\n{}",
                        mode.name(),
                        mode.usage()
                    ));
                }
            }
        }
        index += 1;
    }

    options.program = args
        .get(separator + 1)
        .filter(|program| !program.is_empty())
        .cloned()
        .ok_or_else(|| {
            format!(
                "{} requires a nonempty program\n{}",
                mode.name(),
                mode.usage()
            )
        })?;
    options.program_arguments = args[separator + 2..].to_vec();
    Ok(options)
}

fn option_value<'a>(
    args: &'a [String],
    index: &mut usize,
    separator: usize,
    name: &str,
) -> Result<Option<&'a str>, String> {
    let argument = &args[*index];
    if argument == name {
        *index += 1;
        if *index >= separator {
            return Err(format!("{name} requires a value before `--`"));
        }
        return Ok(Some(args[*index].as_str()));
    }
    Ok(argument
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('=')))
}

fn set_action(options: &mut Options, requested: Action) -> Result<(), String> {
    if options.action_explicit {
        if options.action != requested {
            return Err("--execute and --print-plan are mutually exclusive".to_string());
        }
        return Err(format!(
            "{} was specified more than once",
            if requested == Action::Execute {
                "--execute"
            } else {
                "--print-plan"
            }
        ));
    }
    options.action = requested;
    options.action_explicit = true;
    Ok(())
}

fn set_debug_session(
    mode: Mode,
    options: &mut Options,
    requested: DebugSession,
) -> Result<(), String> {
    if mode != Mode::Debug {
        return Err("--batch and --interactive are valid only for debug".to_string());
    }
    if options.debug_session_explicit {
        return Err("debug session mode was specified more than once".to_string());
    }
    options.debug_session = requested;
    options.debug_session_explicit = true;
    Ok(())
}

fn parse_bounded_u64(name: &str, value: &str, minimum: u64, maximum: u64) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{name} must be an integer"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!("{name} must be between {minimum} and {maximum}"));
    }
    Ok(parsed)
}

fn parse_bounded_usize(
    name: &str,
    value: &str,
    minimum: usize,
    maximum: usize,
) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{name} must be an integer"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!("{name} must be between {minimum} and {maximum}"));
    }
    Ok(parsed)
}

fn validate_requested_capability(mode: Mode, options: &Options) -> Result<(), String> {
    if mode == Mode::Sanitize {
        match options.coverage {
            Coverage::PreciseMemory => {}
            Coverage::Race => {
                return Err("required race coverage is unavailable: reviewed ROCgdb precise-memory mode does not implement a data-race checker".to_string());
            }
            Coverage::Api => {
                return Err("required API coverage is unavailable: reviewed ROCgdb precise-memory mode does not implement a HIP/ROCm API checker".to_string());
            }
        }
    }
    if mode == Mode::Debug && options.action == Action::Execute && !options.debug_session_explicit {
        return Err(
            "debug execution requires an explicit --batch or --interactive session mode"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_override(mode: Mode, path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "{} --tool must be an absolute path to rocgdb",
            mode.name()
        ));
    }
    if !is_reviewed_tool_name(path.file_name()) {
        return Err(format!(
            "{} supports only reviewed ROCgdb executables named rocgdb, rocgdb-py_3.12, or rocgdb-py_3.13; got {}",
            mode.name(),
            path.display()
        ));
    }
    if !is_executable_file(path) {
        return Err(format!(
            "{} ROCgdb tool is unavailable or not executable: {}",
            mode.name(),
            path.display()
        ));
    }
    Ok(path.to_path_buf())
}

fn is_reviewed_tool_name(name: Option<&OsStr>) -> bool {
    matches!(
        name.and_then(OsStr::to_str),
        Some("rocgdb" | "rocgdb-py_3.12" | "rocgdb-py_3.13")
    )
}

fn discover_rocgdb(input: &DiscoveryInput) -> Option<PathBuf> {
    discover_with(input, is_executable_file)
}

fn discover_with(
    input: &DiscoveryInput,
    mut available: impl FnMut(&Path) -> bool,
) -> Option<PathBuf> {
    for root in &input.rocm_roots {
        for relative in [
            "bin/rocgdb-py_3.12",
            "bin/rocgdb-py_3.13",
            "bin/rocgdb",
            "lib/llvm/bin/rocgdb",
        ] {
            let candidate = root.join(relative);
            if available(&candidate) {
                return Some(candidate);
            }
        }
    }
    for directory in &input.path_directories {
        let candidate = directory.join("rocgdb");
        if available(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn build_plan(
    mode: Mode,
    session: DebugSession,
    coverage: Coverage,
    executable: PathBuf,
    program: String,
    program_arguments: Vec<String>,
) -> InvocationPlan {
    let mut arguments = [
        "--quiet",
        "--nx",
        "--nh",
        "-ex",
        "set auto-load off",
        "-ex",
        "set pagination off",
        "-ex",
        "set confirm off",
        "-ex",
        "set startup-with-shell off",
    ]
    .map(str::to_string)
    .to_vec();
    match (mode, session) {
        (Mode::Sanitize, _) => {
            arguments.push("--batch".into());
            append_command(&mut arguments, "set amdgpu precise-memory on");
            append_batch_commands(&mut arguments);
        }
        (Mode::Debug, DebugSession::Batch) => {
            arguments.push("--batch".into());
            append_batch_commands(&mut arguments);
        }
        (Mode::Debug, DebugSession::Interactive) => {}
    }
    arguments.push("--args".into());
    arguments.push(program.clone());
    arguments.extend(program_arguments.iter().cloned());
    InvocationPlan {
        mode,
        session,
        coverage,
        executable,
        program,
        program_arguments,
        arguments,
    }
}

fn append_command(arguments: &mut Vec<String>, command: &str) {
    arguments.push("-ex".into());
    arguments.push(command.into());
}

fn append_batch_commands(arguments: &mut Vec<String>) {
    for command in [
        "run",
        "thread apply all backtrace",
        "echo FE2O3_TARGET_EXIT_CODE=",
        "output $_exitcode",
        "echo \\n",
        "echo FE2O3_TARGET_EXIT_SIGNAL=",
        "output $_exitsignal",
        "echo \\n",
    ] {
        append_command(arguments, command);
    }
}

fn unavailable(mode: Mode) -> String {
    match mode {
        Mode::Sanitize => "cargo fe2o3 sanitize unavailable: a reviewed ROCgdb executable was not found in ROCM_PATH, HIP_PATH, supported /opt/rocm roots, or absolute PATH entries; ROCgdb precise-memory mode can improve GPU memory-fault location but does not detect races, uninitialized memory, synchronization errors, or API misuse".to_string(),
        Mode::Debug => "cargo fe2o3 debug unavailable: a reviewed ROCgdb executable was not found in ROCM_PATH, HIP_PATH, supported /opt/rocm roots, or absolute PATH entries".to_string(),
    }
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
}

fn resolve_working_directory(requested: Option<&Path>) -> Result<PathBuf, String> {
    let path = match requested {
        Some(path) => {
            if !path.is_absolute() {
                return Err("--cwd must be an absolute path".to_string());
            }
            path.to_path_buf()
        }
        None => env::current_dir()
            .map_err(|error| format!("failed to read current directory: {error}"))?,
    };
    let canonical = std::fs::canonicalize(&path).map_err(|error| {
        format!(
            "failed to resolve working directory {}: {error}",
            path.display()
        )
    })?;
    if !canonical.is_dir() {
        return Err(format!(
            "working directory is not a directory: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;

    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectIdentity {
    device: u64,
    inode: u64,
    mode: u32,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[cfg(unix)]
impl ObjectIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            size: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

struct ExecutionPin {
    _validated: PinnedExecutable,
    file: File,
    canonical_path: PathBuf,
    identity: ObjectIdentity,
    sha256: [u8; 32],
}

impl ExecutionPin {
    #[cfg(target_os = "linux")]
    fn open(path: &Path, kind: &str) -> Result<Self, String> {
        let link_metadata = std::fs::symlink_metadata(path)
            .map_err(|error| format!("failed to inspect {kind} {}: {error}", path.display()))?;
        if link_metadata.file_type().is_symlink() {
            return Err(format!(
                "{kind} path must not be a symbolic link: {}",
                path.display()
            ));
        }
        let canonical_path = std::fs::canonicalize(path).map_err(|error| {
            format!("failed to canonicalize {kind} {}: {error}", path.display())
        })?;
        let before = ObjectIdentity::from_metadata(&std::fs::metadata(&canonical_path).map_err(
            |error| {
                format!(
                    "failed to inspect {kind} {}: {error}",
                    canonical_path.display()
                )
            },
        )?);
        let validated = PinnedExecutable::open(&canonical_path)
            .map_err(|error| format!("failed to pin {kind}: {error}"))?;
        let sha256 = *validated.sha256();
        if validated.size() != before.size {
            return Err(format!(
                "{kind} changed while its executable identity was measured"
            ));
        }
        let fd = rustix::fs::open(
            &canonical_path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|error| {
            format!(
                "failed to retain {kind} {}: {error}",
                canonical_path.display()
            )
        })?;
        let file = File::from(fd);
        let identity = ObjectIdentity::from_metadata(
            &file
                .metadata()
                .map_err(|error| format!("failed to inspect retained {kind}: {error}"))?,
        );
        if identity != before {
            return Err(format!(
                "{kind} changed between validation and descriptor pinning"
            ));
        }
        ensure_elf(&file, kind)?;
        Ok(Self {
            _validated: validated,
            file,
            canonical_path,
            identity,
            sha256,
        })
    }

    #[cfg(not(target_os = "linux"))]
    fn open(_path: &Path, _kind: &str) -> Result<Self, String> {
        Err("bounded tool execution requires Linux descriptor-backed executable pinning".into())
    }

    fn validate_path(&self, kind: &str) -> Result<(), String> {
        let current = std::fs::metadata(&self.canonical_path)
            .map_err(|error| format!("{kind} path changed or disappeared: {error}"))?;
        if ObjectIdentity::from_metadata(&current) != self.identity {
            return Err(format!(
                "{kind} path changed after planning: {}",
                self.canonical_path.display()
            ));
        }
        let descriptor = self
            .file
            .metadata()
            .map_err(|error| format!("retained {kind} descriptor changed: {error}"))?;
        if ObjectIdentity::from_metadata(&descriptor) != self.identity {
            return Err(format!("retained {kind} executable changed after planning"));
        }
        Ok(())
    }

    fn execution_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.file.as_raw_fd()))
    }

    fn external_execution_path(&self) -> PathBuf {
        PathBuf::from(format!(
            "/proc/{}/fd/{}",
            std::process::id(),
            self.file.as_raw_fd()
        ))
    }
}

fn ensure_elf(file: &File, kind: &str) -> Result<(), String> {
    let mut reader = file
        .try_clone()
        .map_err(|error| format!("failed to clone retained {kind} descriptor: {error}"))?;
    let mut magic = [0_u8; 4];
    reader
        .read_exact(&mut magic)
        .map_err(|error| format!("failed to read retained {kind} header: {error}"))?;
    if magic != *b"\x7fELF" {
        return Err(format!(
            "{kind} must be a native ELF executable; shell and interpreter wrappers are not executed"
        ));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct EnvironmentBinding {
    name: &'static str,
    value: OsString,
}

fn capture_environment() -> Vec<EnvironmentBinding> {
    ALLOWED_ENVIRONMENT
        .iter()
        .filter_map(|&name| env::var_os(name).map(|value| EnvironmentBinding { name, value }))
        .collect()
}

#[derive(Default)]
struct Capture {
    bytes: Vec<u8>,
    overflow: bool,
    eof: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StopReason {
    Exited,
    Timeout,
    OutputOverflow,
    WaitFailure,
}

struct SupervisedResult {
    status: Option<ExitStatus>,
    stdout: Capture,
    stderr: Capture,
    reason: StopReason,
    wait_error: Option<String>,
}

fn execute(
    plan: InvocationPlan,
    options: &Options,
    working_directory: PathBuf,
) -> Result<CommandReport, String> {
    let tool_path = resolve_native_tool(&plan.executable)?;
    let tool = ExecutionPin::open(&tool_path, "ROCgdb executable")?;
    let requested_program = PathBuf::from(&plan.program);
    let program_path = if requested_program.is_absolute() {
        requested_program
    } else {
        working_directory.join(requested_program)
    };
    let target = ExecutionPin::open(&program_path, "target program")?;
    tool.validate_path("ROCgdb executable")?;
    target.validate_path("target program")?;

    let environment = capture_environment();
    let invocation_digest = invocation_digest(
        &plan,
        options,
        &working_directory,
        &tool,
        &target,
        &environment,
    );
    let mut actual_plan = plan.clone();
    actual_plan.arguments = build_plan(
        plan.mode,
        plan.session,
        plan.coverage,
        tool.canonical_path.clone(),
        target
            .external_execution_path()
            .to_string_lossy()
            .into_owned(),
        plan.program_arguments.clone(),
    )
    .arguments;

    let mut command = Command::new(tool.execution_path());
    command
        .arg0(&tool.canonical_path)
        .args(&actual_plan.arguments)
        .current_dir(&working_directory)
        .env_clear()
        .envs(environment.iter().map(|entry| (entry.name, &entry.value)))
        .stdin(
            if plan.session == DebugSession::Interactive && plan.mode == Mode::Debug {
                Stdio::inherit()
            } else {
                Stdio::null()
            },
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let interactive = plan.mode == Mode::Debug && plan.session == DebugSession::Interactive;
    let _signal_guard = if interactive {
        Some(InteractiveSignalGuard::install(&mut command)?)
    } else {
        command.process_group(0);
        None
    };

    tool.validate_path("ROCgdb executable")?;
    target.validate_path("target program")?;
    let mut child = crate::process_execution::spawn(&mut command)
        .map_err(|error| format!("failed to spawn pinned ROCgdb executable: {error}"))?;
    let result = supervise(
        &mut child,
        options.timeout,
        options.stdout_limit,
        options.stderr_limit,
        interactive,
        !interactive,
    );
    let tool_changed = tool.validate_path("ROCgdb executable").err();
    let target_changed = target.validate_path("target program").err();
    Ok(render_evidence(
        EvidenceContext {
            plan: &plan,
            options,
            working_directory: &working_directory,
            tool: &tool,
            target: &target,
            environment: &environment,
            invocation_digest,
        },
        result,
        IdentityErrors {
            tool: tool_changed,
            target: target_changed,
        },
    ))
}

fn resolve_native_tool(selected: &Path) -> Result<PathBuf, String> {
    if has_elf_magic(selected) {
        return Ok(selected.to_path_buf());
    }
    if selected.file_name() == Some(OsStr::new("rocgdb")) {
        let directory = selected
            .parent()
            .ok_or_else(|| "ROCgdb wrapper has no parent directory".to_string())?;
        for name in ["rocgdb-py_3.12", "rocgdb-py_3.13"] {
            let candidate = directory.join(name);
            if is_executable_file(&candidate) && has_elf_magic(&candidate) {
                return Ok(candidate);
            }
        }
    }
    Err(format!(
        "reviewed native ROCgdb executable unavailable at {}; wrapper scripts are not executed",
        selected.display()
    ))
}

fn has_elf_magic(path: &Path) -> bool {
    File::open(path).is_ok_and(|mut file| {
        let mut magic = [0_u8; 4];
        file.read_exact(&mut magic).is_ok() && magic == *b"\x7fELF"
    })
}

fn supervise(
    child: &mut Child,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    interactive: bool,
    isolated_process_group: bool,
) -> SupervisedResult {
    let mut stdout = Capture::default();
    let mut stderr = Capture::default();
    let mut stdout_pipe = child.stdout.take().expect("stdout was configured as piped");
    let mut stderr_pipe = child.stderr.take().expect("stderr was configured as piped");
    if let Err(error) = make_nonblocking(&stdout_pipe).and_then(|()| make_nonblocking(&stderr_pipe))
    {
        terminate_process_tree(child, isolated_process_group);
        let _ = child.wait();
        return SupervisedResult {
            status: None,
            stdout,
            stderr,
            reason: StopReason::WaitFailure,
            wait_error: Some(format!(
                "failed to configure bounded output capture: {error}"
            )),
        };
    }
    let started = Instant::now();
    let (reason, status, wait_error) = loop {
        drain_pipe(
            &mut stdout_pipe,
            &mut stdout,
            stdout_limit,
            interactive,
            false,
        );
        drain_pipe(
            &mut stderr_pipe,
            &mut stderr,
            stderr_limit,
            interactive,
            true,
        );
        if stdout.overflow || stderr.overflow {
            terminate_process_tree(child, isolated_process_group);
            break (StopReason::OutputOverflow, child.wait().ok(), None);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                terminate_process_tree(child, isolated_process_group);
                break (StopReason::Exited, Some(status), None);
            }
            Ok(None) if started.elapsed() >= timeout => {
                terminate_process_tree(child, isolated_process_group);
                break (StopReason::Timeout, child.wait().ok(), None);
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => {
                terminate_process_tree(child, isolated_process_group);
                let _ = child.wait();
                break (StopReason::WaitFailure, None, Some(error.to_string()));
            }
        }
    };

    let drain_deadline = Instant::now() + PIPE_DRAIN_GRACE;
    while (!stdout.eof || !stderr.eof) && Instant::now() < drain_deadline {
        drain_pipe(
            &mut stdout_pipe,
            &mut stdout,
            stdout_limit,
            interactive,
            false,
        );
        drain_pipe(
            &mut stderr_pipe,
            &mut stderr,
            stderr_limit,
            interactive,
            true,
        );
        if !stdout.eof || !stderr.eof {
            thread::sleep(POLL_INTERVAL);
        }
    }
    SupervisedResult {
        status,
        stdout,
        stderr,
        reason,
        wait_error,
    }
}

fn make_nonblocking(fd: &impl std::os::fd::AsFd) -> io::Result<()> {
    let flags = rustix::fs::fcntl_getfl(fd)?;
    rustix::fs::fcntl_setfl(fd, flags | rustix::fs::OFlags::NONBLOCK)?;
    Ok(())
}

fn drain_pipe<R: Read>(
    pipe: &mut R,
    capture: &mut Capture,
    limit: usize,
    live: bool,
    is_stderr: bool,
) {
    let mut buffer = [0_u8; 8192];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => {
                capture.eof = true;
                return;
            }
            Ok(read) => {
                if live {
                    let result = if is_stderr {
                        io::stderr().write_all(&buffer[..read])
                    } else {
                        io::stdout().write_all(&buffer[..read])
                    };
                    let _ = result;
                }
                let remaining = limit.saturating_sub(capture.bytes.len());
                capture
                    .bytes
                    .extend_from_slice(&buffer[..read.min(remaining)]);
                if read > remaining {
                    capture.overflow = true;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                capture
                    .bytes
                    .extend_from_slice(format!("\n[fe2o3 capture error: {error}]").as_bytes());
                capture.overflow = true;
                return;
            }
        }
    }
}

#[cfg(unix)]
fn terminate_process_tree(child: &mut Child, isolated_process_group: bool) {
    let pid = i32::try_from(child.id()).unwrap_or(i32::MAX);
    for descendant in descendants(child.id()).into_iter().rev() {
        let descendant = i32::try_from(descendant).unwrap_or(i32::MAX);
        let _ = unsafe { kill(descendant, SIGKILL) };
    }
    if isolated_process_group {
        // Batch children are placed in a fresh process group whose id is their pid.
        let _ = unsafe { kill(-pid, SIGKILL) };
    }
    let _ = child.kill();
}

#[cfg(not(unix))]
fn terminate_process_tree(child: &mut Child, _isolated_process_group: bool) {
    let _ = child.kill();
}

#[cfg(target_os = "linux")]
fn descendants(root: u32) -> Vec<u32> {
    const MAX_DESCENDANTS: usize = 4096;
    let mut found = Vec::new();
    let mut pending = vec![root];
    while let Some(parent) = pending.pop() {
        if found.len() >= MAX_DESCENDANTS {
            break;
        }
        let path = format!("/proc/{parent}/task/{parent}/children");
        let Ok(children) = std::fs::read_to_string(path) else {
            continue;
        };
        for child in children.split_ascii_whitespace() {
            if let Ok(child) = child.parse::<u32>()
                && child != root
                && !found.contains(&child)
            {
                found.push(child);
                pending.push(child);
            }
        }
    }
    found
}

#[cfg(all(unix, not(target_os = "linux")))]
fn descendants(_root: u32) -> Vec<u32> {
    Vec::new()
}

struct InteractiveSignalGuard {
    #[cfg(unix)]
    previous_interrupt: usize,
    #[cfg(unix)]
    previous_quit: usize,
}

impl InteractiveSignalGuard {
    #[cfg(unix)]
    fn install(command: &mut Command) -> Result<Self, String> {
        let previous_interrupt = unsafe { signal(SIGINT, SIG_IGN) };
        if previous_interrupt == SIG_ERR {
            return Err(format!(
                "failed to isolate interactive SIGINT handling: {}",
                io::Error::last_os_error()
            ));
        }
        let previous_quit = unsafe { signal(SIGQUIT, SIG_IGN) };
        if previous_quit == SIG_ERR {
            let _ = unsafe { signal(SIGINT, previous_interrupt) };
            return Err(format!(
                "failed to isolate interactive SIGQUIT handling: {}",
                io::Error::last_os_error()
            ));
        }
        unsafe {
            command.pre_exec(|| {
                if signal(SIGINT, SIG_DFL) == SIG_ERR || signal(SIGQUIT, SIG_DFL) == SIG_ERR {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        Ok(Self {
            previous_interrupt,
            previous_quit,
        })
    }

    #[cfg(not(unix))]
    fn install(_command: &mut Command) -> Result<Self, String> {
        Err("interactive debugger execution requires Unix signal isolation".to_string())
    }
}

impl Drop for InteractiveSignalGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let _ = unsafe { signal(SIGINT, self.previous_interrupt) };
            let _ = unsafe { signal(SIGQUIT, self.previous_quit) };
        }
    }
}

fn invocation_digest(
    plan: &InvocationPlan,
    options: &Options,
    working_directory: &Path,
    tool: &ExecutionPin,
    target: &ExecutionPin,
    environment: &[EnvironmentBinding],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"fe2o3-tool-invocation-v2");
    hash_field(&mut hasher, plan.mode.name().as_bytes());
    hash_field(&mut hasher, debug_session_name(plan.session).as_bytes());
    hash_field(&mut hasher, plan.coverage.name().as_bytes());
    hash_field(&mut hasher, &tool.sha256);
    hash_path(&mut hasher, &tool.canonical_path);
    hash_field(&mut hasher, &target.sha256);
    hash_path(&mut hasher, &target.canonical_path);
    hash_field(&mut hasher, plan.program.as_bytes());
    hash_path(&mut hasher, working_directory);
    hash_field(&mut hasher, &options.timeout.as_millis().to_le_bytes());
    hash_field(&mut hasher, &options.stdout_limit.to_le_bytes());
    hash_field(&mut hasher, &options.stderr_limit.to_le_bytes());
    for argument in &plan.arguments {
        hash_field(&mut hasher, argument.as_bytes());
    }
    for entry in environment {
        hash_field(&mut hasher, entry.name.as_bytes());
        hash_os_string(&mut hasher, &entry.value);
    }
    hasher.finalize().into()
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(unix)]
fn hash_path(hasher: &mut Sha256, path: &Path) {
    use std::os::unix::ffi::OsStrExt as _;
    hash_field(hasher, path.as_os_str().as_bytes());
}

#[cfg(not(unix))]
fn hash_path(hasher: &mut Sha256, path: &Path) {
    hash_field(hasher, path.to_string_lossy().as_bytes());
}

#[cfg(unix)]
fn hash_os_string(hasher: &mut Sha256, value: &OsStr) {
    use std::os::unix::ffi::OsStrExt as _;
    hash_field(hasher, value.as_bytes());
}

#[cfg(not(unix))]
fn hash_os_string(hasher: &mut Sha256, value: &OsStr) {
    hash_field(hasher, value.to_string_lossy().as_bytes());
}

struct EvidenceContext<'a> {
    plan: &'a InvocationPlan,
    options: &'a Options,
    working_directory: &'a Path,
    tool: &'a ExecutionPin,
    target: &'a ExecutionPin,
    environment: &'a [EnvironmentBinding],
    invocation_digest: [u8; 32],
}

struct IdentityErrors {
    tool: Option<String>,
    target: Option<String>,
}

fn render_evidence(
    context: EvidenceContext<'_>,
    result: SupervisedResult,
    identity_errors: IdentityErrors,
) -> CommandReport {
    let EvidenceContext {
        plan,
        options,
        working_directory,
        tool,
        target,
        environment,
        invocation_digest,
    } = context;
    let stdout_digest: [u8; 32] = Sha256::digest(&result.stdout.bytes).into();
    let stderr_digest: [u8; 32] = Sha256::digest(&result.stderr.bytes).into();
    let combined = [
        result.stdout.bytes.as_slice(),
        result.stderr.bytes.as_slice(),
    ]
    .concat();
    let target_status = parse_target_status(&combined);
    let memory_diagnostic = contains_memory_diagnostic(&combined);
    let coverage_unavailable = precise_memory_unavailable(&combined);
    let (outcome, succeeded) = classify_outcome(
        plan,
        &result,
        target_status.as_ref(),
        memory_diagnostic,
        coverage_unavailable,
        identity_errors.tool.is_some(),
        identity_errors.target.is_some(),
    );

    let mut output = String::new();
    writeln!(output, "schema: fe2o3-tool-evidence-v2").expect("write to String");
    writeln!(output, "authority: diagnostic-only").expect("write to String");
    writeln!(output, "mode: {}", plan.mode.name()).expect("write to String");
    writeln!(output, "session: {}", debug_session_name(plan.session)).expect("write to String");
    writeln!(output, "outcome: {outcome}").expect("write to String");
    writeln!(output, "invocation-sha256: {}", hex(&invocation_digest)).expect("write to String");
    writeln!(output, "tool-path: {:?}", tool.canonical_path).expect("write to String");
    writeln!(output, "tool-sha256: {}", hex(&tool.sha256)).expect("write to String");
    writeln!(output, "tool-size: {}", tool.identity.size).expect("write to String");
    writeln!(output, "target-path: {:?}", target.canonical_path).expect("write to String");
    writeln!(output, "target-requested: {:?}", plan.program).expect("write to String");
    writeln!(output, "target-sha256: {}", hex(&target.sha256)).expect("write to String");
    writeln!(output, "target-size: {}", target.identity.size).expect("write to String");
    writeln!(output, "target-execution-binding: retained-descriptor").expect("write to String");
    for (index, argument) in plan.program_arguments.iter().enumerate() {
        writeln!(output, "target-arg[{index}]: {argument:?}").expect("write to String");
    }
    writeln!(output, "tool-argv-policy: fixed-v2-plus-exact-target-argv").expect("write to String");
    for (index, argument) in plan.arguments.iter().enumerate() {
        writeln!(output, "tool-arg[{index}]: {argument:?}").expect("write to String");
    }
    writeln!(output, "working-directory: {working_directory:?}").expect("write to String");
    writeln!(output, "timeout-ms: {}", options.timeout.as_millis()).expect("write to String");
    writeln!(output, "stdout-limit: {}", options.stdout_limit).expect("write to String");
    writeln!(output, "stderr-limit: {}", options.stderr_limit).expect("write to String");
    writeln!(output, "environment-policy: clear-then-fixed-allowlist").expect("write to String");
    for (index, entry) in environment.iter().enumerate() {
        let mut hasher = Sha256::new();
        hash_os_string(&mut hasher, &entry.value);
        let digest: [u8; 32] = hasher.finalize().into();
        writeln!(
            output,
            "env[{index}]: {} sha256={}",
            entry.name,
            hex(&digest)
        )
        .expect("write to String");
    }
    render_exit_status(&mut output, result.status.as_ref());
    if let Some(status) = &target_status {
        writeln!(output, "target-status: {}", status.render()).expect("write to String");
    } else {
        writeln!(output, "target-status: unavailable").expect("write to String");
    }
    if plan.mode == Mode::Sanitize {
        writeln!(output, "coverage-requested: {}", plan.coverage.name()).expect("write to String");
        writeln!(output, "coverage-precise-memory: diagnostic").expect("write to String");
        writeln!(output, "coverage-race: unsupported").expect("write to String");
        writeln!(output, "coverage-api: unsupported").expect("write to String");
        writeln!(
            output,
            "coverage-result: {}",
            if coverage_unavailable {
                "requested-precise-memory-unavailable"
            } else if memory_diagnostic {
                "memory-or-access-fault-reported"
            } else {
                "no-memory-fault-reported-not-a-safety-claim"
            }
        )
        .expect("write to String");
    }
    if let Some(error) = result.wait_error {
        writeln!(output, "supervisor-error: {error:?}").expect("write to String");
    }
    if let Some(error) = identity_errors.tool {
        writeln!(output, "tool-identity-error: {error:?}").expect("write to String");
    }
    if let Some(error) = identity_errors.target {
        writeln!(output, "target-identity-error: {error:?}").expect("write to String");
    }
    render_capture(&mut output, "stdout", &result.stdout, &stdout_digest);
    render_capture(&mut output, "stderr", &result.stderr, &stderr_digest);
    output.pop();
    CommandReport { output, succeeded }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TargetStatus {
    Exit(i32),
    Signal(String),
}

impl TargetStatus {
    fn render(&self) -> String {
        match self {
            Self::Exit(code) => format!("exit-{code}"),
            Self::Signal(signal) => format!("signal-{signal}"),
        }
    }
}

fn parse_target_status(output: &[u8]) -> Option<TargetStatus> {
    let text = String::from_utf8_lossy(output);
    if let Some(value) = marker_value(&text, "FE2O3_TARGET_EXIT_SIGNAL=")
        && !matches!(value, "void" | "0" | "")
    {
        return Some(TargetStatus::Signal(value.to_string()));
    }
    marker_value(&text, "FE2O3_TARGET_EXIT_CODE=")
        .and_then(|value| value.parse::<i32>().ok())
        .map(TargetStatus::Exit)
        .or_else(|| parse_debugger_signal(&text).map(TargetStatus::Signal))
        .or_else(|| parse_debugger_exit(&text).map(TargetStatus::Exit))
}

fn marker_value<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    text.lines()
        .rev()
        .find_map(|line| line.strip_prefix(marker).map(str::trim))
}

fn parse_debugger_signal(text: &str) -> Option<String> {
    [
        " received signal ",
        "Program terminated with signal ",
        "Program received signal ",
    ]
    .iter()
    .find_map(|marker| {
        let tail = text.rsplit_once(marker)?.1;
        let signal: String = tail
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric())
            .take(32)
            .collect();
        (!signal.is_empty()).then_some(signal)
    })
}

fn parse_debugger_exit(text: &str) -> Option<i32> {
    if text
        .lines()
        .rev()
        .any(|line| line.starts_with("[Inferior ") && line.ends_with(" exited normally]"))
    {
        return Some(0);
    }
    let tail = text.rsplit_once(" exited with code ")?.1;
    let digits: String = tail
        .chars()
        .take_while(char::is_ascii_digit)
        .take(12)
        .collect();
    if digits.is_empty() {
        return None;
    }
    let radix = if digits.len() > 1 && digits.starts_with('0') {
        8
    } else {
        10
    };
    i32::from_str_radix(&digits, radix).ok()
}

fn contains_memory_diagnostic(output: &[u8]) -> bool {
    let lower = String::from_utf8_lossy(output).to_ascii_lowercase();
    [
        "memory access fault",
        "gpu memory fault",
        "amdgpu wave stopped",
        "segmentation fault",
        "sigsegv",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn precise_memory_unavailable(output: &[u8]) -> bool {
    let lower = String::from_utf8_lossy(output).to_ascii_lowercase();
    [
        "precise memory access reporting could not be enabled",
        "undefined set amdgpu precise-memory",
        "no symbol \"amdgpu\" in current context",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn classify_outcome(
    plan: &InvocationPlan,
    result: &SupervisedResult,
    target_status: Option<&TargetStatus>,
    memory_diagnostic: bool,
    coverage_unavailable: bool,
    tool_changed: bool,
    target_changed: bool,
) -> (&'static str, bool) {
    if tool_changed {
        return ("tool-identity-changed", false);
    }
    if target_changed {
        return ("target-identity-changed", false);
    }
    match result.reason {
        StopReason::Timeout => return ("timeout", false),
        StopReason::OutputOverflow => return ("output-overflow", false),
        StopReason::WaitFailure => return ("supervisor-failure", false),
        StopReason::Exited => {}
    }
    let Some(status) = result.status.as_ref() else {
        return ("tool-status-unavailable", false);
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if status.signal().is_some() {
            return ("tool-signal", false);
        }
    }
    if !status.success() {
        return ("tool-exit-failure", false);
    }
    if plan.mode == Mode::Sanitize && coverage_unavailable {
        return ("required-coverage-unavailable", false);
    }
    if let Some(TargetStatus::Signal(_)) = target_status {
        return ("target-signal", false);
    }
    if let Some(TargetStatus::Exit(code)) = target_status
        && *code != 0
    {
        return ("target-exit-failure", false);
    }
    if plan.mode == Mode::Sanitize && memory_diagnostic {
        return ("sanitizer-diagnostic-reported", false);
    }
    if plan.mode == Mode::Debug && plan.session == DebugSession::Interactive {
        return ("interactive-session-ended", true);
    }
    if target_status.is_none() {
        return ("target-status-unavailable", false);
    }
    ("diagnostic-run-completed", true)
}

fn render_exit_status(output: &mut String, status: Option<&ExitStatus>) {
    match status {
        Some(status) => {
            if let Some(code) = status.code() {
                writeln!(output, "tool-status: exit-{code}").expect("write to String");
                return;
            }
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt as _;
                if let Some(signal) = status.signal() {
                    writeln!(output, "tool-status: signal-{signal}").expect("write to String");
                    return;
                }
            }
            writeln!(output, "tool-status: unknown").expect("write to String");
        }
        None => writeln!(output, "tool-status: unavailable").expect("write to String"),
    }
}

fn render_capture(output: &mut String, name: &str, capture: &Capture, digest: &[u8; 32]) {
    writeln!(output, "{name}-bytes: {}", capture.bytes.len()).expect("write to String");
    writeln!(output, "{name}-sha256: {}", hex(digest)).expect("write to String");
    writeln!(output, "{name}-overflow: {}", capture.overflow).expect("write to String");
    let text = String::from_utf8_lossy(&capture.bytes);
    writeln!(
        output,
        "{name}-text: {}",
        serde_json::to_string(text.as_ref()).expect("serialize diagnostic text")
    )
    .expect("write to String");
}

fn debug_session_name(session: DebugSession) -> &'static str {
    match session {
        DebugSession::Batch => "batch",
        DebugSession::Interactive => "interactive",
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("write to String");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(mode: Mode, values: &[&str]) -> Result<Options, String> {
        parse_options(
            mode,
            &values
                .iter()
                .map(|value| (*value).into())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn parses_bounded_execution_without_reinterpreting_target_arguments() {
        let parsed = parse(
            Mode::Debug,
            &[
                "--execute",
                "--batch",
                "--tool=/opt/rocm/bin/rocgdb-py_3.12",
                "--timeout-ms=17",
                "--stdout-limit=31",
                "--stderr-limit=47",
                "--cwd=/tmp",
                "--",
                "/program with space",
                ";touch",
                "--",
            ],
        )
        .unwrap();
        assert_eq!(parsed.action, Action::Execute);
        assert_eq!(parsed.debug_session, DebugSession::Batch);
        assert_eq!(parsed.program, "/program with space");
        assert_eq!(parsed.program_arguments, [";touch", "--"]);
        assert_eq!(parsed.timeout, Duration::from_millis(17));
    }

    #[test]
    fn rejects_malformed_and_unbounded_options() {
        for args in [
            vec![],
            vec!["program"],
            vec!["--"],
            vec!["--tool", "--", "program"],
            vec!["--bad", "--", "program"],
            vec!["--timeout-ms=0", "--", "program"],
            vec!["--stdout-limit=999999999", "--", "program"],
            vec!["--batch", "--interactive", "--", "program"],
            vec!["--execute", "--execute", "--batch", "--", "program"],
            vec!["--print-plan", "--execute", "--batch", "--", "program"],
        ] {
            assert!(parse(Mode::Debug, &args).is_err(), "accepted {args:?}");
        }
    }

    #[test]
    fn unavailable_coverage_fails_closed() {
        for (coverage, expected) in [("race", "race coverage"), ("api", "API coverage")] {
            let options =
                parse(Mode::Sanitize, &["--coverage", coverage, "--", "program"]).unwrap();
            let error = validate_requested_capability(Mode::Sanitize, &options).unwrap_err();
            assert!(error.contains(expected));
        }
        assert!(parse(Mode::Sanitize, &["--coverage=unknown", "--", "program"]).is_err());
    }

    #[test]
    fn preserves_a_program_separator_as_a_target_argument() {
        let parsed = parse(Mode::Debug, &["--", "program", "--", "value"]).unwrap();
        assert_eq!(parsed.program_arguments, ["--", "value"]);
    }

    #[test]
    fn discovery_order_prefers_native_reviewed_binaries() {
        let input = DiscoveryInput {
            rocm_roots: vec![PathBuf::from("/first"), PathBuf::from("/second")],
            path_directories: vec![PathBuf::from("/path")],
        };
        let expected = Path::new("/second/bin/rocgdb-py_3.12");
        assert_eq!(
            discover_with(&input, |candidate| candidate == expected),
            Some(expected.to_path_buf())
        );
    }

    #[test]
    fn sanitize_plan_states_exact_coverage_limitations() {
        let options = parse(Mode::Sanitize, &["--", "program", "argument"]).unwrap();
        let rendered = build_plan(
            Mode::Sanitize,
            DebugSession::Batch,
            Coverage::PreciseMemory,
            PathBuf::from("/opt/rocm/bin/rocgdb-py_3.12"),
            "program".into(),
            vec!["argument".into()],
        )
        .render(&options, Path::new("/tmp"));
        assert!(rendered.contains("backend: rocgdb-precise-memory"));
        assert!(rendered.contains("\"set amdgpu precise-memory on\""));
        assert!(rendered.contains("coverage-race: unsupported"));
        assert!(rendered.contains("not-a-safety-claim") || rendered.contains("not-proof"));
    }

    #[test]
    fn debug_batch_is_fixed_and_interactive_is_separate() {
        let batch = build_plan(
            Mode::Debug,
            DebugSession::Batch,
            Coverage::PreciseMemory,
            "/rocgdb".into(),
            "program".into(),
            vec![],
        );
        let interactive = build_plan(
            Mode::Debug,
            DebugSession::Interactive,
            Coverage::PreciseMemory,
            "/rocgdb".into(),
            "program".into(),
            vec![],
        );
        assert!(batch.arguments.iter().any(|argument| argument == "--batch"));
        assert!(batch.arguments.iter().any(|argument| argument == "run"));
        assert!(
            !interactive
                .arguments
                .iter()
                .any(|argument| argument == "--batch")
        );
        assert!(
            !interactive
                .arguments
                .iter()
                .any(|argument| argument == "run")
        );
        assert!(
            interactive
                .arguments
                .iter()
                .any(|argument| argument == "set auto-load off")
        );
    }

    #[test]
    fn target_status_parser_is_explicit() {
        assert_eq!(
            parse_target_status(b"x\nFE2O3_TARGET_EXIT_CODE=7\nFE2O3_TARGET_EXIT_SIGNAL=void\n"),
            Some(TargetStatus::Exit(7))
        );
        assert_eq!(
            parse_target_status(b"FE2O3_TARGET_EXIT_CODE=void\nFE2O3_TARGET_EXIT_SIGNAL=SIGSEGV\n"),
            Some(TargetStatus::Signal("SIGSEGV".into()))
        );
        assert_eq!(parse_target_status(b"plain successful run"), None);
        assert_eq!(
            parse_target_status(b"Thread 2 received signal SIGSEGV, Segmentation fault."),
            Some(TargetStatus::Signal("SIGSEGV".into()))
        );
        assert_eq!(
            parse_target_status(b"[Inferior 1 (process 12) exited with code 027]\n"),
            Some(TargetStatus::Exit(23))
        );
        assert_eq!(
            parse_target_status(b"[Inferior 1 (process 12) exited normally]\n"),
            Some(TargetStatus::Exit(0))
        );
    }

    #[test]
    fn unavailable_diagnostics_do_not_overstate_checker_coverage() {
        let diagnostic = unavailable(Mode::Sanitize);
        for gap in [
            "races",
            "uninitialized memory",
            "synchronization errors",
            "API misuse",
        ] {
            assert!(diagnostic.contains(gap));
        }
    }
}
