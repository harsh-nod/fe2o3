use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
};

use fe2o3_hsaco_finalize::InertDecodedWorkerExchangeV2;
use sha2::{Digest, Sha256};

mod worker_derivation_fixture_support;
use worker_derivation_fixture_support::{
    append_derivation_response_fields, append_derivation_response_fields_with_salt,
};

const WORKER_ID: &str = "fixture-worker-v3";
const OUTPUT: &[u8] = b"fixture-output";
const MISMATCH_OUTPUT: &[u8] = b"changed-output";
const APPENDED_HSACO_MAGIC: &[u8; 16] = b"F3NATIVEHSACO01\0";
const MAX_APPENDED_HSACO: u64 = 64 * 1024;
const MAX_NATIVE_REQUEST: u64 = 16 * 1024 * 1024;

fn main() {
    let appended = appended_hsaco().expect("bounded test Worker trailer");
    let mut request = Vec::new();
    if let Some(output) = appended {
        io::stdin()
            .take(MAX_NATIVE_REQUEST + 1)
            .read_to_end(&mut request)
            .unwrap();
        assert!(request.len() as u64 <= MAX_NATIVE_REQUEST);
        // A failure envelope lets the public strict exchange decoder validate
        // the request without a second request parser or signed-module edits.
        let refusal = response_with_diagnostics(&request, WORKER_ID, false, false, &[], &[], &[]);
        let decoded = InertDecodedWorkerExchangeV2::decode(&request, &refusal).unwrap();
        assert!(decoded.request().external_providers().is_empty());
        assert!(output.len() as u64 <= decoded.request().output_constraints().max_bytes());
        io::stdout()
            .write_all(&response_with_diagnostics(
                &request,
                WORKER_ID,
                true,
                false,
                &output,
                &[],
                &[],
            ))
            .unwrap();
        return;
    }
    io::stdin().read_to_end(&mut request).unwrap();
    if !request.starts_with(b"F3LREQ02") || !contains(&request, b"workflow_kernel") {
        std::process::exit(64);
    }

    let exact_replay = output_bound(&request) == OUTPUT.len() as u64;
    let with_output = if exact_replay {
        !contains(&request, b"workflow_v2_failure")
    } else {
        !contains(&request, b"workflow_candidate_failure")
    };
    let wrong_request = (!exact_replay && contains(&request, b"workflow_candidate_bad_response"))
        || (exact_replay && contains(&request, b"workflow_v2_bad_response"));
    let output = if exact_replay && contains(&request, b"workflow_mismatch") {
        MISMATCH_OUTPUT
    } else {
        OUTPUT
    };
    let diagnostics = if contains(&request, b"workflow_phase_trace") {
        if exact_replay {
            &["fixture.phase=v2-exact-replay"][..]
        } else {
            &["fixture.phase=v2-bootstrap"][..]
        }
    } else {
        &[]
    };
    let stage_salt = if exact_replay && contains(&request, b"worker-v3-5b") {
        b"foreign-replay-stage".as_slice()
    } else {
        &[]
    };
    io::stdout()
        .write_all(&response_with_diagnostics(
            &request,
            WORKER_ID,
            with_output,
            wrong_request,
            output,
            diagnostics,
            stage_salt,
        ))
        .unwrap();
}

// The private executable copy, payload, length and footer are measured together
// by PinnedWorkerV1. There is no mutable sidecar or external-provider loophole.
fn appended_hsaco() -> io::Result<Option<Vec<u8>>> {
    // The executor runs a sealed memfd, whose readlink target is not a pathname.
    #[cfg(target_os = "linux")]
    let mut file = File::open("/proc/self/exe")?;
    #[cfg(not(target_os = "linux"))]
    let mut file = File::open(std::env::current_exe()?)?;
    let length = file.metadata()?.len();
    let Some(footer_start) = length.checked_sub(24) else {
        return Ok(None);
    };
    file.seek(SeekFrom::Start(footer_start))?;
    let mut footer = [0; 24];
    file.read_exact(&mut footer)?;
    if &footer[8..] != APPENDED_HSACO_MAGIC {
        return Ok(None);
    }
    let payload_length = u64::from_le_bytes(footer[..8].try_into().unwrap());
    if !(1..=MAX_APPENDED_HSACO).contains(&payload_length) || payload_length > footer_start {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "test HSACO trailer length",
        ));
    }
    file.seek(SeekFrom::Start(footer_start - payload_length))?;
    let mut payload = vec![0; payload_length as usize];
    file.read_exact(&mut payload)?;
    Ok(Some(payload))
}

fn response_with_diagnostics(
    request: &[u8],
    worker: &str,
    with_output: bool,
    wrong_request: bool,
    output_bytes: &[u8],
    diagnostics: &[&str],
    stage_salt: &[u8],
) -> Vec<u8> {
    let request_id: [u8; 32] = request[14..46].try_into().unwrap();
    let mut request_identity: [u8; 32] = field(request, 15).try_into().unwrap();
    if wrong_request {
        request_identity[0] ^= 1;
    }
    let mut bytes = if with_output {
        b"F3LRSP04".to_vec()
    } else {
        b"F3LRSP02".to_vec()
    };
    push_field(&mut bytes, 1, &request_id);
    push_field(&mut bytes, 2, &request_identity);
    push_field(&mut bytes, 3, field(request, 8));
    push_field(&mut bytes, 4, worker.as_bytes());
    push_field(&mut bytes, 5, &[if with_output { 9 } else { 6 }]);
    let mut diagnostic_bytes = Vec::new();
    diagnostic_bytes.extend_from_slice(&(diagnostics.len() as u32).to_le_bytes());
    for diagnostic in diagnostics {
        diagnostic_bytes.extend_from_slice(&(diagnostic.len() as u32).to_le_bytes());
        diagnostic_bytes.extend_from_slice(diagnostic.as_bytes());
    }
    push_field(&mut bytes, 6, &diagnostic_bytes);
    if with_output {
        let output_identity: [u8; 32] = Sha256::digest(output_bytes).into();
        let mut output = vec![1];
        output.extend_from_slice(&output_identity);
        output.extend_from_slice(&(output_bytes.len() as u64).to_le_bytes());
        output.extend_from_slice(output_bytes);
        push_field(&mut bytes, 7, &output);
        if stage_salt.is_empty() {
            append_derivation_response_fields(&mut bytes, request, output_bytes);
        } else {
            append_derivation_response_fields_with_salt(
                &mut bytes,
                request,
                output_bytes,
                stage_salt,
            );
        }
    } else {
        push_field(&mut bytes, 7, &[0]);
    }
    bytes
}

fn output_bound(request: &[u8]) -> u64 {
    u64::from_le_bytes(field(request, 14).try_into().unwrap())
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

fn field(bytes: &[u8], wanted: u16) -> &[u8] {
    let mut offset = 8;
    while offset < bytes.len() {
        let tag = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
        let len = u32::from_le_bytes(bytes[offset + 2..offset + 6].try_into().unwrap()) as usize;
        offset += 6;
        if tag == wanted {
            return &bytes[offset..offset + len];
        }
        offset += len;
    }
    panic!("missing field {wanted}")
}

fn push_field(bytes: &mut Vec<u8>, tag: u16, value: &[u8]) {
    bytes.extend_from_slice(&tag.to_le_bytes());
    bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
    bytes.extend_from_slice(value);
}
