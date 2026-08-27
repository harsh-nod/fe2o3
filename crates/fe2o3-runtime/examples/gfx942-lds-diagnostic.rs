#![allow(unsafe_code)]

//! Exact-artifact direct-KFD diagnosis. This is not a production authority path.

use std::{convert::Infallible, env, fs};

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_kfd::{
    DeviceSelector, GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_SHA256_V1,
    Gfx942KfdDispatchPointerFixupV1, OpenedKfd,
};
use fe2o3_runtime::{
    Gfx942RuntimeBufferAccessV1, Gfx942RuntimeDispatchBufferV1, Gfx942RuntimeDispatchInputsV1,
    WorkerV3Gfx942ExecutionAuthorityV1, execute_authorized_gfx942_runtime_dispatch_v1,
    prepare_gfx942_runtime_dispatch_v1,
};

const USAGE: &str = "usage: gfx942-lds-diagnostic <unique-id> <exact-hsaco-path>";
const EXPECTED_HSACO_SHA256_HEX: &str =
    "ab6bda1e8af05b61c22753382e75dd6a9952db8e598eaac3cb5769863a618ed0";
const KERNEL: &str = "lds_publish_read_reduce_i32_v1";
const CANARY_BYTES: usize = 64;
const INPUT_VALUES: usize = 64;
const EXPECTED_SUM: i32 = 2_080;

struct PinnedDiagnosticAuthorityV1 {
    finalized_hsaco_sha256: [u8; 32],
    finalized_hsaco_length: u64,
    dispatch_contract_sha256: [u8; 32],
    device_unique_id: u64,
}

// SAFETY: this opt-in diagnostic deliberately substitutes one manually audited, SHA-pinned test
// kernel for the absent production Worker V3 verifier. It binds the complete prepared invocation
// and selected device below, runs synchronously, and is unavailable from the library API. It is
// diagnostic evidence only and must never be copied into production authority code.
unsafe impl WorkerV3Gfx942ExecutionAuthorityV1 for PinnedDiagnosticAuthorityV1 {
    type CurrentnessError = Infallible;

    fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.finalized_hsaco_sha256
    }

    fn finalized_hsaco_length(&self) -> u64 {
        self.finalized_hsaco_length
    }

    fn kernel_name(&self) -> &str {
        KERNEL
    }

    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.dispatch_contract_sha256
    }

    fn device_unique_id(&self) -> u64 {
        self.device_unique_id
    }

    fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError> {
        Ok(())
    }
}

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
                Gfx942RuntimeDispatchBufferV1::new(input, Gfx942RuntimeBufferAccessV1::ReadOnly)?,
                Gfx942RuntimeDispatchBufferV1::new(output, Gfx942RuntimeBufferAccessV1::ReadWrite)?,
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
    let authority = PinnedDiagnosticAuthorityV1 {
        finalized_hsaco_sha256: prepared.identity().object_sha256(),
        finalized_hsaco_length: prepared.finalized_hsaco_length(),
        dispatch_contract_sha256: prepared.dispatch_contract_sha256(),
        device_unique_id: unique_id,
    };
    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;

    let result = execute_authorized_gfx942_runtime_dispatch_v1(authority, device, prepared)?;
    let [input, output] = result
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
        "status=measured-diagnostic-only target=gfx942:xnack- unique_id={unique_id:016x} kernel={KERNEL} hsaco_sha256={actual_sha256} closure_sha256={closure_sha256} kfd_dispatch_profile_sha256={} result={observed} canaries=preserved authority=none runtime_gate=unsafe-diagnostic",
        GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_SHA256_V1,
    );
    Ok(())
}
