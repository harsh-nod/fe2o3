use std::{
    env,
    io::{self, Read, Write},
    process::{Command, exit},
    thread,
    time::Duration,
};

use sha2::{Digest, Sha256};

const WORKER_ID: &str = "fixture-worker-v1";
const OUTPUT: &[u8] = b"fixture-output";
const MISMATCH_OUTPUT: &[u8] = b"changed-output";

#[allow(clippy::zombie_processes)] // Mode 8 verifies that the external supervisor owns reaping.
fn main() {
    if env::args().nth(1).as_deref() == Some("--descendant") {
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }

    let mut prefix = [0_u8; 46];
    io::stdin().read_exact(&mut prefix).unwrap();
    let is_v2 = &prefix[..8] == b"F3LREQ02";
    let legacy_mode = prefix[14];
    let is_legacy_control = prefix[14..46].iter().all(|byte| *byte == legacy_mode);
    if !is_v2 && is_legacy_control && legacy_mode == 2 {
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
    if !is_v2 && is_legacy_control && legacy_mode == 9 {
        exit(0);
    }
    let mut request = prefix.to_vec();
    io::stdin().read_to_end(&mut request).unwrap();
    let is_workflow = contains(&request, b"workflow_kernel");
    let mode = if is_v2 || is_workflow { 1 } else { legacy_mode };

    if is_workflow {
        let exact_replay = is_v2 && output_bound(&request) == OUTPUT.len() as u64;
        let with_output = if exact_replay {
            !contains(&request, b"workflow_v2_failure")
        } else {
            !contains(&request, b"workflow_candidate_failure")
        };
        let wrong_request = (!exact_replay
            && contains(&request, b"workflow_candidate_bad_response"))
            || (exact_replay && contains(&request, b"workflow_v2_bad_response"));
        let output = if exact_replay && contains(&request, b"workflow_mismatch") {
            MISMATCH_OUTPUT
        } else {
            OUTPUT
        };
        let diagnostics = if is_v2 && contains(&request, b"workflow_phase_trace") {
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
        return;
    }

    match mode {
        1 => io::stdout()
            .write_all(&response(&request, WORKER_ID, true, false, OUTPUT))
            .unwrap(),
        3 => loop {
            io::stdout().write_all(&[b'x'; 8192]).unwrap()
        },
        4 => loop {
            io::stderr().write_all(&[b'x'; 8192]).unwrap()
        },
        5 => io::stdout().write_all(b"not-a-response").unwrap(),
        6 => io::stdout().write_all(b"F3LRSP01\x01").unwrap(),
        7 => exit(23),
        8 => {
            let child = spawn_descendant();
            eprintln!("descendant={}", child.id());
            loop {
                thread::sleep(Duration::from_secs(60));
            }
        }
        10 => io::stdout()
            .write_all(&response(&request, WORKER_ID, true, true, OUTPUT))
            .unwrap(),
        11 => io::stdout()
            .write_all(&response(&request, "wrong-worker", true, false, OUTPUT))
            .unwrap(),
        12 => io::stdout()
            .write_all(&response(&request, WORKER_ID, true, false, OUTPUT))
            .unwrap(),
        13 => io::stdout()
            .write_all(&response(&request, WORKER_ID, false, false, OUTPUT))
            .unwrap(),
        14 => {
            let mut environment = env::vars().collect::<Vec<_>>();
            environment.sort();
            if environment
                != [("LANG", "C"), ("LC_ALL", "C"), ("TZ", "UTC")]
                    .map(|(name, value)| (name.to_owned(), value.to_owned()))
            {
                exit(70);
            }
            io::stdout()
                .write_all(&response(&request, WORKER_ID, true, false, OUTPUT))
                .unwrap();
        }
        15 => {
            io::stderr().write_all(b"unbound diagnostic").unwrap();
            io::stdout()
                .write_all(&response(&request, WORKER_ID, true, false, OUTPUT))
                .unwrap();
        }
        16 => {
            let mut malformed = response(&request, WORKER_ID, true, false, OUTPUT);
            *malformed.last_mut().unwrap() ^= 1;
            io::stdout().write_all(&malformed).unwrap();
        }
        _ => exit(64),
    }
}

#[allow(clippy::zombie_processes)]
fn spawn_descendant() -> std::process::Child {
    // The supervisor must reap the process tree while this parent is deliberately hung.
    Command::new("/proc/self/exe")
        .arg("--descendant")
        .spawn()
        .unwrap()
}

fn response(
    request: &[u8],
    worker: &str,
    with_output: bool,
    wrong_request: bool,
    output_bytes: &[u8],
) -> Vec<u8> {
    response_with_diagnostics(
        request,
        worker,
        with_output,
        wrong_request,
        output_bytes,
        &[],
    )
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
    let is_v2 = &request[..8] == b"F3LREQ02";
    let mut request_identity: [u8; 32] = field(request, if is_v2 { 15 } else { 10 })
        .try_into()
        .unwrap();
    if wrong_request {
        request_identity[0] ^= 1;
    }
    let mut bytes = if is_v2 {
        b"F3LRSP02".to_vec()
    } else {
        b"F3LRSP01".to_vec()
    };
    push_field(&mut bytes, 1, &request_id);
    push_field(&mut bytes, 2, &request_identity);
    let offset = if is_v2 {
        push_field(&mut bytes, 3, field(request, 8));
        1
    } else {
        0
    };
    push_field(&mut bytes, 3 + offset, worker.as_bytes());
    push_field(&mut bytes, 4 + offset, &[if with_output { 9 } else { 6 }]);
    let mut diagnostic_bytes = Vec::new();
    diagnostic_bytes.extend_from_slice(&(diagnostics.len() as u32).to_le_bytes());
    for diagnostic in diagnostics {
        diagnostic_bytes.extend_from_slice(&(diagnostic.len() as u32).to_le_bytes());
        diagnostic_bytes.extend_from_slice(diagnostic.as_bytes());
    }
    push_field(&mut bytes, 5 + offset, &diagnostic_bytes);
    if with_output {
        let output_identity: [u8; 32] = Sha256::digest(output_bytes).into();
        let mut output = vec![1];
        output.extend_from_slice(&output_identity);
        output.extend_from_slice(&(output_bytes.len() as u64).to_le_bytes());
        output.extend_from_slice(output_bytes);
        push_field(&mut bytes, 6 + offset, &output);
    } else {
        push_field(&mut bytes, 6 + offset, &[0]);
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
