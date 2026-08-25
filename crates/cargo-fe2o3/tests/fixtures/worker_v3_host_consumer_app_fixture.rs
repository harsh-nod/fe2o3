use std::error::Error;
use std::fs;
use std::path::PathBuf;

use fe2o3_host::{
    __hardware_test::application_handoff_observed_context_fixture_v1, KernelId,
    consume_inherited_worker_v3_application_handoff_v1,
};
use fe2o3_worker_v2_bundle::{
    WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1, WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1, WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("Worker V3 host consumer fixture: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if !(3..=4).contains(&arguments.len()) {
        return Err("usage: worker-v3-host-consumer KERNEL-ID TARGET REPORT [TEST-CONTROL]".into());
    }
    let substitute_commitment = match arguments.get(3) {
        None => false,
        Some(control) if control == "--fe2o3-test-substitute-commitment" => true,
        Some(control) => return Err(format!("unknown fixture control {control:?}").into()),
    };
    let handoff_names = [
        WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1,
        WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    ];
    for (name, _) in std::env::vars_os() {
        if !handoff_names
            .iter()
            .any(|allowed| name == std::ffi::OsStr::new(allowed))
        {
            return Err(format!("unexpected application environment survived: {name:?}").into());
        }
    }
    let report = PathBuf::from(&arguments[2]);
    fs::write(
        &report,
        br#"{"host_consumer":true,"loader_environment_clear":true,"admitted":false}"#,
    )?;
    if substitute_commitment {
        let mut commitment = std::env::var(WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1)?;
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
            std::env::set_var(WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, commitment);
        }
    }
    let kernel = KernelId::from_bytes(decode_hex_32(fs::read_to_string(&arguments[0])?.trim())?);
    let target = arguments[1].to_str().ok_or("target is not UTF-8")?;
    let observed = application_handoff_observed_context_fixture_v1(target);
    // SAFETY: the fixture has not created threads, signal handlers, descendants, or touched the
    // inherited handoff descriptors.
    let recovered =
        unsafe { consume_inherited_worker_v3_application_handoff_v1(kernel, &observed)? };
    recovered.revalidate_currentness()?;
    drop(recovered);
    fs::write(
        report,
        br#"{"host_consumer":true,"loader_environment_clear":true,"admitted":true,"current":true}"#,
    )?;
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
