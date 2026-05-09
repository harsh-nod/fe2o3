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
}
