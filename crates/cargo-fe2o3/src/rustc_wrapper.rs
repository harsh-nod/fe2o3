use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::process::{Command, ExitStatus, Stdio};

use fe2o3_rustc_invocation::{
    RustcArgsErrorV2, RustcCompileInvocationV2, RustcInvocationV2, RustcPassthroughInvocationV2,
    classify_rustc_invocation_v2,
};

#[cfg_attr(not(test), allow(dead_code))]
#[path = "rustc_wrapper/pinned_codegen_backend.rs"]
mod pinned_codegen_backend;

#[cfg_attr(not(test), allow(dead_code))]
#[path = "rustc_wrapper/pinned_executable.rs"]
mod pinned_executable;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WrapperPlan<'a> {
    Passthrough(RustcPassthroughInvocationV2<'a>),
    Compile(RustcCompileInvocationV2<'a>),
}

#[derive(Debug)]
pub(crate) enum WrapperError {
    Arguments(RustcArgsErrorV2),
    ExecutionNotPrepared,
    Spawn(std::io::Error),
}

impl fmt::Display for WrapperError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arguments(error) => {
                write!(formatter, "invalid rustc wrapper invocation: {error}")
            }
            Self::ExecutionNotPrepared => formatter.write_str(
                "rustc execution is not activated for this invocation until pinned executable and codegen-backend inheritance are prepared",
            ),
            Self::Spawn(error) => write!(formatter, "failed to execute rustc passthrough: {error}"),
        }
    }
}

impl Error for WrapperError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Arguments(error) => Some(error),
            Self::Spawn(error) => Some(error),
            Self::ExecutionNotPrepared => None,
        }
    }
}

impl From<RustcArgsErrorV2> for WrapperError {
    fn from(value: RustcArgsErrorV2) -> Self {
        Self::Arguments(value)
    }
}

pub(crate) fn run(argv: Vec<OsString>) -> Result<ExitStatus, WrapperError> {
    match plan(&argv)? {
        WrapperPlan::Passthrough(invocation) => run_passthrough(invocation),
        WrapperPlan::Compile(_) => Err(WrapperError::ExecutionNotPrepared),
    }
}

fn plan(argv: &[OsString]) -> Result<WrapperPlan<'_>, WrapperError> {
    let classified = classify_rustc_invocation_v2(argv)?;
    match classified {
        RustcInvocationV2::Terminal(invocation) | RustcInvocationV2::Query(invocation)
            if classified.is_bootstrap_passthrough_approved() =>
        {
            Ok(WrapperPlan::Passthrough(invocation))
        }
        RustcInvocationV2::Compile(invocation) => Ok(WrapperPlan::Compile(invocation)),
        _ => Err(WrapperError::ExecutionNotPrepared),
    }
}

fn run_passthrough(
    invocation: RustcPassthroughInvocationV2<'_>,
) -> Result<ExitStatus, WrapperError> {
    Command::new(invocation.executable())
        .args(invocation.forwarded_args())
        .stdin(Stdio::null())
        .status()
        .map_err(WrapperError::Spawn)
}

pub(crate) fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn cargo_probe_and_terminal_modes_plan_lossless_passthrough() {
        for argv in [
            args(&[
                "/toolchain/bin/rustc",
                "-",
                "--crate-name",
                "___",
                "--print=file-names",
                "--crate-type=bin",
                "--crate-type=rlib",
            ]),
            args(&["/toolchain/bin/rustc", "-vV"]),
            args(&["/toolchain/bin/rustc", "-Zhelp"]),
        ] {
            let WrapperPlan::Passthrough(invocation) = plan(&argv).unwrap() else {
                panic!("expected passthrough plan");
            };
            assert_eq!(invocation.argv(), argv);
        }
    }

    #[test]
    fn compile_plan_preserves_original_arguments() {
        let argv = args(&[
            "/toolchain/bin/rustc",
            "--crate-name",
            "kernel",
            "src/lib.rs",
            "--edition=2024",
        ]);
        let WrapperPlan::Compile(invocation) = plan(&argv).unwrap() else {
            panic!("expected compile plan");
        };
        assert_eq!(invocation.argv(), argv);
        assert_eq!(invocation.crate_name(), "kernel");
        assert_eq!(invocation.source_path(), std::path::Path::new("src/lib.rs"));
    }

    #[test]
    fn malformed_compile_and_response_files_fail_closed() {
        assert!(plan(&args(&["rustc", "--crate-name", "partial"])).is_err());
        assert!(plan(&args(&["rustc", "@args.rsp"])).is_err());
        assert!(plan(&args(&["rustc", "--crate-name", "a", "a.rs", "b.rs"])).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn query_plan_rejects_options_outside_the_bootstrap_grammar() {
        use std::os::unix::ffi::OsStringExt;

        let argv = vec![
            OsString::from("rustc"),
            OsString::from("--print=sysroot"),
            OsString::from("--cfg"),
            OsString::from_vec(vec![0xff]),
        ];
        assert!(matches!(
            plan(&argv),
            Err(WrapperError::ExecutionNotPrepared)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn exit_code_preserves_full_windows_status() {
        use std::os::windows::process::ExitStatusExt;

        let raw = 0xc000_0005;
        assert_eq!(exit_code(ExitStatus::from_raw(raw)), raw as i32);
    }
}
