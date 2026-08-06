use std::io::{self, Read, Write};

use sha2::{Digest, Sha256};

const WORKER_ID: &str = "fixture-worker-v2-hsaco-v1";
const PAYLOAD_MARKER: &[u8] = b"FE2O3/TEST-HSACO-PAYLOAD/V1\0";

fn main() {
    let mut request = Vec::new();
    io::stdin().read_to_end(&mut request).unwrap();
    let is_v2 = request.get(..8) == Some(b"F3LREQ02");
    let output = if is_v2 {
        hsaco_payload(input_payload(field(&request, 9)))
    } else {
        find_hsaco_input(field(&request, 6))
    };
    io::stdout()
        .write_all(&response(&request, is_v2, output))
        .unwrap();
}

fn response(request: &[u8], is_v2: bool, output_bytes: &[u8]) -> Vec<u8> {
    let request_id = field(request, 1);
    let request_identity = field(request, if is_v2 { 15 } else { 10 });
    let mut bytes = if is_v2 {
        b"F3LRSP02".to_vec()
    } else {
        b"F3LRSP01".to_vec()
    };
    push_field(&mut bytes, 1, request_id);
    push_field(&mut bytes, 2, request_identity);
    let offset = if is_v2 {
        push_field(&mut bytes, 3, field(request, 8));
        1
    } else {
        0
    };
    push_field(&mut bytes, 3 + offset, WORKER_ID.as_bytes());
    push_field(&mut bytes, 4 + offset, &[9]);
    push_field(&mut bytes, 5 + offset, &0_u32.to_le_bytes());
    let mut output = vec![1];
    output.extend_from_slice(&<[u8; 32]>::from(Sha256::digest(output_bytes)));
    output.extend_from_slice(&(output_bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(output_bytes);
    push_field(&mut bytes, 6 + offset, &output);
    bytes
}

fn find_hsaco_input(inputs: &[u8]) -> &[u8] {
    let count = u32::from_le_bytes(inputs[..4].try_into().unwrap()) as usize;
    let mut offset = 4;
    for _ in 0..count {
        offset += 1 + 32;
        let length = u64::from_le_bytes(inputs[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;
        let payload = &inputs[offset..offset + length];
        if let Some(payload) = payload
            .windows(PAYLOAD_MARKER.len())
            .position(|window| window == PAYLOAD_MARKER)
            .map(|position| &payload[position + PAYLOAD_MARKER.len()..])
        {
            return payload;
        }
        offset += length;
    }
    panic!("generic request contains no embedded HSACO compiler module")
}

fn input_payload(input: &[u8]) -> &[u8] {
    let length = u64::from_le_bytes(input[33..41].try_into().unwrap()) as usize;
    &input[41..41 + length]
}

fn hsaco_payload(input: &[u8]) -> &[u8] {
    let position = input
        .windows(PAYLOAD_MARKER.len())
        .position(|window| window == PAYLOAD_MARKER)
        .expect("V2 compiler module contains no embedded HSACO");
    &input[position + PAYLOAD_MARKER.len()..]
}

fn field(bytes: &[u8], wanted: u16) -> &[u8] {
    let mut offset = 8;
    while offset < bytes.len() {
        let tag = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
        let length = u32::from_le_bytes(bytes[offset + 2..offset + 6].try_into().unwrap()) as usize;
        offset += 6;
        if tag == wanted {
            return &bytes[offset..offset + length];
        }
        offset += length;
    }
    panic!("missing field {wanted}")
}

fn push_field(bytes: &mut Vec<u8>, tag: u16, value: &[u8]) {
    bytes.extend_from_slice(&tag.to_le_bytes());
    bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
    bytes.extend_from_slice(value);
}
