//! Inert canonical fixture through the actual closed loader; not source custody.
use super::*;
use fe2o3_kernel_ir as physical_lds_exchange_fixture_ir;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, encode_module_v22};
#[path = "../../../fe2o3-kernel-ir/tests/fixtures/physical_lds_exchange_v22.rs"]
mod fixture;
const WORK: usize = 1 << 29;
const STORAGE: usize = 512 * 1024 * 1024;
fn request() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schema":"fe2o3-simulation-request-v1","kernel":"physical_lds_exchange_fixture",
        "grid":[128,1,1],"workgroup":[128,1,1],
        "arguments":[
            {"kind":"buffer","element":"u32","access":"read_only","alignment":4,
                "bytes":format!("0x{}","5a".repeat(512)),"initialized":format!("0x{}","ff".repeat(64))},
            {"kind":"buffer","element":"u32","access":"read_write","alignment":4,
                "bytes":format!("0x{}","a5".repeat(516)),"initialized":format!("0x{}","00".repeat(65))}
        ]
    })).unwrap()
}
fn bytes() -> Vec<u8> {
    encode_module_v22(&fixture::module()).unwrap()
}
fn admit(
    kir: &[u8],
    request: &[u8],
    wl: usize,
    sl: usize,
) -> (
    Result<PhysicalLdsExchangeDebugInputV22, Error>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let mut work = Work::new(wl);
    let mut b = Budget::new(&mut work, sl);
    b.reserve_storage(73).unwrap();
    b.charge_work(29).unwrap();
    let result = b.with_prepaid_scope(73, 0, 0, 0, |b| {
        b.reserve_storage(input_envelope()?)?;
        load_bytes(kir, request, b)
    });
    assert_eq!(b.storage(), 73);
    let charged = b.work();
    let peak = b.peak_storage();
    let failed_storage = b.failed_storage();
    (result, charged, peak, work.failed_work(), failed_storage)
}
#[test]
fn exact_typed_owner_and_request_digest_are_retained() {
    let request = request();
    let (result, _, _, _, _) = admit(&bytes(), &request, WORK, STORAGE);
    let input = result.unwrap();
    assert_eq!(
        input.canonical().identity().digest(),
        input.module().identity().digest()
    );
    assert_eq!(input.module().identity().wire_version(), 22);
    assert_eq!(
        input.request_digest(),
        &<[u8; 32]>::from(Sha256::digest(&request))
    );
    assert_eq!(input.request_bytes(), request.len());
    assert_eq!(input.request().arguments.len(), 2);
}
#[test]
fn wrong_versions_are_not_admitted_as_v22() {
    for version in [19u16, 20, 21, 23] {
        let mut kir = bytes();
        kir[8..10].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            admit(&kir, &request(), WORK, STORAGE).0.err(),
            Some(Error::WrongVersion)
        );
    }
}
#[test]
fn exact_work_and_storage_boundaries_preserve_floor_and_denial() {
    let (result, work, peak, _, _) = admit(&bytes(), &request(), WORK, STORAGE);
    assert!(result.is_ok());
    assert!(admit(&bytes(), &request(), work, peak).0.is_ok());
    let short_work = admit(&bytes(), &request(), work - 1, peak);
    assert!(short_work.0.is_err() && short_work.3.is_some());
    let short_storage = admit(&bytes(), &request(), work, peak - 1);
    assert!(short_storage.0.is_err() && short_storage.4.is_some());
}
#[test]
fn launch_and_extra_logical_parameter_refuse() {
    for mode in 0..5 {
        let mut r: serde_json::Value = serde_json::from_slice(&request()).unwrap();
        match mode {
            0 => r["grid"] = serde_json::json!([64, 1, 1]),
            1 => r["grid"] = serde_json::json!([256, 1, 1]),
            2 => r["workgroup"] = serde_json::json!([64, 1, 1]),
            3 => {
                let x = r["arguments"][0].clone();
                r["arguments"].as_array_mut().unwrap().push(x);
            }
            _ => r["source_authority"] = serde_json::json!("claimed"),
        }
        assert_eq!(
            admit(&bytes(), &serde_json::to_vec(&r).unwrap(), WORK, STORAGE)
                .0
                .err(),
            Some(Error::Request)
        );
    }
}
#[test]
fn byte_and_profile_limits_remain_closed() {
    assert_eq!(
        admit(
            &vec![0; MAX_PHYSICAL_LDS_EXCHANGE_DEBUG_KIR_BYTES_V22 + 1],
            &request(),
            WORK,
            STORAGE
        )
        .0
        .err(),
        Some(Error::Input)
    );
    assert_eq!(
        admit(
            &bytes(),
            &vec![0; MAX_PHYSICAL_LDS_EXCHANGE_DEBUG_REQUEST_BYTES_V22 + 1],
            WORK,
            STORAGE
        )
        .0
        .err(),
        Some(Error::Input)
    );
    let mut kir = bytes();
    kir.pop();
    assert!(admit(&kir, &request(), WORK, STORAGE).0.is_err());
    assert!(common::envelope(usize::MAX).is_err());
}
