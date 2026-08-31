use std::path::Path;

use fe2o3_compiler_execution_deployment::{
    probe_compiler_execution_qualification_host_v1, run_compiler_execution_mount_qualification_v1,
};

const USAGE: &str = "usage: fe2o3-compiler-execution-qualification probe\n       fe2o3-compiler-execution-qualification run BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT INSTALL_PARENT BASE_IMAGE EXPECTED_BASE_IMAGE_SHA256 QUALIFICATION_PARENT";

fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
    let Some(command) = arguments.get(1).and_then(|argument| argument.to_str()) else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    match command {
        "probe" if arguments.len() == 2 => run_probe(),
        "run" if arguments.len() == 9 => run_qualification(&arguments),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
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

fn run_qualification(arguments: &[std::ffi::OsString]) {
    let Some(manifest_sha256) = arguments[3].to_str() else {
        eprintln!("expected manifest SHA-256 must be UTF-8");
        std::process::exit(2);
    };
    let Some(commit) = arguments[4].to_str() else {
        eprintln!("expected git commit must be UTF-8");
        std::process::exit(2);
    };
    let Some(base_sha256) = arguments[7].to_str() else {
        eprintln!("expected base-image SHA-256 must be UTF-8");
        std::process::exit(2);
    };
    match run_compiler_execution_mount_qualification_v1(
        Path::new(&arguments[2]),
        manifest_sha256,
        commit,
        Path::new(&arguments[5]),
        Path::new(&arguments[6]),
        base_sha256,
        Path::new(&arguments[8]),
    ) {
        Ok(report) => print!("{}", report.canonical_report()),
        Err(error) => {
            eprintln!("compiler-execution mount qualification failed: {error}");
            std::process::exit(1);
        }
    }
}
