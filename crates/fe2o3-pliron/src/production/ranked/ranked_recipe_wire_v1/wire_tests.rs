use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[derive(Default)]
pub(super) struct NoProof {
    finished: usize,
    panic_on_finish: bool,
    swallow_work_denial: bool,
}
impl Resolver for NoProof {
    type Error = &'static str;
    fn resolve(
        &mut self,
        _: ProductionRankedRecipeProofClaimV1,
        _: &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
    ) -> Result<ProductionReferenceProofV2, Self::Error> {
        Err("unexpected bound proof")
    }
    fn finish(
        &mut self,
        count: u32,
        work: &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
    ) -> Result<(), Self::Error> {
        assert_eq!(count, 0);
        if self.swallow_work_denial {
            let _ = work.charge_work(usize::MAX);
        }
        if self.panic_on_finish {
            panic!("resolver unwind fixture");
        }
        self.finished += 1;
        Ok(())
    }
}

pub(super) fn kernel() -> ProductionRankedKernelV1 {
    ProductionRankedKernelV1::new(
        "k",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(0),
                value: 0x0123_4567_89ab_cdef,
            }],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap()
}

pub(super) fn encode(kernel: &ProductionRankedKernelV1) -> Vec<u8> {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(13).unwrap();
    let (bytes, receipt) = encode_production_ranked_recipe_v1(kernel, &mut budget).unwrap();
    assert_eq!(receipt.retained_storage(), bytes.capacity());
    assert_eq!(budget.storage(), 13);
    bytes
}

fn decode(bytes: &[u8]) -> ProductionRankedKernelV1 {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(13).unwrap();
    let mut resolver = NoProof::default();
    let (kernel, receipt) =
        decode_production_ranked_recipe_v1(bytes, &mut resolver, &mut budget).unwrap();
    assert_eq!(budget.storage(), 13);
    assert_eq!(resolver.finished, 1);
    assert!(receipt.retained_storage() >= size_of::<ProductionRankedKernelV1>());
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(encode(&kernel), bytes);
    budget.release_storage(receipt.retained_storage()).unwrap();
    kernel
}

#[test]
fn literal_recipe_round_trip_after_dropping_original_input() {
    const BYTES: [u8; 47] = [
        b'F', b'E', b'2', b'O', b'3', b'R', b'R', 0, 1, 0, 0, 0, 1, 0, 0, 0, b'k', 0, 0, 0, 0, 1,
        0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23,
        0x01, 11,
    ];
    let bytes = {
        let original = kernel();
        encode(&original)
    };
    assert_eq!(bytes, BYTES);
    assert_eq!(decode(&bytes), kernel());
}

#[test]
fn malformed_framing_counts_and_every_truncation_fail_without_storage_leaks() {
    let bytes = encode(&kernel());
    let mut cases = (0..bytes.len())
        .map(|end| bytes[..end].to_vec())
        .collect::<Vec<_>>();
    for (offset, replacement) in [(0, 0), (8, 2), (10, 1), (33, 255), (46, 255)] {
        let mut bad = bytes.clone();
        bad[offset] = replacement;
        cases.push(bad);
    }
    for offset in [12, 17, 21, 29] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        cases.push(bad);
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    cases.push(trailing);
    for bad in cases {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(13).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let mut resolver = NoProof::default();
        assert!(decode_production_ranked_recipe_v1(&bad, &mut resolver, &mut budget).is_err());
        assert_eq!(budget.storage(), 13);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
    }
}

#[test]
fn every_decode_work_and_storage_quota_is_fail_closed() {
    let bytes = encode(&kernel());
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(13).unwrap();
    decode_production_ranked_recipe_v1(&bytes, &mut NoProof::default(), &mut budget).unwrap();
    let full_work = budget.work();
    let peak = budget.peak_storage();
    for quota in 0..=full_work {
        let mut work = Work::new(quota);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(13).unwrap();
        let result =
            decode_production_ranked_recipe_v1(&bytes, &mut NoProof::default(), &mut budget);
        assert_eq!(result.is_ok(), quota == full_work, "work {quota}");
        assert_eq!(budget.storage(), 13);
        if let Err(error) = result {
            assert!(matches!(
                error,
                DecodeError::Wire(WireError::Resource(Resource::Work(_)))
            ));
        }
    }
    for quota in 13..=peak {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, quota);
        budget.reserve_storage(13).unwrap();
        let result =
            decode_production_ranked_recipe_v1(&bytes, &mut NoProof::default(), &mut budget);
        assert_eq!(result.is_ok(), quota == peak, "storage {quota}");
        assert_eq!(budget.storage(), 13);
        if let Err(error) = result {
            assert!(matches!(
                error,
                DecodeError::Wire(WireError::Resource(Resource::Storage(_)))
            ));
        }
    }
}

#[test]
fn resolver_unwind_drops_partial_owner_and_preserves_incoming_floor() {
    let bytes = encode(&kernel());
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(19).unwrap();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let mut resolver = NoProof {
            finished: 0,
            panic_on_finish: true,
            swallow_work_denial: false,
        };
        let _ = decode_production_ranked_recipe_v1(&bytes, &mut resolver, &mut budget);
    }))
    .unwrap_err();
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"resolver unwind fixture")
    );
    assert_eq!(budget.storage(), 19);
    assert!(budget.work() > 0);
}

#[test]
fn swallowed_resolver_denial_fails_even_with_prior_denial_history() {
    let bytes = encode(&kernel());
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(13).unwrap();
    budget.charge_work(1).unwrap();
    assert!(budget.charge_work(usize::MAX).is_err());
    decode_production_ranked_recipe_v1(&bytes, &mut NoProof::default(), &mut budget).unwrap();
    assert_eq!(budget.storage(), 13);
    let mut resolver = NoProof {
        swallow_work_denial: true,
        ..Default::default()
    };
    assert!(matches!(
        decode_production_ranked_recipe_v1(&bytes, &mut resolver, &mut budget),
        Err(DecodeError::Wire(WireError::Resource(Resource::Work(_))))
    ));
    assert_eq!(budget.storage(), 13);
}

#[test]
fn normalization_cannot_silently_accept_noncanonical_recipe_bytes() {
    let view = ProductionRankedOperationV1::View {
        result: ProductionRankedValueIdV1::new(0),
        element_width: 32,
        writable: true,
        shape: vec![16],
        dynamic_extents: vec![],
        allocation_origin: 1,
        noalias_class: 1,
    };
    let mut value = ProductionRankedKernelV1::new(
        "v",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![view.clone()],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    assert!(matches!(
        value.blocks[0].operations[0],
        ProductionRankedOperationV1::ViewInSpace { .. }
    ));
    assert_eq!(decode(&encode(&value)), value);
    value.blocks[0].operations[0] = view;
    let bytes = encode(&value);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let result = decode_production_ranked_recipe_v1(&bytes, &mut NoProof::default(), &mut budget);
    assert!(matches!(
        result,
        Err(DecodeError::Wire(WireError::Invalid(
            "noncanonical normalized recipe"
        )))
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn retained_receipt_counts_actual_normalized_vector_and_string_capacities() {
    let original = kernel();
    let bytes = encode(&original);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let (value, receipt) =
        decode_production_ranked_recipe_v1(&bytes, &mut NoProof::default(), &mut budget).unwrap();
    let exact = size_of::<ProductionRankedKernelV1>()
        + value.function_name.capacity()
        + value.blocks.capacity() * size_of::<ProductionRankedBlockV1>()
        + value.blocks[0].operations.capacity() * size_of::<ProductionRankedOperationV1>();
    assert_eq!(receipt.retained_storage(), exact);
    assert_eq!(budget.storage(), 0);
}
