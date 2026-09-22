use super::*;

impl ProductionOwnedUnitLocalRedundantStoreContinuationV1 {
    pub(crate) fn exercise_old_i_report_refusal_v1(
        &mut self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        assert_ne!(self.kernels(), self.prefix().kernels());
        assert_eq!(self.kernels().len(), self.prefix().kernels().len());
        let floor = budget.storage();
        let rows = std::mem::size_of_val(self.prefix().kernels());
        // Both fresh J and hostile copied I rows coexist in this test only.
        budget.reserve_storage(rows * 2).unwrap();
        let old_i = self.prefix().kernels().to_vec().into_boxed_slice();
        let fresh = std::mem::replace(&mut self.data.kernels, old_i);
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(StoreError::Admission(
                crate::ProductionCheckedOutputAdmissionErrorPolicy3V1::Formal(
                    crate::ProductionFormalMemoryErrorV1::ObligationMismatch
                )
            ))
        ));
        let stale = std::mem::replace(&mut self.data.kernels, fresh);
        drop(stale);
        budget.release_storage(rows * 2).unwrap();
        assert_eq!(budget.storage(), floor);
        self.verify_equivalence(budget).unwrap();
    }
}

#[test]
fn owning_wrapper_headers_cover_only_new_fields_and_branch_padding() {
    let direct = header::<ProductionOwnedRedundantStoreContinuationV1>().unwrap();
    let erased = header::<ProductionOwnedUnitLocalRedundantStoreContinuationV1>().unwrap();
    let fields = std::mem::size_of::<Vec<()>>()
        + std::mem::size_of::<Box<[FormalMemoryObligations]>>()
        + std::mem::size_of::<usize>();
    assert!(direct >= fields);
    assert!(erased >= fields);
    assert_eq!(
        direct + std::mem::size_of::<OwnedRedundantStoreContinuationV1>(),
        std::mem::size_of::<ProductionOwnedRedundantStoreContinuationV1>()
    );
    assert_eq!(
        erased + std::mem::size_of::<OwnedRedundantStoreContinuationV1>(),
        std::mem::size_of::<ProductionOwnedUnitLocalRedundantStoreContinuationV1>()
    );
}

#[test]
fn heap_prefix_headers_are_independent_of_both_historical_source_layouts() {
    use std::mem::{align_of, size_of};
    let fields = [
        (size_of::<Vec<()>>(), align_of::<Vec<()>>()),
        (
            size_of::<OwnedRedundantStoreContinuationV1>(),
            align_of::<OwnedRedundantStoreContinuationV1>(),
        ),
        (
            size_of::<Box<[FormalMemoryObligations]>>(),
            align_of::<Box<[FormalMemoryObligations]>>(),
        ),
        (size_of::<usize>(), align_of::<usize>()),
    ];
    let payload: usize = fields.iter().map(|(size, _)| size).sum();
    let alignment = fields.iter().map(|(_, align)| *align).max().unwrap();
    let conservative_padding_bound = fields.len() * (alignment - 1);
    let direct = size_of::<ProductionOwnedRedundantStoreContinuationV1>();
    let erased = size_of::<ProductionOwnedUnitLocalRedundantStoreContinuationV1>();
    for header in [direct, erased] {
        assert!(header >= payload);
        assert!(header <= payload + conservative_padding_bound);
    }
    assert_eq!(direct, erased);
    assert!(direct < size_of::<ProductionCheckedOutputOwnerPolicy6V1>());
    assert!(erased < size_of::<ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1>());
}

const fn direct_prefix_still_const(
    owner: &ProductionOwnedRedundantStoreContinuationV1,
) -> &ProductionCheckedOutputOwnerPolicy6V1 {
    owner.prefix()
}

const fn erased_prefix_still_const(
    owner: &ProductionOwnedUnitLocalRedundantStoreContinuationV1,
) -> &ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1 {
    owner.prefix()
}

#[test]
fn source_prefix_const_getter_signatures_remain_unchanged() {
    let _: fn(
        &ProductionOwnedRedundantStoreContinuationV1,
    ) -> &ProductionCheckedOutputOwnerPolicy6V1 = direct_prefix_still_const;
    let _: fn(
        &ProductionOwnedUnitLocalRedundantStoreContinuationV1,
    ) -> &ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1 = erased_prefix_still_const;
}

fn independently_occupied_header(total: usize, mut fields: [(usize, usize, bool); 4]) -> usize {
    assert_eq!(fields.iter().filter(|(_, _, paid)| *paid).count(), 1);
    fields.sort_unstable_by_key(|(offset, _, _)| *offset);
    let mut end = 0;
    let mut padding = 0_usize;
    let mut new_fields = 0_usize;
    for (offset, bytes, already_paid_tail) in fields {
        assert!(offset >= end, "physical field intervals must not overlap");
        padding = padding.checked_add(offset - end).unwrap();
        end = offset.checked_add(bytes).unwrap();
        assert!(end <= total);
        if !already_paid_tail {
            new_fields = new_fields.checked_add(bytes).unwrap();
        }
    }
    padding = padding.checked_add(total - end).unwrap();
    new_fields.checked_add(padding).unwrap()
}

macro_rules! independent_header {
    ($owner:ty, $prefix:ty) => {{
        use std::mem::{align_of, offset_of, size_of};
        assert_eq!(size_of::<OwnedPrefix<$prefix>>(), size_of::<Vec<$prefix>>());
        assert_eq!(
            align_of::<OwnedPrefix<$prefix>>(),
            align_of::<Vec<$prefix>>()
        );
        let data = offset_of!($owner, data);
        independently_occupied_header(
            size_of::<$owner>(),
            [
                (offset_of!($owner, prefix), size_of::<Vec<$prefix>>(), false),
                (
                    data + offset_of!(StoreData, continuation),
                    size_of::<OwnedRedundantStoreContinuationV1>(),
                    true,
                ),
                (
                    data + offset_of!(StoreData, kernels),
                    size_of::<Box<[FormalMemoryObligations]>>(),
                    false,
                ),
                (
                    data + offset_of!(StoreData, added),
                    size_of::<usize>(),
                    false,
                ),
            ],
        )
    }};
}

#[test]
fn owning_wrapper_exact_headers_include_every_physical_padding_gap() {
    let direct = independent_header!(
        ProductionOwnedRedundantStoreContinuationV1,
        ProductionCheckedOutputOwnerPolicy6V1
    );
    let erased = independent_header!(
        ProductionOwnedUnitLocalRedundantStoreContinuationV1,
        ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1
    );
    assert_eq!(
        header::<ProductionOwnedRedundantStoreContinuationV1>().unwrap(),
        direct
    );
    assert_eq!(
        header::<ProductionOwnedUnitLocalRedundantStoreContinuationV1>().unwrap(),
        erased
    );
}

macro_rules! independent_receipt_check {
    ($owner:ty, $prefix:ty) => {
        impl $owner {
            pub(crate) fn assert_independent_added_receipt_v1(
                &self,
                receipt: ProductionOwnedRedundantStoreStorageV1,
            ) {
                // Child contract plus independently inspected physical parts.
                // Neither parent header/added_storage nor a claimed D is an oracle.
                let a = self.continuation().retained_storage();
                let k = self
                    .kernels()
                    .len()
                    .checked_mul(std::mem::size_of::<FormalMemoryObligations>())
                    .unwrap();
                let h = independent_header!($owner, $prefix);
                let c = self.prefix.test_capacity_v1();
                assert!(c >= 1);
                let backing = c.checked_mul(std::mem::size_of::<$prefix>()).unwrap();
                let expected = a
                    .checked_add(k)
                    .and_then(|n| n.checked_add(h))
                    .and_then(|n| n.checked_add(backing))
                    .unwrap();
                assert_eq!(receipt.retained_storage(), expected);
                assert_eq!(self.additional_retained_storage_v1(), expected);
                let inherited = self.prefix().retained_input_storage_floor_v1().unwrap();
                assert_eq!(
                    self.retained_input_storage_floor_v1().unwrap(),
                    inherited.checked_add(expected).unwrap()
                );
            }
        }
    };
}
independent_receipt_check!(
    ProductionOwnedRedundantStoreContinuationV1,
    ProductionCheckedOutputOwnerPolicy6V1
);
independent_receipt_check!(
    ProductionOwnedUnitLocalRedundantStoreContinuationV1,
    ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1
);
