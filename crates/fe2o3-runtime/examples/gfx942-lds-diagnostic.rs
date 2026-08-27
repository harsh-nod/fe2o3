#![allow(unsafe_code)]

//! Exact-artifact direct-KFD diagnosis. This is not a production authority path.

use std::{env, fs, process};

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_kfd::{
    DeviceSelector, GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_SHA256_V1, Gfx942KfdDispatchBufferV1,
    Gfx942KfdDispatchPointerFixupV1, OpenedKfd, execute_gfx942_kfd_dispatch_unchecked_v1,
};
use fe2o3_runtime::{Gfx942RuntimeDispatchInputsV1, prepare_gfx942_runtime_dispatch_v1};

const USAGE: &str = "usage: gfx942-lds-diagnostic <unique-id> <exact-hsaco-path>";
const EXPECTED_HSACO_SHA256_HEX: &str =
    "ab6bda1e8af05b61c22753382e75dd6a9952db8e598eaac3cb5769863a618ed0";
const KERNEL: &str = "lds_publish_read_reduce_i32_v1";
const CANARY_BYTES: usize = 64;
const INPUT_VALUES: usize = 64;
const EXPECTED_SUM: i32 = 2_080;

fn parse_unique_id(value: &str) -> Result<u64, String> {
    value
        .strip_prefix("0x")
        .map_or_else(|| value.parse(), |hex| u64::from_str_radix(hex, 16))
        .map_err(|error| format!("invalid unique ID `{value}`: {error}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn canaries_unchanged(bytes: &[u8], payload_offset: usize, payload_bytes: usize) -> bool {
    bytes[..payload_offset].iter().all(|byte| *byte == 0xa5)
        && bytes[payload_offset + payload_bytes..]
            .iter()
            .all(|byte| *byte == 0xa5)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let unique_id = parse_unique_id(&args.next().ok_or(USAGE)?)?;
    let hsaco_path = args.next().ok_or(USAGE)?;
    if args.next().is_some() {
        return Err(USAGE.into());
    }
    let hsaco = fs::read(&hsaco_path)?;
    let actual_sha256 = sha256_hex(&hsaco);
    if actual_sha256 != EXPECTED_HSACO_SHA256_HEX {
        return Err(format!(
            "refusing non-pinned HSACO: expected {EXPECTED_HSACO_SHA256_HEX}, got {actual_sha256}"
        )
        .into());
    }

    let input_payload_bytes = INPUT_VALUES * core::mem::size_of::<i32>();
    let mut input = vec![0xa5; CANARY_BYTES + input_payload_bytes + CANARY_BYTES];
    for (index, value) in (1_i32..=INPUT_VALUES as i32).enumerate() {
        let offset = CANARY_BYTES + index * core::mem::size_of::<i32>();
        input[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    let mut output = vec![0xa5; CANARY_BYTES + core::mem::size_of::<i32>() + CANARY_BYTES];
    output[CANARY_BYTES..CANARY_BYTES + 4].copy_from_slice(&i32::MIN.to_le_bytes());
    let mut explicit = vec![0_u8; 32];
    explicit[8..16].copy_from_slice(&(INPUT_VALUES as u64).to_le_bytes());
    explicit[24..32].copy_from_slice(&1_u64.to_le_bytes());
    let prepared = prepare_gfx942_runtime_dispatch_v1(
        &hsaco,
        KERNEL,
        Gfx942RuntimeDispatchInputsV1::new(
            explicit,
            vec![
                Gfx942KfdDispatchBufferV1::new(input)?,
                Gfx942KfdDispatchBufferV1::new(output)?,
            ],
            vec![
                Gfx942KfdDispatchPointerFixupV1::new(0, 0, CANARY_BYTES, 4),
                Gfx942KfdDispatchPointerFixupV1::new(16, 1, CANARY_BYTES, 4),
            ],
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1])
                .map_err(|error| format!("invalid diagnostic geometry: {error:?}"))?,
            0,
            5_000,
        ),
    )?;
    let closure_sha256 = prepared
        .identity()
        .closure_sha256()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;

    // SAFETY: this diagnostic is limited to one SHA-pinned, loader-inspected
    // artifact and its exact reviewed ABI/buffers. It deliberately does not
    // construct or claim the missing Worker V3 production authority.
    let result = match unsafe {
        execute_gfx942_kfd_dispatch_unchecked_v1(device, prepared.into_unchecked_kfd_request())
    } {
        Ok(result) => result,
        Err(error) => {
            eprintln!("terminal direct-KFD diagnostic failure: {error}");
            process::abort();
        }
    };
    let [input, output]: [Gfx942KfdDispatchBufferV1; 2] = result
        .into_buffers()
        .try_into()
        .map_err(|_| "runtime returned the wrong buffer cardinality")?;
    let input = input.into_bytes();
    let output = output.into_bytes();
    if !canaries_unchanged(&input, CANARY_BYTES, input_payload_bytes)
        || !canaries_unchanged(&output, CANARY_BYTES, 4)
    {
        return Err("GPU dispatch changed a canary".into());
    }
    let observed = i32::from_le_bytes(
        output[CANARY_BYTES..CANARY_BYTES + 4]
            .try_into()
            .expect("exact output range"),
    );
    if observed != EXPECTED_SUM {
        return Err(
            format!("wrong reduction result: expected {EXPECTED_SUM}, got {observed}").into(),
        );
    }
    println!(
        "status=measured-diagnostic-only target=gfx942:xnack- unique_id={unique_id:016x} kernel={KERNEL} hsaco_sha256={actual_sha256} closure_sha256={closure_sha256} kfd_dispatch_profile_sha256={} result={observed} canaries=preserved authority=none",
        GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_SHA256_V1,
    );
    Ok(())
}
