use std::io::{self, Read, Write};

use sha2::{Digest, Sha256};

const WORKER_ID: &str = "fixture-worker-v3";
const OUTPUT: &[u8] = b"fixture-output";
const MISMATCH_OUTPUT: &[u8] = b"changed-output";

fn main() {
    let mut request = Vec::new();
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
    io::stdout()
        .write_all(&response_with_diagnostics(
            &request,
            WORKER_ID,
            with_output,
            wrong_request,
            output,
            diagnostics,
        ))
        .unwrap();
}

fn response_with_diagnostics(
    request: &[u8],
    worker: &str,
    with_output: bool,
    wrong_request: bool,
    output_bytes: &[u8],
    diagnostics: &[&str],
) -> Vec<u8> {
    let request_id: [u8; 32] = request[14..46].try_into().unwrap();
    let mut request_identity: [u8; 32] = field(request, 15).try_into().unwrap();
    if wrong_request {
        request_identity[0] ^= 1;
    }
    let mut bytes = b"F3LRSP02".to_vec();
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
