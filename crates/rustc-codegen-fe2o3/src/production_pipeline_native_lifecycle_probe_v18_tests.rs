//! Copy-only oracle over the very output passed to the actual native consumer.
use fe2o3_kernel_ir::{
    CanonicalKirOperationCoordinateV1 as Coordinate, CastKind, ExecutionOperationV15 as Execution,
    OperationKind as Op, Type,
};
use fe2o3_pliron::CanonicalRankedSourceRequirementV18 as Need;

include!("production_pipeline_native_allocation_probe_v18_tests.rs");

#[derive(Clone, Copy, Debug)]
pub(crate) struct Observation {
    pub(crate) first_unresolved: Option<(Coordinate, Need)>,
    pub(crate) first_unresolved_kind: Option<&'static str>,
    pub(crate) first_unresolved_allocation: Option<AllocationObservation>,
    pub(crate) lifecycle: usize,
    pub(crate) operation_count: usize,
    pub(crate) consumers: usize,
    pub(crate) native_entries: usize,
    pub(crate) private_memory: bool,
}

std::thread_local! {
    static OBSERVED: std::cell::Cell<Option<Observation>> = const { std::cell::Cell::new(None) };
    static RESOURCE_CUT: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

pub(super) fn reset(cut: Option<bool>) {
    OBSERVED.with(|slot| slot.set(None));
    RESOURCE_CUT.with(|slot| slot.set(cut));
}
pub(super) fn completed_policy(private_memory: bool) {
    OBSERVED.with(|slot| {
        let mut row = slot.get().expect("actual output precedes completed source/native policy");
        row.private_memory = private_memory;
        slot.set(Some(row));
    });
}
pub(super) fn native_entry(private_memory: bool) -> Option<bool> {
    OBSERVED.with(|slot| {
        let mut row = slot
            .get()
            .expect("exact output precedes actual native entry");
        row.native_entries += 1;
        assert_eq!(row.private_memory, private_memory);
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

pub(super) fn record(
    original: &super::Original<'_>,
    optimized: &super::Optimized<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    budget: &mut super::Budget<'_>,
) -> Result<(), ProbeError> {
    original.check_query_v18(budget)?;
    let result = with_allocation_probe_scope(budget, inventory.operations().len(), |budget| {
        let source = original.source(budget)?;
        require_exact_probe_owner(source, optimized.original_source(budget)?)?;
        require_exact_probe_owner(
            original.inventory(budget)?,
            optimized.input_inventory(budget)?,
        )?;
        require_exact_probe_owner(inventory, optimized.output_inventory(budget)?)?;
        let mut observation = record_inventory(inventory)?;
        if let Some(allocation) = &mut observation.first_unresolved_allocation {
            let (coordinate, _) = observation.first_unresolved.ok_or(ProbeError::Binding(
                "native allocation diagnostic coordinate",
            ))?;
            allocation.source =
                allocation_source_observation(original, optimized, coordinate, budget)?;
        }
        original.check_query_v18(budget)?;
        Ok(observation)
    });
    let observation = result.map_err(|error| match error {
        ProbeError::Resource(error) => original.retain_query_resource_error_v18(error),
        error => error,
    })?;
    OBSERVED.with(|slot| {
        assert!(
            slot.get().is_none(),
            "same invocation recorded more than one output"
        );
        slot.set(Some(observation));
    });
    Ok(())
}

fn record_inventory(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
) -> Result<Observation, ProbeError> {
    let mut observation = Observation {
        first_unresolved: None,
        first_unresolved_kind: None,
        first_unresolved_allocation: None,
        lifecycle: 0,
        operation_count: inventory.operations().len(),
        consumers: 0,
        native_entries: 0,
        private_memory: false,
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
            observation.first_unresolved_kind = role.map(|_| operation_kind(&row.operation.kind));
            if role.is_some() {
                observation.first_unresolved_allocation = allocation_observation(
                    row.operation,
                    &inventory.owner().module().storage_layouts,
                )?;
            }
        }
    }
    Ok(observation)
}

// Diagnostic names describe the actual candidate; they grant no source role.
fn operation_kind(operation: &Op) -> &'static str {
    use fe2o3_kernel_ir::StorageOperationV1 as Storage;
    match operation {
        Op::Constant(_) => "Constant",
        Op::Unary { .. } => "Unary",
        Op::Binary { .. } => "Binary",
        Op::Compare { .. } => "Compare",
        Op::Cast { .. } => "Cast",
        Op::Select { .. } => "Select",
        Op::Call { .. } => "Call",
        Op::Alloca { .. } => "Alloca",
        Op::SliceLength { .. } => "SliceLength",
        Op::SliceData { .. } => "SliceData",
        Op::GetElementPointer { .. } => "GetElementPointer",
        Op::Load { .. } => "Load",
        Op::Store { .. } => "Store",
        Op::GuardedLoad { .. } => "GuardedLoad",
        Op::GuardedStore { .. } => "GuardedStore",
        Op::Storage(Storage::Project { .. }) => "Storage.Project",
        Op::Storage(Storage::ReadValue { .. }) => "Storage.ReadValue",
        Op::Storage(Storage::WriteValue { .. }) => "Storage.WriteValue",
        Op::Storage(Storage::ReadDiscriminant { .. }) => "Storage.ReadDiscriminant",
        Op::Storage(Storage::SetDiscriminant { .. }) => "Storage.SetDiscriminant",
        Op::Storage(Storage::CopyObject { .. }) => "Storage.CopyObject",
        Op::Execution(Execution::ContextIssue) => "Execution.ContextIssue",
        Op::Execution(Execution::WorkgroupDerive { .. }) => "Execution.WorkgroupDerive",
        Op::Execution(Execution::ScopeEnd { .. }) => "Execution.ScopeEnd",
        Op::Execution(Execution::MaskedTileLoadU32 { .. }) => "Execution.MaskedTileLoadU32",
        Op::Execution(Execution::TileIntoFragmentU32 { .. }) => "Execution.TileIntoFragmentU32",
        Op::Execution(Execution::FragmentIntoPartsU32 { .. }) => "Execution.FragmentIntoPartsU32",
        Op::VerificationContract(_) => "VerificationContract",
        Op::VectorLoad(_) => "VectorLoad",
        Op::VectorStore(_) => "VectorStore",
        Op::VectorLayoutConvert(_) => "VectorLayoutConvert",
        Op::Intrinsic(_) => "Intrinsic",
        Op::MemoryIntrinsic(_) => "MemoryIntrinsic",
        Op::Barrier(_) => "Barrier",
        Op::Atomic(_) => "Atomic",
        Op::Fence(_) => "Fence",
        Op::WorkgroupBarrier(_) => "WorkgroupBarrier",
        Op::WorkgroupMemory(_) => "WorkgroupMemory",
        Op::Matrix(_) => "Matrix",
        Op::Gfx950LdsTranspose(_) => "Gfx950LdsTranspose",
        Op::Wave(_) => "Wave",
        Op::InlineAssembly(_) => "InlineAssembly",
        Op::Gfx942OrderedRegion(_) => "Gfx942OrderedRegion",
        Op::Gfx942OrderedProgram(_) => "Gfx942OrderedProgram",
    }
}

#[test]
fn unresolved_native_operation_names_keep_typed_memory_distinct() {
    use fe2o3_kernel_ir::{
        AddressSpace, MemoryAccess, ScalarType, StorageOperationV1 as Storage, ValueId,
    };
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    let alloca = Op::Alloca {
        element: Type::Scalar(ScalarType::U32),
        count: None,
        address_space: AddressSpace::Private,
        alignment: 4,
    };
    let read = Op::Storage(Storage::ReadValue {
        address: ValueId(7),
        access,
    });
    let write = Op::Storage(Storage::WriteValue {
        address: ValueId(7),
        value: ValueId(8),
        access,
    });
    assert_eq!(operation_kind(&alloca), "Alloca");
    assert_eq!(operation_kind(&read), "Storage.ReadValue");
    assert_eq!(operation_kind(&write), "Storage.WriteValue");
    assert_eq!(
        operation_kind(&Op::Load {
            pointer: ValueId(7),
            access
        }),
        "Load"
    );
    assert_eq!(
        operation_kind(&Op::Execution(Execution::ContextIssue)),
        "Execution.ContextIssue"
    );
}
