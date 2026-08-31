use std::path::Path;

use fe2o3_compiler_execution_deployment::{
    CompilerExecutionMountQualificationRequestV1, CompilerExecutionQualificationRecoveryV1,
    QualificationMountFaultPointV1, probe_compiler_execution_qualification_host_v1,
    recover_compiler_execution_qualification_parent_v1, run_compiler_execution_mount_campaign_v1,
    run_compiler_execution_mount_fault_v1, run_compiler_execution_mount_qualification_request_v1,
};

const USAGE: &str = "usage: fe2o3-compiler-execution-qualification probe\n       fe2o3-compiler-execution-qualification fault-points\n       fe2o3-compiler-execution-qualification recover QUALIFICATION_PARENT\n       fe2o3-compiler-execution-qualification run BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT INSTALL_PARENT BASE_IMAGE EXPECTED_BASE_IMAGE_SHA256 QUALIFICATION_PARENT\n       fe2o3-compiler-execution-qualification fault POINT BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT INSTALL_PARENT BASE_IMAGE EXPECTED_BASE_IMAGE_SHA256 QUALIFICATION_PARENT\n       fe2o3-compiler-execution-qualification campaign BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT EMPTY_INSTALL_PARENT BASE_IMAGE EXPECTED_BASE_IMAGE_SHA256 QUALIFICATION_PARENT";

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
        "run" if arguments.len() == 9 => run_qualification(&arguments),
        "fault" if arguments.len() == 10 => run_fault(&arguments),
        "campaign" if arguments.len() == 9 => run_campaign(&arguments),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

fn print_fault_points() {
    for point in QualificationMountFaultPointV1::all() {
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
    match run_compiler_execution_mount_qualification_request_v1(request) {
        Ok(report) => print!("{}", report.canonical_report()),
        Err(error) => {
            eprintln!("compiler-execution mount qualification failed: {error}");
            std::process::exit(1);
        }
    }
}

fn run_fault(arguments: &[std::ffi::OsString]) {
    let Some(point_name) = arguments[2].to_str() else {
        eprintln!("fault point must be UTF-8");
        std::process::exit(2);
    };
    let Some(point) = QualificationMountFaultPointV1::from_canonical_name(point_name) else {
        eprintln!("fault point is not one canonical V1 point");
        std::process::exit(2);
    };
    let request = parse_request(arguments, 3);
    match run_compiler_execution_mount_fault_v1(point, request) {
        Ok(report) => print!("{}", report.canonical_report()),
        Err(error) => {
            eprintln!("compiler-execution mount fault qualification failed: {error}");
            std::process::exit(1);
        }
    }
}

fn run_campaign(arguments: &[std::ffi::OsString]) {
    let request = parse_request(arguments, 2);
    match run_compiler_execution_mount_campaign_v1(request) {
        Ok(report) => print!("{}", report.canonical_report()),
        Err(error) => {
            eprintln!("compiler-execution mount campaign failed: {error}");
            std::process::exit(1);
        }
    }
}

fn parse_request(
    arguments: &[std::ffi::OsString],
    start: usize,
) -> CompilerExecutionMountQualificationRequestV1<'_> {
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
    CompilerExecutionMountQualificationRequestV1::new(
        Path::new(&arguments[start]),
        manifest_sha256,
        commit,
        Path::new(&arguments[start + 3]),
        Path::new(&arguments[start + 4]),
        base_sha256,
        Path::new(&arguments[start + 6]),
    )
}
