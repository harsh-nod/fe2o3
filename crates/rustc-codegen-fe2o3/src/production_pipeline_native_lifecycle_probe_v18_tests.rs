//! Copy-only oracle over the very output passed to the actual native consumer.
use fe2o3_kernel_ir::{
    CanonicalKirOperationCoordinateV1 as Coordinate, CastKind, ExecutionOperationV15 as Execution,
    OperationKind as Op, Type,
};
use fe2o3_pliron::CanonicalRankedSourceRequirementV18 as Need;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Observation {
    pub(crate) first_unresolved: Option<(Coordinate, Need)>,
    pub(crate) lifecycle: usize,
    pub(crate) operation_count: usize,
    pub(crate) consumers: usize,
    pub(crate) native_entries: usize,
}

std::thread_local! {
    static OBSERVED: std::cell::Cell<Option<Observation>> = const { std::cell::Cell::new(None) };
    static RESOURCE_CUT: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

pub(super) fn reset(cut: Option<bool>) {
    OBSERVED.with(|slot| slot.set(None));
    RESOURCE_CUT.with(|slot| slot.set(cut));
}
pub(super) fn native_entry() -> Option<bool> {
    OBSERVED.with(|slot| {
        let mut row = slot
            .get()
            .expect("exact output precedes actual native entry");
        row.native_entries += 1;
        slot.set(Some(row));
    });
    RESOURCE_CUT.with(std::cell::Cell::take)
}
pub(super) fn take() -> Option<Observation> {
    OBSERVED.with(|slot| slot.take())
}
pub(super) fn consumed() {
    OBSERVED.with(|slot| {
        let mut row = slot
            .get()
            .expect("actual output probe precedes native consumer");
        row.consumers += 1;
        slot.set(Some(row));
    });
}

pub(super) fn record(inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>) {
    let mut observation = Observation {
        first_unresolved: None,
        lifecycle: 0,
        operation_count: inventory.operations().len(),
        consumers: 0,
        native_entries: 0,
    };
    for row in inventory.operations() {
        let role = match &row.operation.kind {
            Op::Execution(Execution::ContextIssue | Execution::WorkgroupDerive { .. }) => {
                observation.lifecycle += 1;
                None
            }
            Op::Execution(Execution::ScopeEnd { discarded, .. }) if discarded.is_empty() => {
                observation.lifecycle += 1;
                None
            }
            Op::Constant(_) | Op::Unary { .. } | Op::Binary { .. } | Op::Compare { .. } => None,
            Op::Cast {
                kind:
                    CastKind::RestrictPointerAccess
                    | CastKind::PointerToGeneric
                    | CastKind::SliceToGeneric,
                ..
            } => Some(Need::Memory),
            Op::Cast { .. } => None,
            Op::Select { .. } => match &row.operation.results[0].ty {
                Type::Unit | Type::Scalar(_) => None,
                Type::Pointer(_) | Type::Slice(_) | Type::StorageObject(_) => Some(Need::Memory),
                Type::Vector(_) => Some(Need::Tensor),
                Type::Execution(_) => Some(Need::Execution),
            },
            Op::Call { .. } => Some(Need::Call),
            Op::VerificationContract(_) => Some(Need::Contract),
            Op::Execution(_) | Op::Wave(_) => Some(Need::Execution),
            Op::VectorLoad(_) | Op::VectorStore(_) | Op::VectorLayoutConvert(_) | Op::Matrix(_) => {
                Some(Need::Tensor)
            }
            Op::InlineAssembly(_) | Op::Gfx942OrderedRegion(_) | Op::Gfx942OrderedProgram(_) => {
                Some(Need::Assembly)
            }
            Op::Intrinsic(_) => Some(Need::Intrinsic),
            Op::Storage(_)
            | Op::MemoryIntrinsic(_)
            | Op::Alloca { .. }
            | Op::SliceLength { .. }
            | Op::SliceData { .. }
            | Op::GetElementPointer { .. }
            | Op::Load { .. }
            | Op::GuardedLoad { .. }
            | Op::GuardedStore { .. }
            | Op::Store { .. }
            | Op::Barrier(_)
            | Op::Atomic(_)
            | Op::Fence(_)
            | Op::WorkgroupBarrier(_)
            | Op::WorkgroupMemory(_)
            | Op::Gfx950LdsTranspose(_) => Some(Need::Memory),
        };
        if observation.first_unresolved.is_none() {
            observation.first_unresolved = role.map(|role| (row.coordinate, role));
        }
    }
    OBSERVED.with(|slot| {
        assert!(
            slot.get().is_none(),
            "same invocation recorded more than one output"
        );
        slot.set(Some(observation));
    });
}
