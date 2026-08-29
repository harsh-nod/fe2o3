use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::process::CommandExt,
    process::{Command, exit},
    thread,
    time::Duration,
};

const REQUEST_DOMAIN: &[u8] = b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-REQUEST/V1\0";
const EVIDENCE_DOMAIN: &[u8] = b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-EVIDENCE/V1\0";
const EVIDENCE_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-EVIDENCE-IDENTITY/V1\0";
const TRACE_DOMAIN: &[u8] = b"FE2O3/GFX942-PHYSICAL-MACHINE-TRACE-EVIDENCE/V1\0";
const ANALYSIS_BUNDLE_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-ANALYSIS-BUNDLE/V1\0";
const REQUEST_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-REQUEST-IDENTITY/V1\0";
const IDENTITY_CHALLENGE_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-IDENTITY-CHALLENGE/V1\0";
const IDENTITY_RESPONSE_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-IDENTITY-RESPONSE/V1\0";
const READY_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-WORKER-READY/V1\0";
const DONE_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-WORKER-DONE/V1\0";
const ACK_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-WORKER-ACK/V1\0";

struct Entry {
    symbol: String,
}

struct Request {
    bytes: Vec<u8>,
    challenge: [u8; 32],
    analyzer: [u8; 32],
    toolchain: [u8; 32],
    payload_digest: [u8; 32],
    payload_bytes: u64,
    entries: Vec<Entry>,
    payload: Vec<u8>,
}

pub fn run(analyzer_byte: u8, toolchain_byte: u8) {
    if run_process_group_join_helper() {
        return;
    }
    let mut arguments = std::env::args().skip(1);
    let argument = arguments.next().unwrap_or_default();
    let challenge = parse_argument_array(
        arguments.next().as_deref(),
        "--fe2o3-control-challenge=",
    )
    .unwrap_or_else(|| std::process::exit(64));
    let request_bytes = arguments
        .next()
        .and_then(|value| value.strip_prefix("--fe2o3-request-bytes=").map(str::to_owned))
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value != 0)
        .unwrap_or_else(|| std::process::exit(64));
    if arguments.next().is_some() {
        std::process::exit(64);
    }
    if std::env::var_os("FE2O3_TEST_REEXECUTED_WORKER").is_some() {
        finish_control_handshake(challenge);
        return;
    }
    write_control(std::io::stderr(), READY_DOMAIN, challenge);
    let mut input = vec![0_u8; request_bytes];
    std::io::stdin().read_exact(&mut input).unwrap();
    match argument.as_str() {
        "--machine-effects-gfx942-identities-v1" => {
            identity_response(&input, analyzer_byte, toolchain_byte, challenge)
        }
        "--machine-analysis-gfx942-v1" => analysis_response(input, challenge),
        _ => std::process::exit(64),
    }
    std::io::stdout().flush().unwrap();
    finish_control_handshake(challenge);
}

fn finish_control_handshake(challenge: [u8; 32]) {
    write_control(std::io::stderr(), DONE_DOMAIN, challenge);
    let mut ack = vec![0_u8; ACK_DOMAIN.len() + challenge.len()];
    std::io::stdin().read_exact(&mut ack).unwrap();
    if ack[..ACK_DOMAIN.len()] != *ACK_DOMAIN
        || ack[ACK_DOMAIN.len()..] != challenge
        || std::io::stdin().read(&mut [0_u8; 1]).unwrap() != 0
    {
        std::process::exit(70);
    }
}

fn parse_argument_array(value: Option<&str>, prefix: &str) -> Option<[u8; 32]> {
    let value = value?.strip_prefix(prefix)?;
    if value.len() != 64 {
        return None;
    }
    let mut result = [0_u8; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(result)
}

fn write_control(mut output: impl Write, domain: &[u8], challenge: [u8; 32]) {
    output.write_all(domain).unwrap();
    output.write_all(&challenge).unwrap();
    output.flush().unwrap();
}

fn identity_response(
    input: &[u8],
    analyzer_byte: u8,
    toolchain_byte: u8,
    control_challenge: [u8; 32],
) {
    let expected = IDENTITY_CHALLENGE_DOMAIN.len() + 4 + 2 + 32;
    if input.len() != expected || !input.starts_with(IDENTITY_CHALLENGE_DOMAIN) {
        std::process::exit(65);
    }
    let challenge: [u8; 32] = input[input.len() - 32..].try_into().unwrap();
    if challenge != control_challenge {
        std::process::exit(65);
    }
    let mut output = Vec::new();
    output.extend_from_slice(IDENTITY_RESPONSE_DOMAIN);
    push_u32(&mut output, 0);
    push_u16(&mut output, 1);
    output.extend_from_slice(&challenge);
    output.extend_from_slice(&[analyzer_byte; 32]);
    output.extend_from_slice(&[toolchain_byte; 32]);
    set_length(&mut output, IDENTITY_RESPONSE_DOMAIN.len());
    std::io::stdout().write_all(&output).unwrap();
}

fn analysis_response(bytes: Vec<u8>, control_challenge: [u8; 32]) {
    let request = parse_request(bytes).unwrap_or_else(|| std::process::exit(65));
    if request.challenge != control_challenge {
        std::process::exit(65);
    }
    match request.payload.first().copied().unwrap_or(0) {
        2 => thread::sleep(Duration::from_secs(30)),
        3 => {
            let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
            eprintln!("descendant={}", child.id());
            thread::sleep(Duration::from_secs(30));
            child.wait().unwrap();
        }
        4 => {
            std::io::stdout().write_all(&request.payload[1..]).unwrap();
            return;
        }
        8 => std::process::exit(81),
        9 => {
            let environment = std::env::vars().collect::<std::collections::BTreeMap<_, _>>();
            let expected = std::collections::BTreeMap::from([
                ("LANG".to_string(), "C".to_string()),
                ("LC_ALL".to_string(), "C".to_string()),
                ("TZ".to_string(), "UTC".to_string()),
            ]);
            if environment != expected {
                std::process::exit(82);
            }
        }
        10 => {
            use rustix::process::{Resource, getrlimit};
            for (resource, expected) in [
                (Resource::As, 4 * 1024 * 1024 * 1024),
                (Resource::Data, 2 * 1024 * 1024 * 1024),
                (Resource::Fsize, 16 * 1024 * 1024),
                (Resource::Core, 0),
                (Resource::Nproc, 0),
            ] {
                let limit = getrlimit(resource);
                if limit.current.is_none_or(|value| value > expected)
                    || limit.maximum.is_none_or(|value| value > expected)
                {
                    std::process::exit(83);
                }
            }
            let status = std::fs::read_to_string("/proc/self/status").unwrap();
            let field = |name: &str| {
                status
                    .lines()
                    .find_map(|line| line.strip_prefix(name))
                    .map(str::trim)
            };
            let uids = field("Uid:")
                .map(|value| {
                    value
                        .split_ascii_whitespace()
                        .map(str::parse::<u32>)
                        .collect::<Result<Vec<_>, _>>()
                })
                .and_then(Result::ok);
            if uids.as_deref().is_none_or(|uids| {
                uids.len() != 4 || uids[0] == 0 || !uids.iter().all(|uid| *uid == uids[0])
            })
                || field("CapInh:") != Some("0000000000000000")
                || field("CapPrm:") != Some("0000000000000000")
                || field("CapEff:") != Some("0000000000000000")
                || field("CapAmb:") != Some("0000000000000000")
                || field("NoNewPrivs:") != Some("1")
                || field("Threads:") != Some("1")
            {
                std::process::exit(86);
            }
        }
        11 => {
            let escaped = Command::new("/bin/sh")
                .args([
                    "-c",
                    "( /usr/bin/setsid /bin/sh -c '/bin/sleep 30 &' >/dev/null 2>&1 & ) & wait",
                ])
                .spawn();
            if let Ok(mut escaped) = escaped {
                eprintln!("containment_escape={}", escaped.id());
                thread::sleep(Duration::from_secs(30));
                let _ = escaped.wait();
                std::process::exit(84);
            }
        }
        12 => load_late_runtime_library(),
        14 => remap_self(false),
        15 => remap_self(true),
        17 => write_pid_and_sleep(&request.payload[1..]),
        18 => map_persistent_anonymous_executable(),
        _ => {}
    }
    let mode = request.payload[0];
    let mut effects = evidence(&request);
    let challenge_offset = EVIDENCE_DOMAIN.len() + 4 + 2;
    let analyzer_offset = challenge_offset + 32 + 32 + 8 + 32 + 8;
    match mode {
        5 => effects[analyzer_offset] ^= 1,
        7 => effects[challenge_offset] ^= 1,
        _ => {}
    }
    let trace = trace(&request, &effects);
    let mut output = analysis_bundle(&effects, &trace);
    if mode == 6 {
        output.push(0xaa);
    }
    std::io::stdout().write_all(&output).unwrap();
    if mode == 16 {
        close_stdin_after_done_and_sleep(&request.payload[1..], control_challenge);
    }
    if mode == 13 {
        reexec_from_spoofed_memfd();
    }
}

fn run_process_group_join_helper() -> bool {
    let mut arguments = std::env::args().skip(1);
    let Some(group) = arguments
        .next()
        .and_then(|value| value.strip_prefix("--fe2o3-test-join-process-group=").map(str::to_owned))
        .and_then(|value| value.parse::<i32>().ok())
        .and_then(rustix::process::Pid::from_raw)
    else {
        return false;
    };
    let result_path = arguments
        .next()
        .and_then(|value| value.strip_prefix("--fe2o3-test-result=").map(str::to_owned))
        .unwrap_or_else(|| exit(64));
    let result = match rustix::process::setpgid(None, Some(group)) {
        Ok(()) => "joined".to_string(),
        Err(error) => format!("errno={}", error.raw_os_error()),
    };
    std::fs::write(result_path, result).unwrap();
    thread::sleep(Duration::from_secs(30));
    true
}

fn write_pid_and_sleep(path: &[u8]) {
    let path = std::str::from_utf8(path).unwrap();
    std::fs::write(path, std::process::id().to_string()).unwrap();
    thread::sleep(Duration::from_secs(30));
}

#[allow(unsafe_code)]
fn close_stdin_after_done_and_sleep(path: &[u8], challenge: [u8; 32]) -> ! {
    let path = std::str::from_utf8(path).unwrap();
    std::fs::write(path, std::process::id().to_string()).unwrap();
    std::io::stdout().flush().unwrap();
    write_control(std::io::stderr(), DONE_DOMAIN, challenge);
    // SAFETY: this fixture intentionally makes the protocol ACK write fail.
    unsafe { rustix::io::close(0) };
    thread::sleep(Duration::from_secs(30));
    exit(90)
}

fn reexec_from_spoofed_memfd() -> ! {
    use rustix::fs::{MemfdFlags, Mode, fchmod, memfd_create};
    use std::os::fd::AsRawFd;

    std::io::stdout().flush().unwrap();
    let descriptor = memfd_create(
        c"fe2o3-machine-effect-worker-spoof",
        MemfdFlags::empty(),
    )
    .unwrap();
    let mut image = File::from(descriptor);
    let mut source = File::open("/proc/self/exe").unwrap();
    std::io::copy(&mut source, &mut image).unwrap();
    fchmod(&image, Mode::from_bits_retain(0o500)).unwrap();
    image.seek(SeekFrom::Start(0)).unwrap();
    let path = format!("/proc/self/fd/{}", image.as_raw_fd());
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let error = Command::new(path)
        .args(arguments)
        .env("FE2O3_TEST_REEXECUTED_WORKER", "1")
        .exec();
    eprintln!("spoofed worker re-exec failed: {error}");
    exit(87)
}

#[allow(unsafe_code)]
fn remap_self(transient: bool) {
    use std::os::fd::AsRawFd;

    unsafe extern "C" {
        fn mmap(
            address: *mut std::ffi::c_void,
            length: usize,
            protection: std::ffi::c_int,
            flags: std::ffi::c_int,
            descriptor: std::ffi::c_int,
            offset: i64,
        ) -> *mut std::ffi::c_void;
        fn munmap(address: *mut std::ffi::c_void, length: usize) -> std::ffi::c_int;
    }
    const PROT_READ: std::ffi::c_int = 1;
    const MAP_PRIVATE: std::ffi::c_int = 2;
    const LENGTH: usize = 4096;
    let image = File::open("/proc/self/exe").unwrap();
    let mapping = unsafe {
        mmap(
            std::ptr::null_mut(),
            LENGTH,
            PROT_READ,
            MAP_PRIVATE,
            image.as_raw_fd(),
            0,
        )
    };
    if mapping as isize == -1 {
        exit(88);
    }
    if transient && unsafe { munmap(mapping, LENGTH) } != 0 {
        exit(89);
    }
}

#[allow(unsafe_code)]
fn map_persistent_anonymous_executable() {
    unsafe extern "C" {
        fn mmap(
            address: *mut std::ffi::c_void,
            length: usize,
            protection: std::ffi::c_int,
            flags: std::ffi::c_int,
            descriptor: std::ffi::c_int,
            offset: i64,
        ) -> *mut std::ffi::c_void;
    }
    const PROT_READ: std::ffi::c_int = 1;
    const PROT_WRITE: std::ffi::c_int = 2;
    const PROT_EXEC: std::ffi::c_int = 4;
    const MAP_PRIVATE: std::ffi::c_int = 2;
    const MAP_ANONYMOUS: std::ffi::c_int = 0x20;
    let mapping = unsafe {
        mmap(
            std::ptr::null_mut(),
            4096,
            PROT_READ | PROT_WRITE | PROT_EXEC,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if mapping as isize == -1 {
        exit(90);
    }
}

#[allow(unsafe_code)]
fn load_late_runtime_library() {
    unsafe extern "C" {
        fn dlopen(path: *const std::ffi::c_char, flags: std::ffi::c_int) -> *mut std::ffi::c_void;
    }
    const RTLD_NOW: std::ffi::c_int = 2;
    const RTLD_LOCAL: std::ffi::c_int = 0;
    let path = c"/lib/x86_64-linux-gnu/libutil.so.1";
    // The handle is deliberately retained so the second maps observation must
    // detect the newly loaded DSO.
    let handle = unsafe { dlopen(path.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
    if handle.is_null() {
        std::process::exit(85);
    }
}

fn parse_request(bytes: Vec<u8>) -> Option<Request> {
    if !bytes.starts_with(REQUEST_DOMAIN) {
        return None;
    }
    let mut position = REQUEST_DOMAIN.len();
    if take_u32(&bytes, &mut position)? as usize != bytes.len()
        || take_u16(&bytes, &mut position)? != 1
    {
        return None;
    }
    let challenge = take_array(&bytes, &mut position)?;
    let analyzer = take_array(&bytes, &mut position)?;
    let toolchain = take_array(&bytes, &mut position)?;
    let payload_digest = take_array(&bytes, &mut position)?;
    let payload_bytes = take_u64(&bytes, &mut position)?;
    let entry_count = take_u16(&bytes, &mut position)? as usize;
    let mut entries = Vec::new();
    for _ in 0..entry_count {
        let length = take_u16(&bytes, &mut position)? as usize;
        let symbol = std::str::from_utf8(bytes.get(position..position + length)?)
            .ok()?
            .to_string();
        position += length;
        position = position.checked_add(5 * 4)?;
        if position > bytes.len() {
            return None;
        }
        entries.push(Entry { symbol });
    }
    let payload = bytes.get(position..)?.to_vec();
    if payload.len() as u64 != payload_bytes || Sha256::digest(&payload)[..] != payload_digest {
        return None;
    }
    Some(Request {
        bytes,
        challenge,
        analyzer,
        toolchain,
        payload_digest,
        payload_bytes,
        entries,
        payload,
    })
}

fn evidence(request: &Request) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(EVIDENCE_DOMAIN);
    push_u32(&mut output, 0);
    push_u16(&mut output, 1);
    output.extend_from_slice(&request.challenge);
    output.extend_from_slice(&domain_hash(REQUEST_IDENTITY_DOMAIN, &request.bytes));
    push_u64(&mut output, request.bytes.len() as u64);
    output.extend_from_slice(&request.payload_digest);
    push_u64(&mut output, request.payload_bytes);
    output.extend_from_slice(&request.analyzer);
    output.extend_from_slice(&request.toolchain);
    push_u16(&mut output, 1);
    push_u16(&mut output, request.entries.len() as u16);
    for (index, entry) in request.entries.iter().enumerate() {
        push_text(&mut output, &entry.symbol);
        output.extend_from_slice(&[0x33 + index as u8; 32]);
        push_u64(&mut output, 0);
        push_u64(&mut output, request.payload_bytes);
    }
    push_u32(&mut output, request.entries.len() as u32);
    for entry in &request.entries {
        push_text(&mut output, &entry.symbol);
        push_u64(&mut output, 0);
        push_u64(&mut output, request.payload_bytes);
        push_u16(&mut output, 0);
    }
    let effect_count = request.entries.len() as u32;
    push_u32(&mut output, effect_count);
    for entry in &request.entries {
        let base = 0;
        push_effect(&mut output, &entry.symbol, base, 4, 0);
    }
    set_length(&mut output, EVIDENCE_DOMAIN.len());
    output
}

fn trace(request: &Request, effects: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(TRACE_DOMAIN);
    push_u32(&mut output, 0);
    push_u16(&mut output, 1);
    output.extend_from_slice(&request.challenge);
    output.extend_from_slice(&domain_hash(REQUEST_IDENTITY_DOMAIN, &request.bytes));
    push_u64(&mut output, request.bytes.len() as u64);
    output.extend_from_slice(&domain_hash(EVIDENCE_IDENTITY_DOMAIN, effects));
    push_u64(&mut output, effects.len() as u64);
    output.extend_from_slice(&request.payload_digest);
    push_u64(&mut output, request.payload_bytes);
    output.extend_from_slice(&request.analyzer);
    output.extend_from_slice(&request.toolchain);
    push_u16(&mut output, 1);

    push_u32(&mut output, request.entries.len() as u32);
    for entry in &request.entries {
        push_text(&mut output, &entry.symbol);
        push_u32(&mut output, 0);
        push_u64(&mut output, 0);
        push_u32(&mut output, 1);
        push_u16(&mut output, 0);
    }

    push_u32(&mut output, request.entries.len() as u32);
    for entry in &request.entries {
        push_text(&mut output, &entry.symbol);
        push_u64(&mut output, 0);
        push_u32(&mut output, 0);
        push_text(&mut output, "S_ENDPGM");
        push_u16(&mut output, request.payload.len() as u16);
        output.extend_from_slice(&request.payload);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        output.push(4);
        push_u64(&mut output, 0);
        push_u16(&mut output, 1 << 2);
        output.push(0);
        push_u16(&mut output, 0);
    }
    set_length(&mut output, TRACE_DOMAIN.len());
    output
}

fn analysis_bundle(effects: &[u8], trace: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(ANALYSIS_BUNDLE_DOMAIN);
    push_u32(&mut output, 0);
    push_u16(&mut output, 1);
    push_u32(&mut output, effects.len() as u32);
    output.extend_from_slice(effects);
    push_u32(&mut output, trace.len() as u32);
    output.extend_from_slice(trace);
    set_length(&mut output, ANALYSIS_BUNDLE_DOMAIN.len());
    output
}

fn push_effect(
    output: &mut Vec<u8>,
    symbol: &str,
    instruction_offset: u64,
    kind: u8,
    width: u16,
) {
    push_text(output, symbol);
    push_text(output, symbol);
    push_u64(output, instruction_offset);
    output.push(kind);
    push_u16(output, width);
}

fn take_u16(bytes: &[u8], position: &mut usize) -> Option<u16> {
    let result = u16::from_le_bytes(bytes.get(*position..*position + 2)?.try_into().ok()?);
    *position += 2;
    Some(result)
}

fn take_u32(bytes: &[u8], position: &mut usize) -> Option<u32> {
    let result = u32::from_le_bytes(bytes.get(*position..*position + 4)?.try_into().ok()?);
    *position += 4;
    Some(result)
}

fn take_u64(bytes: &[u8], position: &mut usize) -> Option<u64> {
    let result = u64::from_le_bytes(bytes.get(*position..*position + 8)?.try_into().ok()?);
    *position += 8;
    Some(result)
}

fn take_array(bytes: &[u8], position: &mut usize) -> Option<[u8; 32]> {
    let result = bytes.get(*position..*position + 32)?.try_into().ok()?;
    *position += 32;
    Some(result)
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    push_u16(output, value.len() as u16);
    output.extend_from_slice(value.as_bytes());
}

fn set_length(output: &mut [u8], domain: usize) {
    let length = output.len() as u32;
    output[domain..domain + 4].copy_from_slice(&length.to_le_bytes());
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    hash.finalize().into()
}
