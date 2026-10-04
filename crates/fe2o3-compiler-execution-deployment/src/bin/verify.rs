use std::path::Path;

use fe2o3_compiler_execution_deployment::{
    encode_sha256_lower_hex_v1, verify_compiler_execution_deployment_v1,
    verify_compiler_execution_deployment_v3,
};

fn main() {
    let mut arguments: Vec<_> = std::env::args_os().collect();
    let native = arguments.get(1).is_some_and(|arg| arg == "--v3");
    if native {
        arguments.remove(1);
    }
    if arguments.len() != 4 {
        eprintln!(
            "usage: fe2o3-compiler-execution-deployment-verify BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT"
        );
        std::process::exit(2);
    }
    let Some(manifest_sha256) = arguments[2].to_str() else {
        eprintln!("expected manifest SHA-256 must be UTF-8");
        std::process::exit(2);
    };
    let Some(commit) = arguments[3].to_str() else {
        eprintln!("expected git commit must be UTF-8");
        std::process::exit(2);
    };
    macro_rules! verify {
        ($verify:path) => {
            match $verify(Path::new(&arguments[1]), manifest_sha256, commit) {
                Ok(verified) => {
                    println!("verified_git_commit={}", verified.git_commit());
                    println!("verified_target={}", verified.target());
                    println!(
                        "verified_manifest_sha256={}",
                        encode_sha256_lower_hex_v1(verified.manifest_sha256())
                    );
                    println!("verified_file_count={}", verified.file_count());
                }
                Err(error) => {
                    eprintln!("compiler-execution deployment verification failed: {error}");
                    std::process::exit(1);
                }
            }
        };
    }
    if native {
        verify!(verify_compiler_execution_deployment_v3);
    } else {
        verify!(verify_compiler_execution_deployment_v1);
    }
}
