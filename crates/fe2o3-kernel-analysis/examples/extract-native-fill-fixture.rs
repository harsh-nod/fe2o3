//! Extract a test-only Worker payload through the canonical request decoder.
use fe2o3_kernel_analysis::PhysicalMachineEffectRequestV1;
use sha2::{Digest, Sha256};
use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(args.next().ok_or("expected output HSACO path")?);
    if args.next().is_some() {
        return Err("expected exactly one output HSACO path".into());
    }
    let bytes = include_bytes!("../src/gfx942_fill_analysis_v1/fill.request");
    let request = PhysicalMachineEffectRequestV1::decode_canonical(bytes)
        .map_err(|_| "retained canonical Worker request was rejected")?;
    let payload = request.exact_payload_bytes();
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output, payload)?;
    println!("request_sha256={}", hex(Sha256::digest(bytes).into()));
    println!("payload_sha256={}", hex(Sha256::digest(payload).into()));
    println!("payload_bytes={}", payload.len());
    Ok(())
}

fn hex(bytes: [u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    result
}
