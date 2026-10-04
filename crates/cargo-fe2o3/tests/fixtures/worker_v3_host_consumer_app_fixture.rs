use std::error::Error;
use std::fs;
use std::path::PathBuf;

use fe2o3_host::{
    __hardware_test::ApplicationHandoffVecAddRosterFixtureV1, KernelId,
    consume_inherited_worker_v3_application_handoff_v1,
    consume_inherited_worker_v3_application_roster_handoff_v1,
};
use fe2o3_runtime_protocol::{
    WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1, WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1, WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
    WORKER_V3_APPLICATION_PROOF_FD_ENV_V1,
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
    let (substitute_commitment, consume_roster) = match arguments.get(3) {
        None => (false, false),
        Some(control) if control == "--fe2o3-test-substitute-commitment" => (true, false),
        Some(control) if control == "--fe2o3-test-consume-roster" => (false, true),
        Some(control)
            if matches!(
                control.to_str(),
                Some(
                    "--fe2o3-test-missing-proof"
                        | "--fe2o3-test-aliased-proof"
                        | "--fe2o3-test-reserved-proof"
                        | "--fe2o3-test-blocking-proof"
                        | "--fe2o3-test-substituted-proof-slot"
                        | "--fe2o3-test-missing-proof-slot"
                )
            ) =>
        {
            (false, false)
        }
        Some(control) => return Err(format!("unknown fixture control {control:?}").into()),
    };
    let handoff_names = [
        WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1,
        WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
        WORKER_V3_APPLICATION_PROOF_FD_ENV_V1,
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
    alter_proof_input(arguments.get(3).and_then(|argument| argument.to_str()))?;
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
    if consume_roster {
        if kernel.as_bytes() != &[0xa1; 32] {
            return Err("roster fixture received a different kernel identity".into());
        }
        // SAFETY: the fixture has not created threads, signal handlers, descendants, or touched
        // the inherited handoff descriptors.
        let recovered = unsafe {
            consume_inherited_worker_v3_application_roster_handoff_v1::<
                ApplicationHandoffVecAddRosterFixtureV1,
            >()?
        };
        if recovered.target().to_string() != target {
            return Err("recovered roster target differs from the expected target".into());
        }
        if recovered.entrypoints().len() != 1
            || recovered.descriptor(0).map(|entry| entry.kernel_id()) != Some(kernel)
        {
            return Err("recovered roster differs from the exact fixture marker".into());
        }
        recovered.revalidate_currentness()?;
        drop(recovered);
        fs::write(
            report,
            br#"{"host_consumer":true,"loader_environment_clear":true,"admitted":true,"current":true,"roster":true}"#,
        )?;
        return Ok(());
    }
    // SAFETY: the fixture has not created threads, signal handlers, descendants, or touched the
    // inherited handoff descriptors.
    let recovered = unsafe { consume_inherited_worker_v3_application_handoff_v1(kernel)? };
    if recovered.target().to_string() != target {
        return Err("recovered artifact target differs from the expected target".into());
    }
    recovered.revalidate_currentness()?;
    drop(recovered);
    fs::write(
        report,
        br#"{"host_consumer":true,"loader_environment_clear":true,"admitted":true,"current":true}"#,
    )?;
    Ok(())
}

fn alter_proof_input(control: Option<&str>) -> Result<(), Box<dyn Error>> {
    use fe2o3_runtime_protocol::{
        WorkerV3ApplicationInputOccurrenceV1, WorkerV3ApplicationOccurrenceV1,
    };
    // SAFETY: this adversarial fixture changes its own startup environment before host claim,
    // without threads or signal handlers. No descriptor ownership is manufactured here.
    unsafe {
        match control {
            Some("--fe2o3-test-missing-proof") => {
                std::env::remove_var(WORKER_V3_APPLICATION_PROOF_FD_ENV_V1)
            }
            Some("--fe2o3-test-aliased-proof") => std::env::set_var(
                WORKER_V3_APPLICATION_PROOF_FD_ENV_V1,
                std::env::var(WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1)?,
            ),
            Some("--fe2o3-test-reserved-proof") => {
                std::env::set_var(WORKER_V3_APPLICATION_PROOF_FD_ENV_V1, "195")
            }
            Some("--fe2o3-test-blocking-proof") => {
                let fd = std::env::var(WORKER_V3_APPLICATION_PROOF_FD_ENV_V1)?.parse::<i32>()?;
                let flags = libc::fcntl(fd, libc::F_GETFL);
                if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) < 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
            }
            Some("--fe2o3-test-substituted-proof-slot" | "--fe2o3-test-missing-proof-slot") => {
                let encoded = std::env::var(WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1)?;
                let bytes = (0..encoded.len())
                    .step_by(2)
                    .map(|index| u8::from_str_radix(&encoded[index..index + 2], 16))
                    .collect::<Result<Vec<_>, _>>()?;
                let occurrence = WorkerV3ApplicationOccurrenceV1::decode_canonical(&bytes)?;
                let mut inputs = occurrence.inputs()[..3].to_vec();
                if control == Some("--fe2o3-test-substituted-proof-slot") {
                    inputs.push(WorkerV3ApplicationInputOccurrenceV1::new(4, [0x73; 32])?);
                }
                let changed = WorkerV3ApplicationOccurrenceV1::new(
                    occurrence.application(),
                    occurrence.spawn_identity(),
                    &inputs,
                )?;
                let encoded = changed
                    .encode_canonical()?
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                std::env::set_var(WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1, encoded);
            }
            _ => {}
        }
    }
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
