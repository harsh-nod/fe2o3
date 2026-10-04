//! Genuine retained Policy6 owners; no active-pipeline or peak-memory claim.
use super::*;
use crate::{
    checked_load_forwarding_v1::tests::{fixture, with_owner},
    continue_checked_canonical_kernel_ir_policy6_v1,
    optimize_checked_canonical_kernel_ir_policy5_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, Module,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
type Policy6 = CheckedCanonicalKernelIrOwnerPolicy6V1;
fn limits(bytes: Option<usize>, items: usize) -> Limits {
    Limits {
        max_bytes: bytes,
        max_items: items,
    }
}
fn with_chain(module: Module, body: impl FnOnce(&Owner, &Policy6, &mut Budget<'_>)) {
    with_owner(module, |input, budget| {
        let prefix = optimize_checked_canonical_kernel_ir_policy5_v1(input, budget).unwrap();
        budget.reserve_storage(prefix.retained_storage()).unwrap();
        // The ordinary consuming continuation restores the pre-prefix floor.
        let owner = continue_checked_canonical_kernel_ir_policy6_v1(input, prefix, budget).unwrap();
        let retained = owner.retained_storage();
        budget.reserve_storage(retained).unwrap();
        body(input, &owner, budget);
        drop(owner);
        budget.release_storage(retained).unwrap();
    });
}
fn observe(owner: &Policy6) -> Policy6RetainedLogicalStorageV1 {
    owner
        .retained_logical_storage_v11(limits(None, 100_000))
        .unwrap()
}

#[test]
fn genuine_nonempty_chain_counts_prefix_continuation_and_one_enclosing_header() {
    with_chain(fixture(), |input, owner, budget| {
        assert_eq!(owner.intermediate_policy5().load_forwarding_rows().len(), 2);
        let before = (
            input.canonical().canonical_bytes().to_vec(),
            owner.owner().canonical().canonical_bytes().to_vec(),
            owner.execution().canonical_bytes().to_vec(),
            owner.retained_storage(),
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
        );
        let report = observe(owner);
        let mut prefix = Counter::new(limits(None, 100_000));
        owner.prefix.charge_retained_heap_v11(&mut prefix).unwrap();
        let mut continuation = Counter::new(limits(None, 100_000));
        owner
            .continuation
            .charge_retained_heap_v11(&mut continuation)
            .unwrap();
        assert_eq!(report.inline_bytes, size_of::<Policy6>());
        assert_eq!(report.prefix_owned_bytes, prefix.bytes());
        assert_eq!(report.continuation_owned_bytes, continuation.bytes());
        assert_eq!(
            report.total_bytes,
            size_of::<Policy6>() + prefix.bytes() + continuation.bytes()
        );
        assert_eq!(
            report.visited_items,
            1 + prefix.items() + continuation.items()
        );
        assert_eq!(
            owner.native_input_audit_bytes(),
            input.canonical().canonical_bytes()
        );
        assert_eq!(
            owner.continuation.native_input_audit_bytes(),
            owner.prefix.owner().canonical().canonical_bytes()
        );
        assert_eq!(input.canonical().canonical_bytes(), before.0);
        assert_eq!(owner.owner().canonical().canonical_bytes(), before.1);
        assert_eq!(owner.execution().canonical_bytes().as_slice(), before.2);
        assert_eq!(
            (
                owner.retained_storage(),
                budget.work(),
                budget.storage(),
                budget.peak_storage()
            ),
            (before.3, before.4, before.5, before.6)
        );
        assert!(!owner.grants_authority());
    });
}

#[test]
fn exact_and_one_short_standalone_limits_never_return_partial_reports() {
    with_chain(fixture(), |_, owner, _| {
        let report = observe(owner);
        assert_eq!(
            owner.retained_logical_storage_v11(limits(
                Some(report.total_bytes),
                report.visited_items
            )),
            Ok(report)
        );
        assert_eq!(
            owner.retained_logical_storage_v11(limits(
                Some(report.total_bytes - 1),
                report.visited_items
            )),
            Err(Error::ByteLimit)
        );
        assert_eq!(
            owner.retained_logical_storage_v11(limits(None, report.visited_items - 1)),
            Err(Error::ItemLimit)
        );
        assert_eq!(
            owner.retained_logical_storage_v11(limits(None, 0)),
            Err(Error::ItemLimit)
        );
        assert_eq!(
            owner.retained_logical_storage_v11(limits(Some(size_of::<Policy6>() - 1), 100_000)),
            Err(Error::ByteLimit)
        );
    });
}

#[test]
fn heap_composition_replaces_not_duplicates_the_inline_header_and_root() {
    with_chain(fixture(), |_, owner, _| {
        let report = observe(owner);
        let enclosing_header = size_of::<(Policy6, [u8; 17])>();
        let expected = enclosing_header + report.total_bytes - report.inline_bytes;
        let mut counter = Counter::new(limits(Some(expected), report.visited_items));
        counter.charge(enclosing_header, 1).unwrap();
        owner.charge_retained_heap_v11(&mut counter).unwrap();
        assert_eq!(
            (counter.bytes(), counter.items()),
            (expected, report.visited_items)
        );
        let mut heap = Counter::new(limits(None, report.visited_items - 1));
        owner.charge_retained_heap_v11(&mut heap).unwrap();
        assert_eq!(
            (heap.bytes(), heap.items()),
            (
                report.total_bytes - report.inline_bytes,
                report.visited_items - 1
            )
        );
    });
}

#[test]
fn exhausted_enclosing_limits_fail_before_any_unbounded_traversal() {
    with_chain(fixture(), |_, owner, _| {
        for (limits, expected) in [
            (limits(None, 0), Error::ItemLimit),
            (limits(Some(0), 100_000), Error::ByteLimit),
        ] {
            let mut counter = Counter::new(limits);
            assert_eq!(owner.charge_retained_heap_v11(&mut counter), Err(expected));
            assert_eq!((counter.bytes(), counter.items()), (0, 0));
        }
        let mut counter = Counter::new(limits(None, usize::MAX));
        counter.charge(usize::MAX, 0).unwrap();
        assert_eq!(
            owner.charge_retained_heap_v11(&mut counter),
            Err(Error::Arithmetic)
        );
        assert_eq!((counter.bytes(), counter.items()), (usize::MAX, 0));
    });
}

#[test]
fn identical_bytes_still_count_four_independent_outputs_and_two_audit_copies() {
    with_chain(
        Module::new("identical-retained-policy6"),
        |input, owner, _| {
            let policy5 = owner.intermediate_policy5();
            let policy4 = policy5.intermediate_policy4();
            let policy3 = policy4.intermediate_policy3();
            let outputs = [
                policy3.owner(),
                policy4.owner(),
                policy5.owner(),
                owner.owner(),
            ];
            for output in outputs {
                assert_eq!(
                    output.canonical().canonical_bytes(),
                    input.canonical().canonical_bytes()
                );
            }
            for (index, first) in outputs.iter().enumerate() {
                for second in outputs.iter().skip(index + 1) {
                    assert!(!std::ptr::eq(
                        first.canonical().canonical_bytes().as_ptr(),
                        second.canonical().canonical_bytes().as_ptr()
                    ));
                }
            }
            let b_history = policy3.native_input_audit_bytes();
            let o_history = owner.continuation().native_input_audit_bytes();
            assert!(!std::ptr::eq(
                b_history.as_ptr(),
                input.canonical().canonical_bytes().as_ptr()
            ));
            assert!(!std::ptr::eq(
                o_history.as_ptr(),
                policy5.owner().canonical().canonical_bytes().as_ptr()
            ));
            assert!(!std::ptr::eq(b_history.as_ptr(), o_history.as_ptr()));
            let mut output_heaps = Counter::new(limits(None, 100_000));
            for output in outputs {
                output.charge_retained_heap_v11(&mut output_heaps).unwrap();
            }
            let report = observe(owner);
            // Constructor checks enforce exact input-history capacity == length.
            // Other native report/map/occurrence/bridge allocations add to this
            // independent lower bound; none of these six payloads may be deduped.
            assert!(
                report.total_bytes
                    >= size_of::<Policy6>()
                        + output_heaps.bytes()
                        + b_history.len()
                        + o_history.len()
            );
            assert_eq!(report, observe(owner));
        },
    );
}
