//! Codec/account composition only, not protected endpoint or policy approval.
use super::*;
use crate::CompilerExecutionAttestationReceiptV3 as Receipt;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

#[allow(dead_code)]
#[path = "../tests/support/native_attestation_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../tests/support/native_receipt_fixture.rs"]
mod receipt_fixture;

fn retain<T, E: fmt::Debug>(result: std::result::Result<(T, Storage), E>, b: &mut Budget<'_>) -> T {
    let (value, charge) = result.unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    value
}

fn carriage() -> CompilerExecutionReceiptCarriageV3 {
    let mut work = Work::new(10_000_000);
    let mut b = Budget::new(&mut work, 2_000_000);
    let policy_wire = fixture::policy_wire(3);
    let request_wire = fixture::request_wire(3);
    let receipt_wire = receipt_fixture::receipt_wire(3);
    b.reserve_storage(policy_wire.len() + request_wire.len() + receipt_wire.len())
        .unwrap();
    let policy = retain(Policy::decode(&policy_wire, &mut b), &mut b);
    let request = retain(Request::decode(&request_wire, &mut b), &mut b);
    let receipt = retain(Receipt::decode(&receipt_wire, &mut b), &mut b);
    let publication = retain(
        Publication::new([0x81; 32], [0x82; 32], receipt, &mut b),
        &mut b,
    );
    let ack = retain(Ack::new(&publication, [0x83; 32], &mut b), &mut b);
    retain(
        CompilerExecutionReceiptCarriageV3::new(policy, request, publication, ack, &mut b),
        &mut b,
    )
}

fn exercise<T: fmt::Debug, E: fmt::Debug>(
    bytes: &[u8],
    work: usize,
    scratch: usize,
    decode: impl Fn(&[u8], &mut Budget<'_>) -> std::result::Result<(T, Storage), E>,
    legacy: impl Fn(&[u8], &mut Budget<'_>) -> std::result::Result<(T, Storage), E>,
    inspect: impl Fn(&T),
) {
    let floor = fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5 + bytes.len();
    let mut peak = scratch;
    for case in 0..4 {
        let quota = work - usize::from(case == 1);
        let limit = floor + peak - usize::from(case == 2);
        let mut account = Owned::new(Work::new(quota), limit);
        account.with_budget(|b| {
            b.reserve_storage(floor).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let identity = b.storage_account_identity_v1();
            let result = if case == 3 {
                legacy(bytes, b)
            } else {
                decode(bytes, b)
            };
            if case == 0 {
                let (decoded, _) = result.unwrap();
                inspect(&decoded);
                assert_eq!(b.work(), work);
                peak = b.peak_storage() - floor;
            } else {
                assert!(result.is_err());
                if case == 1 {
                    assert!(b.failed_work().is_some());
                }
                if case == 2 {
                    assert!(b.failed_storage().is_some());
                }
            }
            assert_eq!(b.storage(), floor);
            assert_eq!(b.storage_limit(), limit);
            assert!(b.work_ledger_identity_v1() == ledger);
            assert_eq!(b.storage_account_identity_v1(), identity);
        });
    }
    for index in [0, bytes.len() / 2, bytes.len() - 1] {
        let mut altered = bytes.to_vec();
        altered[index] ^= 1;
        let mut account = Owned::new(Work::new(work), floor + scratch);
        account.with_budget(|b| {
            b.reserve_storage(floor).unwrap();
            assert!(decode(&altered, b).is_err());
            assert_eq!(b.storage(), floor);
        });
    }
}

#[test]
fn original_request_decode_uses_same_schema_and_account_with_exact_quotes() {
    let bytes = fixture::request_wire(3);
    exercise(
        &bytes,
        Request::COMPOSED_DECODE_WORK,
        Request::COMPOSED_DECODE_STORAGE,
        Request::decode_in_original_account_v3,
        Request::decode,
        |value| assert_eq!(value.canonical_bytes(), &bytes),
    );
}

#[test]
fn original_carriage_decode_preserves_signatures_and_every_nested_join() {
    let value = carriage();
    type Carriage = CompilerExecutionReceiptCarriageV3;
    exercise(
        value.canonical_bytes(),
        Carriage::COMPOSED_DECODE_WORK,
        Carriage::COMPOSED_DECODE_STORAGE,
        Carriage::decode_in_original_account_v3,
        Carriage::decode,
        |decoded| assert_eq!(decoded, &value),
    );
}
