use std::path::Path;

use fe2o3_compiler_execution_deployment::{
    CompilerExecutionInstalledRootPublicationV1, encode_sha256_lower_hex_v1,
    install_compiler_execution_deployment_v1, install_compiler_execution_deployment_v3,
    recover_compiler_execution_install_parent_v3, verify_compiler_execution_deployment_v1,
    verify_compiler_execution_deployment_v3,
};

fn main() {
    let mut arguments: Vec<_> = std::env::args_os().collect();
    let native = arguments.get(1).is_some_and(|arg| arg == "--v3");
    if native {
        arguments.remove(1);
    }
    if native && arguments.get(1).is_some_and(|arg| arg == "recover") {
        if arguments.len() != 4 {
            eprintln!(
                "usage: fe2o3-compiler-execution-deployment-install --v3 recover EXPECTED_MANIFEST_SHA256 INSTALL_PARENT"
            );
            std::process::exit(2);
        }
        let Some(digest) = arguments[2].to_str() else {
            eprintln!("expected manifest SHA-256 must be UTF-8");
            std::process::exit(2);
        };
        match recover_compiler_execution_install_parent_v3(Path::new(&arguments[3]), digest) {
            Ok(outcome) => println!("install_recovery={outcome:?}"),
            Err(error) => {
                eprintln!("compiler-execution V3 recovery failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if arguments.len() != 5 {
        eprintln!(
            "usage: fe2o3-compiler-execution-deployment-install BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT INSTALL_PARENT"
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
    macro_rules! install {
        ($verify:path, $install:path) => {{
            let verified = match $verify(Path::new(&arguments[1]), manifest_sha256, commit) {
                Ok(verified) => verified,
                Err(error) => {
                    eprintln!("compiler-execution deployment verification failed: {error}");
                    std::process::exit(1);
                }
            };
            let installed = match $install(verified, Path::new(&arguments[4])) {
                Ok(installed) => installed,
                Err(error) => {
                    eprintln!("compiler-execution deployment installation failed: {error}");
                    std::process::exit(1);
                }
            };
            println!("installed_git_commit={}", installed.git_commit());
            println!("installed_target={}", installed.target());
            println!(
                "installed_manifest_sha256={}",
                encode_sha256_lower_hex_v1(installed.manifest_sha256())
            );
            println!("installed_root_name={}", installed.root_name());
            println!("installed_file_count={}", installed.file_count());
            println!(
                "installed_publication={}",
                match installed.publication() {
                    CompilerExecutionInstalledRootPublicationV1::Created => "created",
                    CompilerExecutionInstalledRootPublicationV1::Reacquired => "reacquired",
                }
            );
        }};
    }
    if native {
        install!(
            verify_compiler_execution_deployment_v3,
            install_compiler_execution_deployment_v3
        );
    } else {
        install!(
            verify_compiler_execution_deployment_v1,
            install_compiler_execution_deployment_v1
        );
    }
}
