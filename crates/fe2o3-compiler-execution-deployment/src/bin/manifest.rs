use std::path::Path;

use fe2o3_compiler_execution_deployment::{
    encode_sha256_lower_hex_v1, generate_compiler_execution_install_manifest_v1,
};

fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
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
    match generate_compiler_execution_install_manifest_v1(Path::new(&arguments[1]), commit, target)
    {
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
