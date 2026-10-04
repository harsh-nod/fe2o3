use std::path::Path;

use fe2o3_compiler_execution_deployment::{
    encode_sha256_lower_hex_v1, generate_compiler_execution_install_manifest_v1,
    generate_compiler_execution_install_manifest_v3,
};

fn main() {
    let mut arguments: Vec<_> = std::env::args_os().collect();
    let native = arguments.get(1).is_some_and(|arg| arg == "--v3");
    if native {
        arguments.remove(1);
    }
    if arguments.len() != 4 {
        eprintln!("usage: fe2o3-compiler-execution-manifest BUNDLE_ROOT GIT_COMMIT TARGET");
        std::process::exit(2);
    }
    let Some(commit) = arguments[2].to_str() else {
        eprintln!("git commit must be UTF-8");
        std::process::exit(2);
    };
    let Some(target) = arguments[3].to_str() else {
        eprintln!("target must be UTF-8");
        std::process::exit(2);
    };
    let generate = if native {
        generate_compiler_execution_install_manifest_v3
    } else {
        generate_compiler_execution_install_manifest_v1
    };
    match generate(Path::new(&arguments[1]), commit, target) {
        Ok(report) => {
            println!(
                "manifest_sha256={}",
                encode_sha256_lower_hex_v1(report.sha256())
            );
            println!("manifest_byte_len={}", report.byte_len());
        }
        Err(error) => {
            eprintln!("cannot generate compiler-execution install manifest: {error}");
            std::process::exit(1);
        }
    }
}
