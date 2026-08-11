use std::error::Error;
use std::fs;
use std::path::PathBuf;

use fe2o3_host::{
    __hardware_test::application_handoff_observed_context_fixture_v1, KernelId,
    consume_inherited_worker_v2_application_handoff_v1,
};
use fe2o3_worker_v2_bundle::{
    CompilerTransactionEvidenceCapsuleV2, WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("host consumer fixture: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 4 {
        return Err("usage: host-consumer-app CAPSULE KERNEL-ID TARGET REPORT".into());
    }
    for name in [
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "LD_AUDIT",
        "GLIBC_TUNABLES",
    ] {
        if std::env::var_os(name).is_some() {
            return Err(format!("loader-sensitive environment survived: {name}").into());
        }
    }
    let report = PathBuf::from(&arguments[3]);
    fs::write(
        &report,
        br#"{"host_consumer":true,"loader_environment_clear":true,"admitted":false}"#,
    )?;
    if std::env::var_os("RUNNER_HOST_FIXTURE_SUBSTITUTE_COMMITMENT").is_some() {
        let mut commitment = std::env::var(WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1)?;
        commitment.replace_range(
            ..1,
            if commitment.starts_with('0') {
                "1"
            } else {
                "0"
            },
        );
        // SAFETY: Cargo starts this fixture as a single-threaded cooperative handoff consumer.
        unsafe {
            std::env::set_var(WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, commitment);
        }
    }
    let capsule = CompilerTransactionEvidenceCapsuleV2::from_bytes(&fs::read(&arguments[0])?)?;
    let kernel = KernelId::from_bytes(decode_hex_32(fs::read_to_string(&arguments[1])?.trim())?);
    let target = arguments[2].to_str().ok_or("target is not UTF-8")?;
    let observed = application_handoff_observed_context_fixture_v1(target);
    // SAFETY: this is the first operation after argument/capsule decoding; the fixture has not
    // created threads, signal handlers, descendants, or touched inherited descriptors.
    let recovered =
        unsafe { consume_inherited_worker_v2_application_handoff_v1(capsule, kernel, &observed)? };
    recovered.revalidate_currentness()?;
    fs::write(
        report,
        br#"{"host_consumer":true,"loader_environment_clear":true,"admitted":true,"current":true}"#,
    )?;
    drop(recovered);
    Ok(())
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], Box<dyn Error>> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("kernel ID is not 32-byte hex".into());
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)?;
    }
    Ok(output)
}
