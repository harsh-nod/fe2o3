use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, Module};

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 128 * 1024 * 1024;

fn actual(run: impl FnOnce(&Owner, &Owner, &[u8; 416], &mut Budget<'_>)) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(19).unwrap();
    let (input, receipt) = Owner::from_module_ref_with_verification_budget_v12(
        &Module::new("integer-claim"),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let observed =
        crate::optimize_native_neutral_kernel_ir_integer_continuation_v1(&input, &mut budget)
            .unwrap();
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    let checked = observed.try_check_and_finish_v1(&mut budget).unwrap();
    budget
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    let floor = budget.storage();
    run(
        &input,
        checked.owner(),
        checked.execution().canonical_bytes(),
        &mut budget,
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn actual_record_has_exact_shared_roster_and_inert_borrowed_fields() {
    actual(|input, output, bytes, budget| {
        let before = budget.work();
        let claim =
            read_unauthenticated_integer_continuation_claim_v1(input, output, bytes, budget)
                .unwrap();
        assert!(std::ptr::eq(claim.canonical_bytes(), bytes));
        assert_eq!(claim.declared_map_digest().as_slice(), &bytes[264..296]);
        assert_eq!(
            claim.declared_profile_work(),
            u64::from_le_bytes(bytes[96..104].try_into().unwrap())
        );
        assert!(!claim.grants_authority());
        assert_eq!(budget.work() - before, 418);
        assert_eq!(&bytes[..8], &[6, 0, 1, 0, 2, 0, 0, 0]);
        assert_eq!(bytes[296], 7);
        assert_eq!(bytes[356], 1);
    });
}

#[test]
fn fixed_control_endpoints_caps_tags_booleans_and_padding_are_closed() {
    actual(|input, output, bytes, budget| {
        for offset in (0..88)
            .chain(120..128)
            .chain(136..176)
            .chain([296, 298, 299, 356, 358, 359])
        {
            let mut changed = *bytes;
            changed[offset] ^= 1;
            assert!(
                read_unauthenticated_integer_continuation_claim_v1(input, output, &changed, budget)
                    .is_err(),
                "fixed field byte {offset}"
            );
        }
        for offset in [297, 357] {
            let mut changed = *bytes;
            changed[offset] = 2;
            assert!(matches!(
                read_unauthenticated_integer_continuation_claim_v1(input, output, &changed, budget),
                Err(Error::Pass)
            ));
        }
        for record in [&bytes[..415], &[]] {
            assert!(matches!(
                read_unauthenticated_integer_continuation_claim_v1(input, output, record, budget),
                Err(Error::Framing)
            ));
        }
        let mut extra = [0; 417];
        extra[..416].copy_from_slice(bytes);
        assert!(matches!(
            read_unauthenticated_integer_continuation_claim_v1(input, output, &extra, budget),
            Err(Error::Framing)
        ));
    });
}

#[test]
fn dynamic_work_epoch_and_map_bytes_remain_unauthenticated_u64_claims() {
    actual(|input, output, bytes, budget| {
        for offset in (88..120)
            .chain(128..136)
            .chain(176..296)
            .chain(300..356)
            .chain(360..416)
        {
            let mut changed = *bytes;
            changed[offset] ^= 255;
            assert!(
                read_unauthenticated_integer_continuation_claim_v1(input, output, &changed, budget)
                    .is_ok(),
                "dynamic field byte {offset}"
            );
        }
    });
}

#[test]
fn exact_and_short_reader_work_leave_the_inherited_storage_unchanged() {
    actual(|input, output, bytes, _| {
        for (limit, accepted) in [(418, true), (417, false), (1, false)] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 19);
            budget.reserve_storage(19).unwrap();
            let result = read_unauthenticated_integer_continuation_claim_v1(
                input,
                output,
                bytes,
                &mut budget,
            );
            assert_eq!(result.is_ok(), accepted);
            assert_eq!(budget.storage(), 19);
            assert_eq!(budget.peak_storage(), 19);
            assert_eq!(
                budget.work(),
                if accepted {
                    418
                } else if limit == 1 {
                    0
                } else {
                    2
                }
            );
        }
    });
}
