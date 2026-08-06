use std::env;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const MAX_DISCOVERY_DIRECTORIES: usize = 64;

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
            "usage: cargo fe2o3 {} [--tool /absolute/path/to/rocgdb] -- <program> [arguments...]",
            self.name()
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    tool_override: Option<PathBuf>,
    program: String,
    program_arguments: Vec<String>,
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
    executable: PathBuf,
    arguments: Vec<String>,
}

impl InvocationPlan {
    fn render(&self) -> String {
        let mut output = String::new();
        writeln!(output, "mode: {}", self.mode.name()).expect("write to String");
        writeln!(output, "authority: plan-only").expect("write to String");
        match self.mode {
            Mode::Sanitize => {
                writeln!(output, "backend: rocgdb-precise-memory").expect("write to String");
                writeln!(output, "coverage: gpu-memory-fault-location-only")
                    .expect("write to String");
                writeln!(
                    output,
                    "not-covered: data-races,uninitialized-memory,synchronization-errors"
                )
                .expect("write to String");
            }
            Mode::Debug => {
                writeln!(output, "backend: rocgdb-interactive").expect("write to String");
                writeln!(output, "coverage: debugger-launch-only").expect("write to String");
                writeln!(
                    output,
                    "not-covered: source-map-generation,local-layout-validation"
                )
                .expect("write to String");
            }
        }
        writeln!(output, "executable: {:?}", self.executable).expect("write to String");
        for (index, argument) in self.arguments.iter().enumerate() {
            writeln!(output, "arg[{index}]: {argument:?}").expect("write to String");
        }
        output.pop();
        output
    }
}

pub(crate) fn command(mode: Mode, args: &[String]) -> Result<String, String> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        return Ok(mode.usage());
    }
    let options = parse_options(mode, args)?;
    let executable = match options.tool_override {
        Some(path) => validate_override(mode, &path)?,
        None => discover_rocgdb(&DiscoveryInput::capture()).ok_or_else(|| unavailable(mode))?,
    };
    Ok(build_plan(mode, executable, options.program, options.program_arguments).render())
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
    let mut tool_override = None;
    let mut index = 0;
    while index < separator {
        let argument = &args[index];
        let value = if argument == "--tool" {
            index += 1;
            if index >= separator {
                return Err("--tool requires a path before `--`".to_string());
            }
            &args[index]
        } else if let Some(value) = argument.strip_prefix("--tool=") {
            value
        } else {
            return Err(format!(
                "unknown {} option `{argument}`\n{}",
                mode.name(),
                mode.usage()
            ));
        };
        if value.is_empty() {
            return Err("--tool path must not be empty".to_string());
        }
        if tool_override.replace(PathBuf::from(value)).is_some() {
            return Err("--tool was specified more than once".to_string());
        }
        index += 1;
    }

    let program = args
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
    let program_arguments = args[separator + 2..].to_vec();
    Ok(Options {
        tool_override,
        program,
        program_arguments,
    })
}

fn validate_override(mode: Mode, path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "{} --tool must be an absolute path to rocgdb",
            mode.name()
        ));
    }
    if path.file_name() != Some(OsStr::new("rocgdb")) {
        return Err(format!(
            "{} supports only a tool named `rocgdb`, got {}",
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
    std::fs::canonicalize(path).map_err(|error| {
        format!(
            "failed to resolve {} ROCgdb tool {}: {error}",
            mode.name(),
            path.display()
        )
    })
}

fn discover_rocgdb(input: &DiscoveryInput) -> Option<PathBuf> {
    discover_with(input, is_executable_file).and_then(|path| std::fs::canonicalize(path).ok())
}

fn discover_with(
    input: &DiscoveryInput,
    mut available: impl FnMut(&Path) -> bool,
) -> Option<PathBuf> {
    for root in &input.rocm_roots {
        for relative in ["bin/rocgdb", "lib/llvm/bin/rocgdb"] {
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
    executable: PathBuf,
    program: String,
    program_arguments: Vec<String>,
) -> InvocationPlan {
    let mut arguments = vec!["--quiet".to_string()];
    if mode == Mode::Sanitize {
        arguments.extend(
            [
                "--batch",
                "-ex",
                "set pagination off",
                "-ex",
                "set confirm off",
                "-ex",
                "set amdgpu precise-memory on",
                "-ex",
                "run",
                "-ex",
                "thread apply all backtrace",
            ]
            .map(str::to_string),
        );
    }
    arguments.push("--args".to_string());
    arguments.push(program);
    arguments.extend(program_arguments);
    InvocationPlan {
        mode,
        executable,
        arguments,
    }
}

fn unavailable(mode: Mode) -> String {
    match mode {
        Mode::Sanitize => "cargo fe2o3 sanitize unavailable: ROCgdb was not found in ROCM_PATH, HIP_PATH, supported /opt/rocm roots, or absolute PATH entries; the current checker foundation covers precise GPU memory fault reporting only, not races, uninitialized memory, or synchronization errors".to_string(),
        Mode::Debug => "cargo fe2o3 debug unavailable: ROCgdb was not found in ROCM_PATH, HIP_PATH, supported /opt/rocm roots, or absolute PATH entries".to_string(),
    }
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
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

#[cfg(test)]
mod tests {
    use super::{
        DiscoveryInput, Mode, Options, build_plan, discover_with, parse_options, unavailable,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn parses_program_and_explicit_tool_without_shell_interpretation() {
        assert_eq!(
            parse_options(
                Mode::Debug,
                &[
                    "--tool=/opt/rocm/bin/rocgdb".into(),
                    "--".into(),
                    "./program with space".into(),
                    "--target-option".into(),
                ],
            ),
            Ok(Options {
                tool_override: Some(PathBuf::from("/opt/rocm/bin/rocgdb")),
                program: "./program with space".into(),
                program_arguments: vec!["--target-option".into()],
            })
        );
    }

    #[test]
    fn rejects_malformed_command_lines() {
        for args in [
            vec![],
            vec!["program".into()],
            vec!["--".into()],
            vec!["--tool".into(), "--".into(), "program".into()],
            vec!["--bad".into(), "--".into(), "program".into()],
            vec![
                "--tool=/a/rocgdb".into(),
                "--tool=/b/rocgdb".into(),
                "--".into(),
                "program".into(),
            ],
        ] {
            assert!(
                parse_options(Mode::Sanitize, &args).is_err(),
                "accepted {args:?}"
            );
        }
    }

    #[test]
    fn preserves_a_program_separator_as_a_target_argument() {
        let parsed = parse_options(
            Mode::Debug,
            &["--".into(), "program".into(), "--".into(), "value".into()],
        )
        .unwrap();
        assert_eq!(parsed.program_arguments, ["--", "value"]);
    }

    #[test]
    fn discovery_order_is_rocm_root_then_llvm_then_path() {
        let input = DiscoveryInput {
            rocm_roots: vec![PathBuf::from("/first"), PathBuf::from("/second")],
            path_directories: vec![PathBuf::from("/path")],
        };
        let expected = Path::new("/second/lib/llvm/bin/rocgdb");
        assert_eq!(
            discover_with(&input, |candidate| candidate == expected),
            Some(expected.to_path_buf())
        );

        let path_tool = Path::new("/path/rocgdb");
        assert_eq!(
            discover_with(&input, |candidate| candidate == path_tool),
            Some(path_tool.to_path_buf())
        );
    }

    #[test]
    fn sanitize_plan_enables_precise_memory_and_states_coverage() {
        let rendered = build_plan(
            Mode::Sanitize,
            PathBuf::from("/opt/rocm/bin/rocgdb"),
            "program".into(),
            vec!["argument".into()],
        )
        .render();
        assert!(rendered.contains("backend: rocgdb-precise-memory"));
        assert!(rendered.contains("\"set amdgpu precise-memory on\""));
        assert!(rendered.contains("not-covered: data-races"));
        assert!(rendered.ends_with("arg[14]: \"argument\""));
    }

    #[test]
    fn debug_plan_is_interactive_and_does_not_add_checker_commands() {
        let rendered = build_plan(
            Mode::Debug,
            PathBuf::from("/opt/rocm/bin/rocgdb"),
            "program".into(),
            vec![],
        )
        .render();
        assert!(rendered.contains("backend: rocgdb-interactive"));
        assert!(rendered.contains("arg[2]: \"program\""));
        assert!(!rendered.contains("precise-memory on"));
        assert!(!rendered.contains("--batch"));
    }

    #[test]
    fn unavailable_diagnostics_do_not_overstate_checker_coverage() {
        let diagnostic = unavailable(Mode::Sanitize);
        for gap in ["races", "uninitialized memory", "synchronization errors"] {
            assert!(diagnostic.contains(gap));
        }
    }
}
