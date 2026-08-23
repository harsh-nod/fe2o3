use std::error::Error;
use std::fs;
use std::path::PathBuf;

use fe2o3_host::{
    __hardware_test::application_handoff_observed_context_fixture_v1, KernelId,
    consume_inherited_worker_v2_application_handoff_v1,
    consume_inherited_worker_v3_application_handoff_v1,
};
use fe2o3_worker_v2_bundle::{
    CompilerTransactionEvidenceCapsuleV2, WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
    WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1, WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
    WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
    WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1, WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("host consumer fixture: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if !(4..=5).contains(&arguments.len()) {
        return Err(
            "usage: host-consumer-app CAPSULE|--worker-v3 KERNEL-ID TARGET REPORT [TEST-CONTROL]"
                .into(),
        );
    }
    let worker_v3 = arguments[0] == "--worker-v3";
    let substitute_commitment = match arguments.get(4) {
        None => false,
        Some(control) if control == "--fe2o3-test-substitute-commitment" => true,
        Some(control) => return Err(format!("unknown fixture control {control:?}").into()),
    };
    let worker_v2_handoff_names = [
        WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
        WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    ];
    let worker_v3_handoff_names = [
        WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1,
        WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    ];
    for (name, _) in std::env::vars_os() {
        if !worker_v2_handoff_names
            .iter()
            .chain(&worker_v3_handoff_names)
            .any(|allowed| name == std::ffi::OsStr::new(allowed))
        {
            return Err(format!("unexpected application environment survived: {name:?}").into());
        }
    }
    let report = PathBuf::from(&arguments[3]);
    fs::write(
        &report,
        br#"{"host_consumer":true,"loader_environment_clear":true,"admitted":false}"#,
    )?;
    if substitute_commitment {
        let name = if worker_v3 {
            WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1
        } else {
            WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1
        };
        let mut commitment = std::env::var(name)?;
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
            std::env::set_var(name, commitment);
        }
    }
    let kernel = KernelId::from_bytes(decode_hex_32(fs::read_to_string(&arguments[1])?.trim())?);
    let target = arguments[2].to_str().ok_or("target is not UTF-8")?;
    let observed = application_handoff_observed_context_fixture_v1(target);
    // SAFETY: the fixture has not created threads, signal handlers, descendants, or touched the
    // inherited handoff descriptors.
    if worker_v3 {
        let recovered =
            unsafe { consume_inherited_worker_v3_application_handoff_v1(kernel, &observed)? };
        recovered.revalidate_currentness()?;
        drop(recovered);
    } else {
        let capsule = CompilerTransactionEvidenceCapsuleV2::from_bytes(&fs::read(&arguments[0])?)?;
        let recovered = unsafe {
            consume_inherited_worker_v2_application_handoff_v1(capsule, kernel, &observed)?
        };
        recovered.revalidate_currentness()?;
        drop(recovered);
    }
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
