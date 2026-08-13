use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=RUSTC");

    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let output = Command::new(rustc)
        .args(["--print", "sysroot"])
        .output()
        .expect("failed to query rustc sysroot");

    if !output.status.success() {
        panic!("rustc --print sysroot failed");
    }

    let sysroot = String::from_utf8(output.stdout).expect("rustc sysroot was not UTF-8");
    let sysroot = sysroot.trim();
    println!("cargo:rustc-link-search=native={sysroot}/lib");

    let verbose = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string()))
        .arg("-vV")
        .output()
        .expect("failed to query rustc version identity");
    if !verbose.status.success() {
        panic!("rustc -vV failed");
    }
    let verbose = String::from_utf8(verbose.stdout).expect("rustc -vV output was not UTF-8");
    for (key, prefix) in [
        ("FE2O3_BUILD_RUSTC_RELEASE", "release: "),
        ("FE2O3_BUILD_RUSTC_COMMIT", "commit-hash: "),
        ("FE2O3_BUILD_RUSTC_LLVM", "LLVM version: "),
    ] {
        let value = verbose
            .lines()
            .find_map(|line| line.strip_prefix(prefix))
            .unwrap_or_else(|| panic!("rustc -vV omitted {prefix:?}"));
        println!("cargo:rustc-env={key}={value}");
    }
}
