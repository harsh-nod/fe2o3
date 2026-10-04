use super::*;
use crate::production::*;
use crate::production_analysis::ProductionAnalysisResourceLimitsV1 as Limits;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn seeded(limits: Limits) -> Resources {
    let mut resources = Resources::new(limits);
    resources
        .admit_retained(PHASE, Bound::checked_phase(PHASE, 7, 11, 5).unwrap())
        .unwrap();
    resources
}

fn nested(resources: &mut Resources) -> Result<[u8; 32], Limit> {
    metered(resources, |outer| {
        outer.update(b"outer")?;
        outer.append_nested(|inner| {
            inner.update(b"inner")?;
            inner.append_nested(|leaf| leaf.update(b"leaf"))
        })?;
        outer.update(b"done")
    })
}

#[test]
fn exact_work_and_peak_include_inherited_and_overlapping_owners() {
    let mut generous = seeded(Limits::production_hard_ceiling());
    let expected = nested(&mut generous).unwrap();
    let used = generous.cumulative();
    assert_eq!(used.retained_storage_upper_bound(), 11 + 32);
    assert_eq!(used.peak_storage_upper_bound(), 11 + 32 + 3 * HASH_STORAGE);
    let mut exact = seeded(Limits::new(
        used.work_upper_bound(),
        used.peak_storage_upper_bound(),
    ));
    assert_eq!(nested(&mut exact).unwrap(), expected);
    assert_eq!(exact.cumulative(), used);
    let mut short = seeded(Limits::new(
        used.work_upper_bound(),
        used.peak_storage_upper_bound() - 1,
    ));
    assert!(nested(&mut short).is_err());
    assert_eq!(short.cumulative().retained_storage_upper_bound(), 11);
    assert!(short.cumulative().work_upper_bound() > 7);
}

#[test]
fn every_work_cut_refuses_without_refunding_accepted_work_or_peak() {
    let mut generous = seeded(Limits::production_hard_ceiling());
    nested(&mut generous).unwrap();
    let used = generous.cumulative();
    for work in 7..used.work_upper_bound() {
        let mut limited = seeded(Limits::new(work, used.peak_storage_upper_bound()));
        assert!(nested(&mut limited).is_err(), "work limit {work}");
        let actual = limited.cumulative();
        assert_eq!(
            actual.retained_storage_upper_bound(),
            11,
            "work limit {work}"
        );
        assert!((7..=work).contains(&actual.work_upper_bound()));
        assert!(actual.peak_storage_upper_bound() >= 16);
    }
}

#[test]
fn repeated_hashes_keep_their_output_reservations_and_original_work() {
    let mut resources = seeded(Limits::production_hard_ceiling());
    let first = nested(&mut resources).unwrap();
    let before = resources.cumulative();
    assert_eq!(nested(&mut resources).unwrap(), first);
    let after = resources.cumulative();
    assert_eq!(after.retained_storage_upper_bound(), 11 + 64);
    assert_eq!(after.work_upper_bound(), 2 * before.work_upper_bound() - 7);
    assert_eq!(
        after.peak_storage_upper_bound(),
        before.peak_storage_upper_bound() + 32
    );
}

#[test]
fn unwind_drops_nested_hashes_before_output_rollback() {
    let mut resources = seeded(Limits::production_hard_ceiling());
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = metered(&mut resources, |outer| {
            outer.update(b"outer")?;
            outer.append_nested(|inner| {
                inner.update(b"inner")?;
                panic!("hash cleanup fixture")
            })
        });
    }));
    assert!(panic.is_err());
    assert_eq!(resources.cumulative().retained_storage_upper_bound(), 11);
    assert!(resources.cumulative().work_upper_bound() > 7);
    assert_eq!(
        resources.cumulative().peak_storage_upper_bound(),
        11 + 32 + 2 * HASH_STORAGE
    );
}

#[test]
fn cleanup_fault_prevents_returning_a_digest() {
    struct BadRelease(bool);
    impl HashMeterV1 for BadRelease {
        type Error = &'static str;
        fn work(&mut self, _: usize) -> Result<(), Self::Error> {
            if self.0 { Err("cleanup") } else { Ok(()) }
        }
        fn reserve(&mut self, _: usize) -> Result<(), Self::Error> {
            Ok(())
        }
        fn release(&mut self, _: usize) {
            self.0 = true;
        }
        fn expression_node(&mut self, _: &mut usize, _: usize) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    assert_eq!(
        hash(&mut BadRelease(false), |digest| digest.update(b"value")),
        Err("cleanup")
    );
}

fn expression_hash(
    expression: &ProductionSemanticExpressionV2,
    resources: &mut Resources,
) -> Result<[u8; 32], Limit> {
    metered(resources, |digest| {
        expression.emit_canonical_transcript_v1(
            digest,
            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        )
    })
}

#[test]
fn expression_depth_is_checked_during_the_paid_walk() {
    use ProductionSemanticExpressionV2 as E;
    let mut expression = E::Constant {
        scalar: ProductionSemanticScalarTypeV2::Bool,
        bits: 1,
    };
    for _ in 1..MAX_DEPTH {
        expression = E::Unary {
            operation: ProductionSemanticUnaryOpV2::Not,
            scalar: ProductionSemanticScalarTypeV2::Bool,
            operand: Box::new(expression),
        };
    }
    let mut resources = seeded(Limits::production_hard_ceiling());
    assert_eq!(
        expression_hash(&expression, &mut resources).unwrap(),
        expression.canonical_transcript_sha256(
            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence
        )
    );
    let deeper = E::Unary {
        operation: ProductionSemanticUnaryOpV2::Not,
        scalar: ProductionSemanticScalarTypeV2::Bool,
        operand: Box::new(expression),
    };
    let mut limited = seeded(Limits::production_hard_ceiling());
    assert_eq!(
        expression_hash(&deeper, &mut limited).unwrap_err().resource,
        "recipe expression nodes or depth"
    );
    assert_eq!(limited.cumulative().retained_storage_upper_bound(), 11);
    assert!(limited.cumulative().work_upper_bound() > MAX_DEPTH);
    let refusal_frames = (MAX_DEPTH + 1) * std::mem::size_of::<(&E, usize, usize, u8)>();
    assert_eq!(
        limited.cumulative().peak_storage_upper_bound(),
        11 + 32 + 2 * HASH_STORAGE + refusal_frames
    );
}

#[test]
fn expression_node_limit_is_not_a_separate_unmetered_preflight() {
    use ProductionSemanticExpressionV2 as E;
    fn tree(depth: usize) -> E {
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        if depth == 0 {
            E::Constant { scalar, bits: 1 }
        } else {
            E::Binary {
                operation: ProductionSemanticBinaryOpV2::Add,
                scalar,
                overflow: ProductionOverflowContractV2::Wrapping,
                lhs: Box::new(tree(depth - 1)),
                rhs: Box::new(tree(depth - 1)),
            }
        }
    }
    let expression = tree(13);
    let mut resources = seeded(Limits::production_hard_ceiling());
    assert_eq!(
        expression_hash(&expression, &mut resources)
            .unwrap_err()
            .resource,
        "recipe expression nodes or depth"
    );
    assert!(resources.cumulative().work_upper_bound() > MAX_NODES);
    assert_eq!(resources.cumulative().retained_storage_upper_bound(), 11);
    let mut limited = seeded(Limits::new(8, usize::MAX));
    assert_ne!(
        expression_hash(&expression, &mut limited)
            .unwrap_err()
            .resource,
        "recipe expression nodes or depth"
    );
    assert!(limited.cumulative().work_upper_bound() <= 8);
}
