//! Test-only genuine source consumer. No output, proof or executable authority.
use super::*;
use fe2o3_kernel_ir::{
    AddressSpace, AtomicKind, CastKind, MemoryOrdering, OperationKind, ScalarType,
    SynchronizationScope, Type,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAtomicOrderingV1, SemanticAtomicRmwOpV1, SemanticAtomicScopeV1,
    SemanticMirWireVersionV1, SemanticScalarTypeV1, SemanticStatementKindV1, SemanticTypeShapeV1,
};

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub(crate) struct AtomicRootObservationV41 {
    pub source: [u8; 32],
    pub canonical: [u8; 32],
    pub roots: usize,
    pub instances: usize,
    pub active_instances: usize,
    pub anchors: usize,
    pub pointer_to_generic: usize,
    pub original: [[usize; 5]; 10],
    pub emitted: [[usize; 5]; 10],
}

// Selecting this policy does not bypass preparation. The unchanged parent
// invokes original ABI/context/reference obligations, pending ExpandedRaw
// graph/lifetime/relocation checks and finish_source_claims before consume.
struct AtomicWholeRootV41;
impl<R, F> SourceHandoffPolicyV29<R, F> for AtomicWholeRootV41
where
    F: for<'view, 'source, 'work> FnOnce(
        &'view Source<'source>,
        &mut Budget<'work>,
    ) -> Result<R, Error>,
{
    fn entry_headers() -> Result<usize, Resource> {
        entry_headers_for_handoff::<R, F, ()>()
    }
    fn consume<'view, 'source, 'abi, 'work>(
        source: &'view Source<'source>,
        _roots: &[AbiRoot<'abi>],
        _context: &SourceBindingContextV29<'_>,
        budget: &mut Budget<'work>,
        consume: F,
    ) -> Result<R, Error> {
        source.check_original_source(source.source_ssa(budget)?, budget)?;
        consume(source, budget)
    }
}

fn add_v41(value: &mut usize) -> Result<(), Error> {
    *value = value.checked_add(1).ok_or(Resource::Arithmetic)?;
    Ok(())
}
fn source_kind_v41(kind: SemanticAtomicRmwOpV1) -> Option<(usize, bool)> {
    Some(match kind {
        SemanticAtomicRmwOpV1::Exchange => (0, false),
        SemanticAtomicRmwOpV1::Add => (1, false),
        SemanticAtomicRmwOpV1::Subtract => (2, false),
        SemanticAtomicRmwOpV1::BitAnd => (3, false),
        SemanticAtomicRmwOpV1::BitOr => (4, false),
        SemanticAtomicRmwOpV1::BitXor => (5, false),
        SemanticAtomicRmwOpV1::UnsignedMinimum => (6, false),
        SemanticAtomicRmwOpV1::UnsignedMaximum => (7, false),
        SemanticAtomicRmwOpV1::SignedMinimum => (8, true),
        SemanticAtomicRmwOpV1::SignedMaximum => (9, true),
        SemanticAtomicRmwOpV1::BitNand => return None,
    })
}
fn physical_kind_v41(kind: AtomicKind, scalar: ScalarType) -> Option<usize> {
    match (kind, scalar) {
        (AtomicKind::Exchange, ScalarType::U32) => Some(0),
        (AtomicKind::Add, ScalarType::U32) => Some(1),
        (AtomicKind::Subtract, ScalarType::U32) => Some(2),
        (AtomicKind::BitAnd, ScalarType::U32) => Some(3),
        (AtomicKind::BitOr, ScalarType::U32) => Some(4),
        (AtomicKind::BitXor, ScalarType::U32) => Some(5),
        (AtomicKind::Min, ScalarType::U32) => Some(6),
        (AtomicKind::Max, ScalarType::U32) => Some(7),
        (AtomicKind::Min, ScalarType::I32) => Some(8),
        (AtomicKind::Max, ScalarType::I32) => Some(9),
        _ => None,
    }
}
fn source_order_v41(order: SemanticAtomicOrderingV1) -> usize {
    match order {
        SemanticAtomicOrderingV1::Relaxed => 0,
        SemanticAtomicOrderingV1::Release => 1,
        SemanticAtomicOrderingV1::Acquire => 2,
        SemanticAtomicOrderingV1::AcquireRelease => 3,
        SemanticAtomicOrderingV1::SequentiallyConsistent => 4,
    }
}
fn physical_order_v41(order: MemoryOrdering) -> usize {
    match order {
        MemoryOrdering::Relaxed => 0,
        MemoryOrdering::Release => 1,
        MemoryOrdering::Acquire => 2,
        MemoryOrdering::AcquireRelease => 3,
        MemoryOrdering::SequentiallyConsistent => 4,
    }
}

fn observe_v41(
    source: &Source<'_>,
    budget: &mut Budget<'_>,
) -> Result<AtomicRootObservationV41, Error> {
    // Fixed report, return/result envelopes and counter scratch are paid before
    // construction. No detached source/graph clone or variable report buffer.
    let bytes = size_of::<AtomicRootObservationV41>()
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of::<Result<AtomicRootObservationV41, Error>>()))
        .and_then(|n| n.checked_add(16 * size_of::<usize>()))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(bytes)?;
    budget.reserve_storage(bytes)?;
    let semantic = source.source_semantic(budget)?;
    if semantic.wire_version() != SemanticMirWireVersionV1::V41 {
        return Err(Error::Unsupported(
            "atomic whole-root observation requires original V41",
        ));
    }
    let canonical = source.canonical(budget)?;
    let mut report = AtomicRootObservationV41 {
        source: *semantic.semantic_sha256().as_bytes(),
        canonical: *canonical.identity().digest(),
        roots: source.root_count(budget)?,
        instances: 0,
        active_instances: 0,
        anchors: 0,
        pointer_to_generic: 0,
        original: [[0; 5]; 10],
        emitted: [[0; 5]; 10],
    };
    for function in semantic.functions() {
        budget.charge_work(1)?;
        for block in function.blocks() {
            budget.charge_work(1)?;
            for statement in block.statements() {
                budget.charge_work(1)?;
                let SemanticStatementKindV1::AtomicRmw(atomic) = statement.kind() else {
                    continue;
                };
                let (kind, signed) = source_kind_v41(atomic.operation())
                    .ok_or(Error::Unsupported("atomic fixture source kind"))?;
                let element = semantic
                    .types()
                    .get(atomic.value().ty().index() as usize)
                    .ok_or(Error::Unsupported("atomic fixture source scalar"))?;
                if atomic.access().scope() != SemanticAtomicScopeV1::System
                    || atomic.address().ty() != atomic.value().ty()
                    || atomic.destination().ty() != atomic.value().ty()
                    || element.shape()
                        != &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            signed,
                            bits: 32,
                        })
                {
                    return Err(Error::Unsupported("atomic fixture source contract"));
                }
                add_v41(&mut report.original[kind][source_order_v41(atomic.access().ordering())])?;
            }
        }
    }
    for root in 0..report.roots {
        let count = source.instance_count(root, budget)?;
        report.instances = report
            .instances
            .checked_add(count)
            .ok_or(Resource::Arithmetic)?;
        for instance in 0..count {
            budget.charge_work(1)?;
            if source.instance_active(root, instance, budget)? {
                add_v41(&mut report.active_instances)?;
            }
            report.anchors = report
                .anchors
                .checked_add(source.memory_anchor_count(root, instance, budget)?)
                .ok_or(Resource::Arithmetic)?;
        }
    }
    for function in &canonical.module().functions {
        budget.charge_work(1)?;
        let Some(body) = &function.body else { continue };
        for block in &body.blocks {
            budget.charge_work(1)?;
            for operation in &block.operations {
                budget.charge_work(1)?;
                if matches!(
                    &operation.kind,
                    OperationKind::Cast {
                        kind: CastKind::PointerToGeneric,
                        ..
                    }
                ) {
                    add_v41(&mut report.pointer_to_generic)?;
                }
                let OperationKind::Atomic(atomic) = &operation.kind else {
                    continue;
                };
                let [result] = operation.results.as_slice() else {
                    return Err(Error::Unsupported("atomic fixture old-result arity"));
                };
                let Type::Scalar(scalar) = &result.ty else {
                    return Err(Error::Unsupported("atomic fixture old-result scalar"));
                };
                let kind = physical_kind_v41(atomic.kind, *scalar)
                    .ok_or(Error::Unsupported("atomic fixture physical kind"))?;
                let order = physical_order_v41(atomic.ordering);
                if atomic.scope != SynchronizationScope::System
                    || atomic.access.address_space != AddressSpace::Generic
                    || atomic.access.alignment != 4
                    || atomic.access.volatile
                    || atomic.value.is_none()
                    || atomic.compare.is_some()
                    || atomic.failure_ordering.is_some()
                    || report.original[kind][order] == 0
                {
                    return Err(Error::Unsupported("atomic fixture physical contract"));
                }
                add_v41(&mut report.emitted[kind][order])?;
            }
        }
    }
    // Original control may remove inactive order arms or duplicate instances.
    // Census both sides; never assert fifty physical operations or substitute
    // equal counts for the mandatory original rooted address/relocation proof.
    if report
        .original
        .iter()
        .any(|row| row.iter().all(|n| *n == 0))
        || report.emitted.iter().any(|row| row.iter().all(|n| *n == 0))
        || report.pointer_to_generic == 0
        || report.anchors == 0
    {
        return Err(Error::Unsupported(
            "atomic fixture incomplete whole-root observation",
        ));
    }
    budget.check_prior_denials_v1()?;
    Ok(report)
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn source_owned_atomic_root_for_test_v41(
        self,
    ) -> Result<AtomicRootObservationV41, Error> {
        self.with_source_owned_custody_policy_v29::<AtomicWholeRootV41, _, _>(
            ImportProfile::AtomicV41,
            WORK_LIMIT,
            STORAGE_LIMIT,
            observe_v41,
        )
        .map(SourceOwnedCompilationContinuationV29::into_observation)
    }
}

#[test]
fn atomic_v41_whole_root_oracle_distinguishes_signed_minimum_and_closed_kinds() {
    assert_eq!(physical_kind_v41(AtomicKind::Min, ScalarType::I32), Some(8));
    assert_eq!(physical_kind_v41(AtomicKind::Min, ScalarType::U32), Some(6));
    assert_eq!(physical_kind_v41(AtomicKind::Max, ScalarType::I32), Some(9));
    for kind in [
        AtomicKind::Load,
        AtomicKind::Store,
        AtomicKind::CompareExchange,
    ] {
        assert_eq!(physical_kind_v41(kind, ScalarType::U32), None);
    }
    assert_eq!(physical_kind_v41(AtomicKind::Add, ScalarType::I32), None);
    assert_eq!(physical_kind_v41(AtomicKind::Min, ScalarType::U64), None);
    assert_eq!(source_kind_v41(SemanticAtomicRmwOpV1::BitNand), None);
}
#[test]
fn atomic_v41_whole_root_oracle_preserves_all_five_orderings() {
    for (source, physical, expected) in [
        (
            SemanticAtomicOrderingV1::Relaxed,
            MemoryOrdering::Relaxed,
            0,
        ),
        (
            SemanticAtomicOrderingV1::Release,
            MemoryOrdering::Release,
            1,
        ),
        (
            SemanticAtomicOrderingV1::Acquire,
            MemoryOrdering::Acquire,
            2,
        ),
        (
            SemanticAtomicOrderingV1::AcquireRelease,
            MemoryOrdering::AcquireRelease,
            3,
        ),
        (
            SemanticAtomicOrderingV1::SequentiallyConsistent,
            MemoryOrdering::SequentiallyConsistent,
            4,
        ),
    ] {
        assert_eq!(source_order_v41(source), expected);
        assert_eq!(physical_order_v41(physical), expected);
    }
}
#[test]
fn atomic_v41_whole_root_report_counter_cannot_wrap() {
    let mut count = usize::MAX;
    assert!(matches!(
        add_v41(&mut count),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert_eq!(count, usize::MAX);
}
