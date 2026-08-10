use sha2::{Digest, Sha256};
use std::{io::{Read, Write}, process::Command, thread, time::Duration};

const REQUEST_DOMAIN: &[u8] = b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-REQUEST/V1\0";
const EVIDENCE_DOMAIN: &[u8] = b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-EVIDENCE/V1\0";
const REQUEST_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-REQUEST-IDENTITY/V1\0";
const IDENTITY_CHALLENGE_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-IDENTITY-CHALLENGE/V1\0";
const IDENTITY_RESPONSE_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-IDENTITY-RESPONSE/V1\0";

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
    let argument = std::env::args().nth(1).unwrap_or_default();
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    match argument.as_str() {
        "--machine-effects-gfx942-identities-v1" => {
            identity_response(&input, analyzer_byte, toolchain_byte)
        }
        "--machine-effects-gfx942-v1" => analysis_response(input),
        _ => std::process::exit(64),
    }
}

fn identity_response(input: &[u8], analyzer_byte: u8, toolchain_byte: u8) {
    let expected = IDENTITY_CHALLENGE_DOMAIN.len() + 4 + 2 + 32;
    if input.len() != expected || !input.starts_with(IDENTITY_CHALLENGE_DOMAIN) {
        std::process::exit(65);
    }
    let challenge: [u8; 32] = input[input.len() - 32..].try_into().unwrap();
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

fn analysis_response(bytes: Vec<u8>) {
    let request = parse_request(bytes).unwrap_or_else(|| std::process::exit(65));
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
        _ => {}
    }
    let mode = request.payload[0];
    let mut output = evidence(&request);
    let challenge_offset = EVIDENCE_DOMAIN.len() + 4 + 2;
    let analyzer_offset = challenge_offset + 32 + 32 + 8 + 32 + 8;
    match mode {
        5 => output[analyzer_offset] ^= 1,
        6 => output.push(0xaa),
        7 => output[challenge_offset] ^= 1,
        _ => {}
    }
    std::io::stdout().write_all(&output).unwrap();
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
        push_u64(&mut output, 0x100 + index as u64 * 0x100);
        push_u64(&mut output, 0x40);
    }
    push_u32(&mut output, request.entries.len() as u32);
    for (index, entry) in request.entries.iter().enumerate() {
        push_text(&mut output, &entry.symbol);
        push_u64(&mut output, 0x100 + index as u64 * 0x100);
        push_u64(&mut output, 0x40);
        push_u16(&mut output, 0);
    }
    push_u32(&mut output, request.entries.len() as u32);
    for (index, entry) in request.entries.iter().enumerate() {
        push_text(&mut output, &entry.symbol);
        push_text(&mut output, &entry.symbol);
        push_u64(&mut output, 0x100 + index as u64 * 0x100);
        output.push(4);
        push_u16(&mut output, 0);
    }
    set_length(&mut output, EVIDENCE_DOMAIN.len());
    output
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
