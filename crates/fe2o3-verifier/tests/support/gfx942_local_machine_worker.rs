#[cfg(target_os = "linux")]
mod common {
    include!(
        "../../../fe2o3-kernel-analysis/tests/fixtures/machine_effect_worker_fixture_common.rs"
    );
}

#[cfg(target_os = "linux")]
fn main() {
    common::run(0xa1, 0xb2);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("the authenticated machine-analysis test worker requires Linux");
    std::process::exit(77);
}
