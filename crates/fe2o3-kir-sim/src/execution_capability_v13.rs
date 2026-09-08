use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Atomic, AtomicKind, Axis, BarrierSemantics, BinaryOp, BlockId,
    ComparePredicate, Constant, Convergence, ExecutionAtomicKindV1, ExecutionCapabilityOpV1,
    ExecutionCapabilityOperationV1, ExecutionCapabilitySourceV1, ExecutionCollectiveKindV1,
    ExecutionDynamicExtentV1, ExecutionElementLayoutV1, ExecutionMemoryAccessV1,
    ExecutionMemoryAddressSpaceV1, ExecutionMemoryExtentV1, ExecutionMemorySemanticsV1, Fence,
    FunctionId, IndexKind, IntrinsicKind, IntrinsicOperation, MemoryAccess, Module, Operation,
    OperationKind, ScalarType, SynchronizationScope, Type, ValueDef, ValueId, WaveOperation,
    WaveOperationKind, WaveWidth, WorkgroupMemory, WorkgroupMemoryExtent,
};

use crate::IncompleteExecutionCapabilityOperationV13;

/// Closed V13 execution-capability family recorded before logical erasure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SimulationExecutionCapabilityFamilyV13 {
    WorkgroupDerive,
    SubgroupDerive,
    LdsAllocate,
    LdsInitializeByInvocation,
    LdsPublish,
    LdsReadPublished,
    WorkgroupBarrier,
    SubgroupBarrier,
    WorkgroupFence,
    SubgroupFence,
    Atomic,
    WorkgroupCollective,
    SubgroupCollective,
    MatrixAccess,
    AsyncCopy,
    AsyncWait,
    RawMemoryBind,
    PrivateMemoryAllocate,
    WorkgroupMemoryIndex,
    WorkgroupMemoryAllocate,
    WorkgroupMemoryPublish,
    MemoryLoad,
    MemoryStore,
}

impl SimulationExecutionCapabilityFamilyV13 {
    pub const fn of(operation: &ExecutionCapabilityOperationV1) -> Self {
        use ExecutionCapabilityOperationV1 as Operation;
        match operation {
            Operation::WorkgroupDerive { .. } => Self::WorkgroupDerive,
            Operation::SubgroupDerive { .. } => Self::SubgroupDerive,
            Operation::LdsAllocate { .. } => Self::LdsAllocate,
            Operation::LdsInitializeByInvocation { .. } => Self::LdsInitializeByInvocation,
            Operation::LdsPublish { .. } => Self::LdsPublish,
            Operation::LdsReadPublished { .. } => Self::LdsReadPublished,
            Operation::WorkgroupBarrier { .. } => Self::WorkgroupBarrier,
            Operation::SubgroupBarrier { .. } => Self::SubgroupBarrier,
            Operation::WorkgroupFence { .. } => Self::WorkgroupFence,
            Operation::SubgroupFence { .. } => Self::SubgroupFence,
            Operation::Atomic { .. } => Self::Atomic,
            Operation::WorkgroupCollective { .. } => Self::WorkgroupCollective,
            Operation::SubgroupCollective { .. } => Self::SubgroupCollective,
            Operation::MatrixAccess { .. } => Self::MatrixAccess,
            Operation::AsyncCopy { .. } => Self::AsyncCopy,
            Operation::AsyncWait { .. } => Self::AsyncWait,
            Operation::RawMemoryBind { .. } => Self::RawMemoryBind,
            Operation::PrivateMemoryAllocate { .. } => Self::PrivateMemoryAllocate,
            Operation::WorkgroupMemoryIndex { .. } => Self::WorkgroupMemoryIndex,
            Operation::WorkgroupMemoryAllocate { .. } => Self::WorkgroupMemoryAllocate,
            Operation::WorkgroupMemoryPublish { .. } => Self::WorkgroupMemoryPublish,
            Operation::MemoryLoad { .. } => Self::MemoryLoad,
            Operation::MemoryStore { .. } => Self::MemoryStore,
        }
    }
}

/// Logical type whose definition or use was checked before V13 projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SimulationLogicalCapabilityKindV13 {
    KernelContext,
    GlobalMemory,
    Execution,
}

/// Exact canonical-KIR position checked before a logical capability is erased.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SimulationCapabilityCoordinateKindV13 {
    FunctionParameterDefinition { parameter: u16 },
    BlockParameterDefinition { parameter: u16 },
    OperationResultDefinition { result: u16 },
    OperationOperandUse { operand: u16 },
    TerminatorOperandUse { operand: u16 },
}

/// One deterministic definition/use record retained by V13 simulator admission.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationCapabilityCoordinateV13 {
    function: u32,
    block: Option<BlockId>,
    operation: Option<u32>,
    value: ValueId,
    logical_kind: SimulationLogicalCapabilityKindV13,
    coordinate_kind: SimulationCapabilityCoordinateKindV13,
    execution_family: Option<SimulationExecutionCapabilityFamilyV13>,
    source: Option<ExecutionCapabilitySourceV1>,
}

impl SimulationCapabilityCoordinateV13 {
    pub const fn function(&self) -> u32 {
        self.function
    }

    pub const fn block(&self) -> Option<BlockId> {
        self.block
    }

    pub const fn operation(&self) -> Option<u32> {
        self.operation
    }

    pub const fn value(&self) -> ValueId {
        self.value
    }

    pub const fn logical_kind(&self) -> SimulationLogicalCapabilityKindV13 {
        self.logical_kind
    }

    pub const fn coordinate_kind(&self) -> SimulationCapabilityCoordinateKindV13 {
        self.coordinate_kind
    }

    pub const fn execution_family(&self) -> Option<SimulationExecutionCapabilityFamilyV13> {
        self.execution_family
    }

    pub const fn source(&self) -> Option<ExecutionCapabilitySourceV1> {
        self.source
    }
}

/// Authority-free audit receipt for the exact V13 graph consumed by simulation.
#[derive(Debug, Eq, PartialEq)]
pub struct SimulationCapabilityProjectionReceiptV13 {
    coordinates: Vec<SimulationCapabilityCoordinateV13>,
}

impl SimulationCapabilityProjectionReceiptV13 {
    pub fn coordinates(&self) -> &[SimulationCapabilityCoordinateV13] {
        &self.coordinates
    }

    pub fn definitions(&self) -> usize {
        self.coordinates
            .iter()
            .filter(|coordinate| {
                !matches!(
                    coordinate.coordinate_kind,
                    SimulationCapabilityCoordinateKindV13::OperationOperandUse { .. }
                        | SimulationCapabilityCoordinateKindV13::TerminatorOperandUse { .. }
                )
            })
            .count()
    }

    pub fn uses(&self) -> usize {
        self.coordinates.len() - self.definitions()
    }

    pub(crate) fn retained_heap_bytes(&self) -> Option<usize> {
        self.coordinates
            .capacity()
            .checked_mul(size_of::<SimulationCapabilityCoordinateV13>())
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ExecutionCapabilityProjectionV13 {
    pub(crate) aliases: BTreeMap<FunctionId, Vec<(ValueId, ValueId)>>,
    pub(crate) scratch_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MemoryExtentProjectionV13 {
    Static(u64),
    Dynamic(ValueId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingAsyncCopyProjectionV13 {
    destination: ValueId,
    scalar: ScalarType,
    layout: ExecutionElementLayoutV1,
    elements: u64,
}

impl From<u64> for MemoryExtentProjectionV13 {
    fn from(value: u64) -> Self {
        Self::Static(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecutionCapabilityProjectionErrorV13 {
    AllocationFailure,
    Incomplete(IncompleteExecutionCapabilityOperationV13),
    Invalid(&'static str),
}

pub(crate) fn record_projection_coordinates_v13(
    module: &Module,
) -> Result<(SimulationCapabilityProjectionReceiptV13, usize), ExecutionCapabilityProjectionErrorV13>
{
    let mut coordinates = Vec::new();
    let mut maximum_scratch_bytes = 0;
    for (function_ordinal, function) in module.functions.iter().enumerate() {
        let function_ordinal = u32::try_from(function_ordinal).map_err(|_| {
            ExecutionCapabilityProjectionErrorV13::Invalid(
                "function ordinal overflow while recording capability projection",
            )
        })?;
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        let logical_count = body
            .parameters
            .iter()
            .zip(&function.signature.parameters)
            .filter(|(_, ty)| logical_kind(ty).is_some())
            .count()
            + body
                .blocks
                .iter()
                .flat_map(|block| &block.parameters)
                .filter(|parameter| logical_kind(&parameter.ty).is_some())
                .count()
            + body
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .flat_map(|operation| &operation.results)
                .filter(|result| logical_kind(&result.ty).is_some())
                .count();
        let mut logical_values = Vec::new();
        logical_values
            .try_reserve_exact(logical_count)
            .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;

        for (parameter, (value, ty)) in body
            .parameters
            .iter()
            .zip(&function.signature.parameters)
            .enumerate()
        {
            let Some(kind) = logical_kind(ty) else {
                continue;
            };
            let parameter = u16::try_from(parameter).map_err(|_| {
                ExecutionCapabilityProjectionErrorV13::Invalid(
                    "function parameter ordinal overflow while recording capability projection",
                )
            })?;
            logical_values.push((*value, kind));
            push_coordinate(
                &mut coordinates,
                SimulationCapabilityCoordinateV13 {
                    function: function_ordinal,
                    block: None,
                    operation: None,
                    value: *value,
                    logical_kind: kind,
                    coordinate_kind:
                        SimulationCapabilityCoordinateKindV13::FunctionParameterDefinition {
                            parameter,
                        },
                    execution_family: None,
                    source: None,
                },
            )?;
        }

        for block in &body.blocks {
            for (parameter, definition) in block.parameters.iter().enumerate() {
                let Some(kind) = logical_kind(&definition.ty) else {
                    continue;
                };
                let parameter = u16::try_from(parameter).map_err(|_| {
                    ExecutionCapabilityProjectionErrorV13::Invalid(
                        "block parameter ordinal overflow while recording capability projection",
                    )
                })?;
                logical_values.push((definition.id, kind));
                push_coordinate(
                    &mut coordinates,
                    SimulationCapabilityCoordinateV13 {
                        function: function_ordinal,
                        block: Some(block.id),
                        operation: None,
                        value: definition.id,
                        logical_kind: kind,
                        coordinate_kind:
                            SimulationCapabilityCoordinateKindV13::BlockParameterDefinition {
                                parameter,
                            },
                        execution_family: None,
                        source: None,
                    },
                )?;
            }
            for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                let operation_ordinal = u32::try_from(operation_ordinal).map_err(|_| {
                    ExecutionCapabilityProjectionErrorV13::Invalid(
                        "operation ordinal overflow while recording capability projection",
                    )
                })?;
                let (family, source) = operation_execution_coordinate(&operation.kind);
                for (result, definition) in operation.results.iter().enumerate() {
                    let Some(kind) = logical_kind(&definition.ty) else {
                        continue;
                    };
                    let result = u16::try_from(result).map_err(|_| {
                        ExecutionCapabilityProjectionErrorV13::Invalid(
                            "result ordinal overflow while recording capability projection",
                        )
                    })?;
                    logical_values.push((definition.id, kind));
                    push_coordinate(
                        &mut coordinates,
                        SimulationCapabilityCoordinateV13 {
                            function: function_ordinal,
                            block: Some(block.id),
                            operation: Some(operation_ordinal),
                            value: definition.id,
                            logical_kind: kind,
                            coordinate_kind:
                                SimulationCapabilityCoordinateKindV13::OperationResultDefinition {
                                    result,
                                },
                            execution_family: family,
                            source,
                        },
                    )?;
                }
            }
        }
        logical_values.sort_unstable_by_key(|(value, _)| *value);
        if logical_values.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(ExecutionCapabilityProjectionErrorV13::Invalid(
                "logical SSA value is defined more than once",
            ));
        }

        for block in &body.blocks {
            for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                let operation_ordinal = u32::try_from(operation_ordinal).map_err(|_| {
                    ExecutionCapabilityProjectionErrorV13::Invalid(
                        "operation ordinal overflow while recording capability projection",
                    )
                })?;
                let (family, source) = operation_execution_coordinate(&operation.kind);
                let operands = operation.operands();
                maximum_scratch_bytes = maximum_scratch_bytes.max(
                    operands
                        .capacity()
                        .checked_mul(size_of::<ValueId>())
                        .ok_or(ExecutionCapabilityProjectionErrorV13::AllocationFailure)?,
                );
                for (operand, value) in operands.into_iter().enumerate() {
                    let Ok(index) =
                        logical_values.binary_search_by_key(&value, |(value, _)| *value)
                    else {
                        continue;
                    };
                    let operand = u16::try_from(operand).map_err(|_| {
                        ExecutionCapabilityProjectionErrorV13::Invalid(
                            "operand ordinal overflow while recording capability projection",
                        )
                    })?;
                    push_coordinate(
                        &mut coordinates,
                        SimulationCapabilityCoordinateV13 {
                            function: function_ordinal,
                            block: Some(block.id),
                            operation: Some(operation_ordinal),
                            value,
                            logical_kind: logical_values[index].1,
                            coordinate_kind:
                                SimulationCapabilityCoordinateKindV13::OperationOperandUse {
                                    operand,
                                },
                            execution_family: family,
                            source,
                        },
                    )?;
                }
            }
            if let Some(terminator) = &block.terminator {
                let operands = terminator.operands();
                maximum_scratch_bytes = maximum_scratch_bytes.max(
                    operands
                        .capacity()
                        .checked_mul(size_of::<ValueId>())
                        .ok_or(ExecutionCapabilityProjectionErrorV13::AllocationFailure)?,
                );
                for (operand, value) in operands.into_iter().enumerate() {
                    let Ok(index) =
                        logical_values.binary_search_by_key(&value, |(value, _)| *value)
                    else {
                        continue;
                    };
                    let operand = u16::try_from(operand).map_err(|_| {
                        ExecutionCapabilityProjectionErrorV13::Invalid(
                            "terminator operand ordinal overflow while recording capability projection",
                        )
                    })?;
                    push_coordinate(
                        &mut coordinates,
                        SimulationCapabilityCoordinateV13 {
                            function: function_ordinal,
                            block: Some(block.id),
                            operation: None,
                            value,
                            logical_kind: logical_values[index].1,
                            coordinate_kind:
                                SimulationCapabilityCoordinateKindV13::TerminatorOperandUse {
                                    operand,
                                },
                            execution_family: None,
                            source: None,
                        },
                    )?;
                }
            }
        }
        maximum_scratch_bytes = maximum_scratch_bytes.max(
            logical_values
                .capacity()
                .checked_mul(size_of::<(ValueId, SimulationLogicalCapabilityKindV13)>())
                .ok_or(ExecutionCapabilityProjectionErrorV13::AllocationFailure)?,
        );
    }
    Ok((
        SimulationCapabilityProjectionReceiptV13 { coordinates },
        maximum_scratch_bytes,
    ))
}

fn push_coordinate(
    coordinates: &mut Vec<SimulationCapabilityCoordinateV13>,
    coordinate: SimulationCapabilityCoordinateV13,
) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
    coordinates
        .try_reserve(1)
        .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
    coordinates.push(coordinate);
    Ok(())
}

const fn logical_kind(ty: &Type) -> Option<SimulationLogicalCapabilityKindV13> {
    match ty {
        Type::KernelContext(_) => Some(SimulationLogicalCapabilityKindV13::KernelContext),
        Type::GlobalCapability(_) => Some(SimulationLogicalCapabilityKindV13::GlobalMemory),
        Type::ExecutionCapability(_) => Some(SimulationLogicalCapabilityKindV13::Execution),
        Type::Unit | Type::Scalar(_) | Type::Pointer(_) | Type::Slice(_) => None,
    }
}

fn operation_execution_coordinate(
    operation: &OperationKind,
) -> (
    Option<SimulationExecutionCapabilityFamilyV13>,
    Option<ExecutionCapabilitySourceV1>,
) {
    match operation {
        OperationKind::ExecutionCapability(contract) => (
            Some(SimulationExecutionCapabilityFamilyV13::of(
                &contract.operation,
            )),
            Some(contract.source),
        ),
        _ => (None, None),
    }
}

pub(crate) fn project_execution_capabilities_v13(
    module: &mut Module,
) -> Result<ExecutionCapabilityProjectionV13, ExecutionCapabilityProjectionErrorV13> {
    let mut report = ExecutionCapabilityProjectionV13::default();
    let kernel_workgroups = kernel_workgroups(module)?;
    let mut module_requirements = BTreeSet::new();

    for function in &mut module.functions {
        let signature = function.signature.clone();
        let function_id = function.id.clone();
        let Some(body) = function.body.as_mut() else {
            continue;
        };
        let types = value_types(&signature, body)?;
        let scalar_identities = scalar_identities(body, &types)?;
        let mut aliases = global_aliases(body)?;
        let mut promoted = BTreeMap::new();
        let mut dynamic_extents = BTreeMap::new();
        let mut pending_async_copies = BTreeMap::new();
        let mut next_value = next_value_id(body)?;
        let workgroup = kernel_workgroups.get(&function_id).copied().flatten();

        for block in &mut body.blocks {
            let source = std::mem::take(&mut block.operations);
            let mut projected = Vec::new();
            projected
                .try_reserve(source.len())
                .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
            for operation in source {
                let OperationKind::ExecutionCapability(contract) = operation.kind else {
                    projected.push(operation);
                    continue;
                };
                let mut expansion = project_operation(
                    operation.results,
                    contract,
                    &types,
                    &scalar_identities,
                    &mut aliases,
                    &mut promoted,
                    &mut dynamic_extents,
                    &mut pending_async_copies,
                    &mut next_value,
                    workgroup,
                )?;
                projected
                    .try_reserve(expansion.len())
                    .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
                projected.append(&mut expansion);
            }
            block.operations = projected;
        }
        if !pending_async_copies.is_empty() {
            return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
                IncompleteExecutionCapabilityOperationV13::AsyncWait,
            ));
        }

        let generated = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .flat_map(Operation::required_capabilities)
            .collect::<BTreeSet<_>>();
        function.required_capabilities.extend(generated.clone());
        module_requirements.extend(generated);
        aliases.sort_unstable_by_key(|(source, _)| *source);
        if aliases.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(ExecutionCapabilityProjectionErrorV13::Invalid(
                "execution capability projection defined one alias more than once",
            ));
        }
        report.scratch_bytes = report
            .scratch_bytes
            .checked_add(
                aliases
                    .capacity()
                    .checked_mul(size_of::<(ValueId, ValueId)>())
                    .ok_or(ExecutionCapabilityProjectionErrorV13::AllocationFailure)?,
            )
            .ok_or(ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
        let capability_aliases = aliases
            .iter()
            .filter(|(source, _)| matches!(types.get(source), Some(Type::ExecutionCapability(_))))
            .copied()
            .collect();
        report.aliases.insert(function_id, capability_aliases);
    }
    module.required_capabilities.extend(module_requirements);
    Ok(report)
}

fn kernel_workgroups(
    module: &Module,
) -> Result<
    BTreeMap<FunctionId, Option<fe2o3_kernel_ir::WorkgroupSize>>,
    ExecutionCapabilityProjectionErrorV13,
> {
    let mut workgroups = BTreeMap::new();
    for kernel in &module.kernels {
        match workgroups.get(&kernel.entry) {
            Some(existing) if *existing != kernel.workgroup_size => {
                return Err(ExecutionCapabilityProjectionErrorV13::Invalid(
                    "one capability root is exported with different workgroup geometries",
                ));
            }
            _ => {
                workgroups.insert(kernel.entry.clone(), kernel.workgroup_size);
            }
        }
    }
    Ok(workgroups)
}

fn value_types(
    signature: &fe2o3_kernel_ir::Signature,
    body: &fe2o3_kernel_ir::FunctionBody,
) -> Result<BTreeMap<ValueId, Type>, ExecutionCapabilityProjectionErrorV13> {
    let mut types = BTreeMap::new();
    for (value, ty) in body.parameters.iter().zip(&signature.parameters) {
        insert_type(&mut types, *value, ty.clone())?;
    }
    for block in &body.blocks {
        for parameter in &block.parameters {
            insert_type(&mut types, parameter.id, parameter.ty.clone())?;
        }
        for result in block
            .operations
            .iter()
            .flat_map(|operation| &operation.results)
        {
            insert_type(&mut types, result.id, result.ty.clone())?;
        }
    }
    Ok(types)
}

fn insert_type(
    types: &mut BTreeMap<ValueId, Type>,
    value: ValueId,
    ty: Type,
) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
    if types.insert(value, ty).is_some() {
        return Err(ExecutionCapabilityProjectionErrorV13::Invalid(
            "duplicate SSA value reached capability projection",
        ));
    }
    Ok(())
}

fn next_value_id(
    body: &fe2o3_kernel_ir::FunctionBody,
) -> Result<u32, ExecutionCapabilityProjectionErrorV13> {
    body.parameters
        .iter()
        .copied()
        .chain(
            body.blocks
                .iter()
                .flat_map(|block| block.parameters.iter().map(|value| value.id)),
        )
        .chain(
            body.blocks
                .iter()
                .flat_map(|block| &block.operations)
                .flat_map(|operation| operation.results.iter().map(|value| value.id)),
        )
        .map(|value| value.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(ExecutionCapabilityProjectionErrorV13::Invalid(
            "SSA value identity overflow during capability projection",
        ))
}

fn global_aliases(
    body: &fe2o3_kernel_ir::FunctionBody,
) -> Result<Vec<(ValueId, ValueId)>, ExecutionCapabilityProjectionErrorV13> {
    let count = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::GlobalCapabilityBind(_) | OperationKind::GlobalCapabilityIndex(_)
            )
        })
        .count();
    let mut aliases = Vec::new();
    aliases
        .try_reserve_exact(count)
        .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
    for operation in body.blocks.iter().flat_map(|block| &block.operations) {
        let target = match operation.kind {
            OperationKind::GlobalCapabilityBind(bind) => Some(bind.physical),
            OperationKind::GlobalCapabilityIndex(index) => Some(index.index),
            _ => None,
        };
        if let Some(target) = target {
            let [result] = operation.results.as_slice() else {
                return Err(ExecutionCapabilityProjectionErrorV13::Invalid(
                    "logical global operation has invalid result arity",
                ));
            };
            aliases.push((result.id, target));
        }
    }
    Ok(aliases)
}

fn scalar_identities(
    body: &fe2o3_kernel_ir::FunctionBody,
    types: &BTreeMap<ValueId, Type>,
) -> Result<
    BTreeMap<fe2o3_kernel_ir::ExecutionTypeIdentityV1, ScalarType>,
    ExecutionCapabilityProjectionErrorV13,
> {
    let mut identities = BTreeMap::new();
    for operation in body.blocks.iter().flat_map(|block| &block.operations) {
        let OperationKind::ExecutionCapability(contract) = &operation.kind else {
            continue;
        };
        use ExecutionCapabilityOperationV1 as Capability;
        let explicit = match &contract.operation {
            Capability::Atomic {
                element,
                value_type,
                ..
            }
            | Capability::WorkgroupCollective {
                element,
                value_type,
                ..
            }
            | Capability::SubgroupCollective {
                element,
                value_type,
                ..
            } => Some((*element, *value_type)),
            Capability::LdsInitializeByInvocation { element, .. }
            | Capability::MemoryStore { element, .. } => contract
                .operands
                .iter()
                .filter_map(|value| types.get(value).and_then(Type::as_scalar))
                .next_back()
                .map(|scalar| (*element, scalar)),
            Capability::RawMemoryBind { element, .. } | Capability::AsyncCopy { element, .. } => {
                contract
                    .operands
                    .iter()
                    .filter_map(|value| match types.get(value) {
                        Some(Type::Pointer(pointer)) => pointer.pointee.as_scalar(),
                        Some(Type::Slice(slice)) => slice.element.as_scalar(),
                        _ => None,
                    })
                    .next()
                    .map(|scalar| (*element, scalar))
            }
            Capability::LdsReadPublished { element, .. }
            | Capability::MemoryLoad { element, .. } => operation
                .results
                .iter()
                .filter_map(|result| result.ty.as_scalar())
                .find(|scalar| *scalar != ScalarType::Bool)
                .map(|scalar| (*element, scalar)),
            _ => None,
        };
        if let Some((identity, scalar)) = explicit
            && identities
                .insert(identity, scalar)
                .is_some_and(|old| old != scalar)
        {
            return Err(ExecutionCapabilityProjectionErrorV13::Invalid(
                "one execution element identity names different physical scalar types",
            ));
        }
    }
    Ok(identities)
}

#[allow(clippy::too_many_arguments)]
fn project_operation(
    results: Vec<ValueDef>,
    contract: ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    scalar_identities: &BTreeMap<fe2o3_kernel_ir::ExecutionTypeIdentityV1, ScalarType>,
    aliases: &mut Vec<(ValueId, ValueId)>,
    promoted: &mut BTreeMap<ValueId, Type>,
    dynamic_extents: &mut BTreeMap<ValueId, ValueId>,
    pending_async_copies: &mut BTreeMap<ValueId, PendingAsyncCopyProjectionV13>,
    next_value: &mut u32,
    workgroup: Option<fe2o3_kernel_ir::WorkgroupSize>,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    use ExecutionCapabilityOperationV1 as Capability;
    use IncompleteExecutionCapabilityOperationV13 as Incomplete;

    let operation = contract.operation.clone();
    match operation {
        Capability::WorkgroupDerive { .. } | Capability::SubgroupDerive { .. } => {
            require_only_logical_results(&results)?;
            Ok(Vec::new())
        }
        Capability::LdsAllocate {
            element,
            layout,
            elements,
            ..
        } => project_workgroup_allocation(
            results,
            element,
            layout,
            elements,
            scalar_identities,
            promoted,
            Incomplete::LdsAllocate,
        ),
        Capability::WorkgroupMemoryAllocate {
            element,
            layout,
            elements,
            ..
        } => project_workgroup_allocation(
            results,
            element,
            layout,
            elements,
            scalar_identities,
            promoted,
            Incomplete::WorkgroupMemoryAllocate,
        ),
        Capability::PrivateMemoryAllocate {
            element,
            layout,
            elements,
            ..
        } => project_private_allocation(
            results,
            element,
            layout,
            elements,
            scalar_identities,
            promoted,
            next_value,
        ),
        Capability::LdsInitializeByInvocation {
            element,
            layout,
            elements,
            ..
        } => project_lds_initialize(
            results,
            &contract,
            types,
            aliases,
            promoted,
            next_value,
            workgroup,
            element,
            layout,
            elements,
            scalar_identities,
        ),
        Capability::LdsPublish { .. }
        | Capability::WorkgroupMemoryPublish { .. }
        | Capability::WorkgroupBarrier { .. } => {
            let semantics = match operation {
                Capability::WorkgroupBarrier { semantics, .. } => semantics,
                _ => ExecutionMemorySemanticsV1 {
                    scope: fe2o3_kernel_ir::ExecutionMemoryScopeV1::Workgroup,
                    ordering: fe2o3_kernel_ir::ExecutionMemoryOrderingV1::AcquireRelease,
                    spaces: fe2o3_kernel_ir::ExecutionMemorySpacesV1::Workgroup,
                },
            };
            alias_logical_results_to_first_physical_operand(
                &results, &contract, types, aliases, promoted,
            )?;
            Ok(vec![workgroup_barrier(semantics)])
        }
        Capability::SubgroupBarrier {
            semantics, width, ..
        } => {
            require_only_logical_results(&results)?;
            project_subgroup_barrier(semantics, width, next_value)
        }
        Capability::WorkgroupFence { semantics, .. }
        | Capability::SubgroupFence { semantics, .. } => {
            require_only_logical_results(&results)?;
            Ok(vec![Operation::new(
                Vec::new(),
                OperationKind::Fence(fence(semantics)),
            )])
        }
        Capability::Atomic { .. } => project_atomic(
            results,
            &contract,
            types,
            aliases,
            promoted,
            dynamic_extents,
            next_value,
        ),
        Capability::WorkgroupCollective {
            kind,
            element,
            value_type,
            layout,
            elements,
            ..
        } => project_workgroup_collective(
            results, &contract, types, aliases, promoted, next_value, workgroup, kind, element,
            value_type, layout, elements,
        ),
        Capability::SubgroupCollective {
            kind,
            value_type,
            width,
            ..
        } => project_subgroup_collective(
            results, &contract, types, aliases, promoted, next_value, workgroup, kind, value_type,
            width,
        ),
        Capability::MatrixAccess { .. } => {
            require_only_logical_results(&results)?;
            Ok(Vec::new())
        }
        Capability::LdsReadPublished {
            layout, elements, ..
        } => project_bounded_load(
            results,
            &contract,
            types,
            aliases,
            promoted,
            next_value,
            layout,
            elements,
            AddressSpace::Workgroup,
            Incomplete::LdsReadPublished,
        ),
        Capability::WorkgroupMemoryIndex { .. } => {
            project_workgroup_index(results, promoted, next_value)
        }
        Capability::MemoryLoad {
            layout,
            space,
            access,
            ..
        } => project_memory_load(
            results,
            &contract,
            types,
            aliases,
            promoted,
            dynamic_extents,
            next_value,
            layout,
            space,
            access,
        ),
        Capability::MemoryStore {
            layout,
            space,
            access,
            ..
        } => project_memory_store(
            results,
            &contract,
            types,
            aliases,
            promoted,
            dynamic_extents,
            next_value,
            layout,
            space,
            access,
        ),
        Capability::RawMemoryBind {
            extent,
            element,
            layout,
            space,
            access,
            ..
        } => project_raw_memory_bind(
            results,
            &contract,
            types,
            scalar_identities,
            aliases,
            promoted,
            dynamic_extents,
            extent,
            element,
            layout,
            space,
            access,
        ),
        Capability::AsyncCopy {
            element,
            layout,
            elements,
            ..
        } => project_async_copy(
            results,
            &contract,
            types,
            scalar_identities,
            aliases,
            promoted,
            dynamic_extents,
            pending_async_copies,
            next_value,
            workgroup,
            element,
            layout,
            elements,
        ),
        Capability::AsyncWait {
            element,
            layout,
            elements,
            ..
        } => project_async_wait(
            results,
            &contract,
            types,
            scalar_identities,
            aliases,
            promoted,
            pending_async_copies,
            element,
            layout,
            elements,
        ),
    }
}

fn require_only_logical_results(
    results: &[ValueDef],
) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
    if results
        .iter()
        .all(|result| result.ty.contains_logical_capability())
    {
        Ok(())
    } else {
        Err(ExecutionCapabilityProjectionErrorV13::Invalid(
            "capability-only projection carried an unexpected physical result",
        ))
    }
}

fn project_workgroup_allocation(
    mut results: Vec<ValueDef>,
    element: fe2o3_kernel_ir::ExecutionTypeIdentityV1,
    layout: ExecutionElementLayoutV1,
    elements: u64,
    scalar_identities: &BTreeMap<fe2o3_kernel_ir::ExecutionTypeIdentityV1, ScalarType>,
    promoted: &mut BTreeMap<ValueId, Type>,
    incomplete: IncompleteExecutionCapabilityOperationV13,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let Some(scalar) = scalar_identities.get(&element).copied() else {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    };
    validate_layout(layout, scalar)?;
    let Some(result) = sole_logical_result_mut(&mut results) else {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    };
    let elements = u32::try_from(elements)
        .map_err(|_| ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete))?;
    result.ty = Type::pointer(
        Type::Scalar(scalar),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    promoted.insert(result.id, result.ty.clone());
    Ok(vec![Operation::new(
        results,
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: Type::Scalar(scalar),
            extent: WorkgroupMemoryExtent::Static(elements),
            alignment: u32::from(layout.byte_alignment),
        }),
    )])
}

fn project_private_allocation(
    mut results: Vec<ValueDef>,
    element: fe2o3_kernel_ir::ExecutionTypeIdentityV1,
    layout: ExecutionElementLayoutV1,
    elements: u64,
    scalar_identities: &BTreeMap<fe2o3_kernel_ir::ExecutionTypeIdentityV1, ScalarType>,
    promoted: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = IncompleteExecutionCapabilityOperationV13::PrivateMemoryAllocate;
    let Some(scalar) = scalar_identities.get(&element).copied() else {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    };
    validate_layout(layout, scalar)?;
    let Some(result) = sole_logical_result_mut(&mut results) else {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    };
    let count = u32::try_from(elements)
        .map_err(|_| ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete))?;
    result.ty = Type::pointer(
        Type::Scalar(scalar),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    promoted.insert(result.id, result.ty.clone());
    let mut emitter = Emitter::new(next_value);
    let count_id = emitter.fresh(
        Type::INDEX,
        OperationKind::Constant(Constant::Index(u64::from(count))),
    )?;
    emitter.push(Operation::new(
        results,
        OperationKind::Alloca {
            element: Type::Scalar(scalar),
            count: Some(count_id),
            address_space: AddressSpace::Private,
            alignment: u32::from(layout.byte_alignment),
        },
    ))?;
    Ok(emitter.finish())
}

fn sole_logical_result_mut(results: &mut [ValueDef]) -> Option<&mut ValueDef> {
    let mut logical = results
        .iter_mut()
        .filter(|result| result.ty.contains_logical_capability());
    let result = logical.next()?;
    logical.next().is_none().then_some(result)
}

fn validate_layout(
    layout: ExecutionElementLayoutV1,
    scalar: ScalarType,
) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
    if scalar_bytes(scalar) == Some(layout.byte_size)
        && layout.byte_alignment != 0
        && layout.byte_alignment.is_power_of_two()
        && u32::from(layout.byte_alignment) >= layout.byte_size
    {
        Ok(())
    } else {
        Err(ExecutionCapabilityProjectionErrorV13::Invalid(
            "execution element layout differs from its physical scalar layout",
        ))
    }
}

const fn scalar_bytes(scalar: ScalarType) -> Option<u32> {
    match scalar {
        ScalarType::Bool | ScalarType::I8 | ScalarType::U8 => Some(1),
        ScalarType::I16 | ScalarType::U16 | ScalarType::F16 | ScalarType::Bf16 => Some(2),
        ScalarType::I32 | ScalarType::U32 | ScalarType::F32 => Some(4),
        ScalarType::I64 | ScalarType::U64 | ScalarType::F64 => Some(8),
        ScalarType::I128 | ScalarType::U128 => Some(16),
        ScalarType::Index => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn project_lds_initialize(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    aliases: &mut Vec<(ValueId, ValueId)>,
    promoted: &BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    workgroup: Option<fe2o3_kernel_ir::WorkgroupSize>,
    element: fe2o3_kernel_ir::ExecutionTypeIdentityV1,
    layout: ExecutionElementLayoutV1,
    elements: u64,
    scalar_identities: &BTreeMap<fe2o3_kernel_ir::ExecutionTypeIdentityV1, ScalarType>,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = IncompleteExecutionCapabilityOperationV13::LdsInitializeByInvocation;
    let Some(scalar) = scalar_identities.get(&element).copied() else {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    };
    validate_layout(layout, scalar)?;
    require_linear_workgroup(workgroup, elements, incomplete)?;
    let physical = physical_operands(contract, types, aliases, promoted)?;
    let pointer = unique_pointer(
        &physical,
        types,
        aliases,
        promoted,
        AddressSpace::Workgroup,
        scalar,
    )
    .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
        incomplete,
    ))?;
    let value = unique_scalar(&physical, types, scalar).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    for result in &results {
        if result.ty.contains_logical_capability() {
            aliases.push((result.id, pointer));
        } else {
            return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
                incomplete,
            ));
        }
    }
    let mut emitter = Emitter::new(next_value);
    let rank = emitter.fresh(
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    )?;
    let location = emitter.fresh(
        pointer_type(scalar, AddressSpace::Workgroup),
        OperationKind::GetElementPointer {
            base: pointer,
            offset: rank,
        },
    )?;
    emitter.push(Operation::new(
        Vec::new(),
        OperationKind::Store {
            pointer: location,
            value,
            access: MemoryAccess::new(AddressSpace::Workgroup, u32::from(layout.byte_alignment)),
        },
    ))?;
    Ok(emitter.finish())
}

fn alias_logical_results_to_first_physical_operand(
    results: &[ValueDef],
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    aliases: &mut Vec<(ValueId, ValueId)>,
    promoted: &BTreeMap<ValueId, Type>,
) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
    let physical = physical_operands(contract, types, aliases, promoted)?;
    let backing = physical.iter().copied().find(|value| {
        resolved_type(*value, types, aliases, promoted)
            .is_some_and(|ty| matches!(ty, Type::Pointer(_) | Type::Slice(_)))
    });
    for result in results {
        if !result.ty.contains_logical_capability() {
            return Err(ExecutionCapabilityProjectionErrorV13::Invalid(
                "synchronization capability carried an unexpected physical result",
            ));
        }
        if let Some(backing) = backing {
            aliases.push((result.id, backing));
        }
    }
    Ok(())
}

fn workgroup_barrier(semantics: ExecutionMemorySemanticsV1) -> Operation {
    Operation::new(
        Vec::new(),
        OperationKind::WorkgroupBarrier(fe2o3_kernel_ir::WorkgroupBarrier {
            memory_scope: semantics.scope.synchronization_scope(),
            semantics: BarrierSemantics::new(
                semantics.ordering.memory_ordering(),
                semantics.spaces.address_spaces(),
            ),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    )
}

fn fence(semantics: ExecutionMemorySemanticsV1) -> Fence {
    Fence {
        memory_scope: semantics.scope.synchronization_scope(),
        semantics: BarrierSemantics::new(
            semantics.ordering.memory_ordering(),
            semantics.spaces.address_spaces(),
        ),
    }
}

fn project_subgroup_barrier(
    semantics: ExecutionMemorySemanticsV1,
    width: u32,
    next_value: &mut u32,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let wave = wave_width(width).ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
        IncompleteExecutionCapabilityOperationV13::SubgroupBarrier,
    ))?;
    let mut emitter = Emitter::new(next_value);
    let predicate = emitter.fresh(Type::BOOL, OperationKind::Constant(Constant::Bool(true)))?;
    let synchronized = emitter.fresh(
        Type::BOOL,
        OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::All { predicate },
            wave,
        )),
    )?;
    let _ = synchronized;
    emitter.push(Operation::new(
        Vec::new(),
        OperationKind::Fence(fence(semantics)),
    ))?;
    Ok(emitter.finish())
}

#[allow(clippy::too_many_arguments)]
fn project_atomic(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    aliases: &mut Vec<(ValueId, ValueId)>,
    promoted: &mut BTreeMap<ValueId, Type>,
    dynamic_extents: &BTreeMap<ValueId, ValueId>,
    next_value: &mut u32,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let ExecutionCapabilityOperationV1::Atomic {
        kind,
        value_type,
        address_space,
        scope,
        success,
        failure,
        ..
    } = contract.operation
    else {
        unreachable!();
    };
    let physical = physical_operands(contract, types, aliases, promoted)?;
    if kind == ExecutionAtomicKindV1::BindGlobalView {
        let Some(slice) = unique_slice(
            &physical,
            types,
            aliases,
            promoted,
            AddressSpace::Global,
            value_type,
        ) else {
            return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
                IncompleteExecutionCapabilityOperationV13::Atomic,
            ));
        };
        for result in results {
            if !result.ty.contains_logical_capability() {
                return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
                    IncompleteExecutionCapabilityOperationV13::Atomic,
                ));
            }
            aliases.push((result.id, slice));
        }
        return Ok(Vec::new());
    }
    if kind == ExecutionAtomicKindV1::BindGlobalLocation {
        return project_atomic_location(
            results,
            contract,
            physical,
            types,
            aliases,
            promoted,
            dynamic_extents,
            next_value,
            value_type,
        );
    }

    let space = address_space.address_space();
    let pointer = unique_pointer(&physical, types, aliases, promoted, space, value_type).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(
            IncompleteExecutionCapabilityOperationV13::Atomic,
        ),
    )?;
    let scalars = physical
        .iter()
        .copied()
        .filter(|value| {
            *value != pointer && types.get(value).and_then(Type::as_scalar) == Some(value_type)
        })
        .collect::<Vec<_>>();
    let (atomic_kind, value, compare) = match kind {
        ExecutionAtomicKindV1::Load => (AtomicKind::Load, None, None),
        ExecutionAtomicKindV1::Store => (AtomicKind::Store, scalars.first().copied(), None),
        ExecutionAtomicKindV1::FetchAdd => (AtomicKind::Add, scalars.first().copied(), None),
        ExecutionAtomicKindV1::CompareExchange => (
            AtomicKind::CompareExchange,
            scalars.get(1).copied(),
            scalars.first().copied(),
        ),
        ExecutionAtomicKindV1::BindGlobalLocation | ExecutionAtomicKindV1::BindGlobalView => {
            unreachable!()
        }
    };
    let expected_scalars = match kind {
        ExecutionAtomicKindV1::Load => 0,
        ExecutionAtomicKindV1::Store | ExecutionAtomicKindV1::FetchAdd => 1,
        ExecutionAtomicKindV1::CompareExchange => 2,
        _ => unreachable!(),
    };
    if scalars.len() != expected_scalars {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            IncompleteExecutionCapabilityOperationV13::Atomic,
        ));
    }
    let expected_results = match kind {
        ExecutionAtomicKindV1::Store => Vec::new(),
        ExecutionAtomicKindV1::CompareExchange => {
            vec![Type::Scalar(value_type), Type::BOOL]
        }
        _ => vec![Type::Scalar(value_type)],
    };
    if physical_results(&results) != expected_results {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            IncompleteExecutionCapabilityOperationV13::Atomic,
        ));
    }
    let results = results
        .into_iter()
        .filter(|result| !result.ty.contains_logical_capability())
        .collect();
    Ok(vec![Operation::new(
        results,
        OperationKind::Atomic(Atomic {
            kind: atomic_kind,
            pointer,
            value,
            compare,
            access: MemoryAccess::new(space, scalar_bytes(value_type).unwrap_or(1)),
            scope: scope.synchronization_scope(),
            ordering: success.map_or(fe2o3_kernel_ir::MemoryOrdering::Relaxed, |order| {
                order.memory_ordering()
            }),
            failure_ordering: failure.map(|order| order.memory_ordering()),
        }),
    )])
}

#[allow(clippy::too_many_arguments)]
fn project_atomic_location(
    mut results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    physical: Vec<ValueId>,
    types: &BTreeMap<ValueId, Type>,
    aliases: &mut [(ValueId, ValueId)],
    promoted: &mut BTreeMap<ValueId, Type>,
    dynamic_extents: &BTreeMap<ValueId, ValueId>,
    next_value: &mut u32,
    scalar: ScalarType,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = IncompleteExecutionCapabilityOperationV13::Atomic;
    let slice = unique_slice(
        &physical,
        types,
        aliases,
        promoted,
        AddressSpace::Global,
        scalar,
    );
    let pointer = unique_pointer(
        &physical,
        types,
        aliases,
        promoted,
        AddressSpace::Global,
        scalar,
    );
    if slice.is_some() == pointer.is_some() {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    let index = unique_resolved_scalar(&physical, types, aliases, promoted, ScalarType::Index)
        .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ))?;
    let logical_index = results
        .iter()
        .position(|result| result.ty.contains_logical_capability())
        .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ))?;
    if results
        .iter()
        .filter(|result| result.ty.contains_logical_capability())
        .count()
        != 1
    {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    let pointer_result = results[logical_index].id;
    results[logical_index].ty = pointer_type(scalar, AddressSpace::Global);
    promoted.insert(pointer_result, results[logical_index].ty.clone());
    let outcome = results
        .iter()
        .find(|result| result.ty == Type::BOOL)
        .map(|result| result.id);
    let mut emitter = Emitter::new(next_value);
    let length = if let Some(slice) = slice {
        emitter.fresh(Type::INDEX, OperationKind::SliceLength { slice })?
    } else {
        let extent = source_memory_extent(contract, types, dynamic_extents).ok_or(
            ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
        )?;
        emit_extent(&mut emitter, extent)?
    };
    let present = if let Some(outcome) = outcome {
        emitter.with_id(
            outcome,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: index,
                rhs: length,
            },
        )?;
        outcome
    } else {
        emitter.fresh(
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: index,
                rhs: length,
            },
        )?
    };
    let zero = emitter.fresh(Type::INDEX, OperationKind::Constant(Constant::Index(0)))?;
    let safe_index = emitter.fresh(
        Type::INDEX,
        OperationKind::Select {
            condition: present,
            true_value: index,
            false_value: zero,
        },
    )?;
    let base = if let Some(slice) = slice {
        emitter.fresh(
            pointer_type(scalar, AddressSpace::Global),
            OperationKind::SliceData { slice },
        )?
    } else {
        pointer.unwrap()
    };
    emitter.with_id(
        pointer_result,
        pointer_type(scalar, AddressSpace::Global),
        OperationKind::GetElementPointer {
            base,
            offset: safe_index,
        },
    )?;
    if results
        .iter()
        .any(|result| result.id != pointer_result && result.ty != Type::BOOL)
    {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    Ok(emitter.finish())
}

#[allow(clippy::too_many_arguments)]
fn project_workgroup_collective(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    aliases: &[(ValueId, ValueId)],
    promoted: &BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    workgroup: Option<fe2o3_kernel_ir::WorkgroupSize>,
    kind: ExecutionCollectiveKindV1,
    _element: fe2o3_kernel_ir::ExecutionTypeIdentityV1,
    scalar: ScalarType,
    layout: ExecutionElementLayoutV1,
    elements: u64,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = IncompleteExecutionCapabilityOperationV13::WorkgroupCollective;
    if !matches!(scalar, ScalarType::U32 | ScalarType::I32 | ScalarType::F32) {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    validate_layout(layout, scalar)?;
    require_linear_workgroup(workgroup, elements, incomplete)?;
    if elements > 256
        || (kind == ExecutionCollectiveKindV1::ReduceSum && !elements.is_power_of_two())
    {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    let physical = physical_operands(contract, types, aliases, promoted)?;
    let scratch = unique_pointer(
        &physical,
        types,
        aliases,
        promoted,
        AddressSpace::Workgroup,
        scalar,
    )
    .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
        incomplete,
    ))?;
    let value = unique_scalar(&physical, types, scalar).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    let output = sole_scalar_result(&results, scalar).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    let mut emitter = Emitter::new(next_value);
    emit_workgroup_collective(
        &mut emitter,
        scratch,
        value,
        output,
        scalar,
        u32::try_from(elements).unwrap(),
        kind,
        u32::from(layout.byte_alignment),
    )?;
    Ok(emitter.finish())
}

#[allow(clippy::too_many_arguments)]
fn project_subgroup_collective(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    aliases: &[(ValueId, ValueId)],
    promoted: &BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    workgroup: Option<fe2o3_kernel_ir::WorkgroupSize>,
    kind: ExecutionCollectiveKindV1,
    scalar: ScalarType,
    width: u32,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = IncompleteExecutionCapabilityOperationV13::SubgroupCollective;
    let Some(workgroup) = workgroup else {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    };
    if workgroup.y != 1 || workgroup.z != 1 || workgroup.x % width != 0 {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    let wave = wave_width(width).ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
        incomplete,
    ))?;
    if !matches!(scalar, ScalarType::U32 | ScalarType::I32 | ScalarType::F32) {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    let physical = physical_operands(contract, types, aliases, promoted)?;
    let value = unique_scalar(&physical, types, scalar).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    let output = sole_scalar_result(&results, scalar).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    let mut emitter = Emitter::new(next_value);
    emit_subgroup_collective(&mut emitter, value, output, scalar, width, wave, kind)?;
    Ok(emitter.finish())
}

fn require_linear_workgroup(
    workgroup: Option<fe2o3_kernel_ir::WorkgroupSize>,
    elements: u64,
    incomplete: IncompleteExecutionCapabilityOperationV13,
) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
    let Some(workgroup) = workgroup else {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    };
    if workgroup.y == 1 && workgroup.z == 1 && u64::from(workgroup.x) == elements {
        Ok(())
    } else {
        Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ))
    }
}

#[allow(clippy::too_many_arguments)]
fn project_async_copy(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    scalar_identities: &BTreeMap<fe2o3_kernel_ir::ExecutionTypeIdentityV1, ScalarType>,
    aliases: &mut Vec<(ValueId, ValueId)>,
    promoted: &BTreeMap<ValueId, Type>,
    dynamic_extents: &BTreeMap<ValueId, ValueId>,
    pending_async_copies: &mut BTreeMap<ValueId, PendingAsyncCopyProjectionV13>,
    next_value: &mut u32,
    workgroup: Option<fe2o3_kernel_ir::WorkgroupSize>,
    element: fe2o3_kernel_ir::ExecutionTypeIdentityV1,
    layout: ExecutionElementLayoutV1,
    elements: u64,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = IncompleteExecutionCapabilityOperationV13::AsyncCopy;
    let scalar = scalar_identities.get(&element).copied().ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    validate_layout(layout, scalar)?;
    let workgroup = workgroup.ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
        incomplete,
    ))?;
    let participants = u64::from(workgroup.x)
        .checked_mul(u64::from(workgroup.y))
        .and_then(|xy| xy.checked_mul(u64::from(workgroup.z)))
        .filter(|participants| *participants != 0 && *participants == elements)
        .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ))?;
    let _ = participants;
    let physical = physical_operands(contract, types, aliases, promoted)?;
    let destination = unique_pointer(
        &physical,
        types,
        aliases,
        promoted,
        AddressSpace::Workgroup,
        scalar,
    )
    .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
        incomplete,
    ))?;
    let source_offset = unique_scalar(&physical, types, ScalarType::Index).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    let mut emitter = Emitter::new(next_value);
    let (source, source_extent, source_pointer_type) = if let Some(slice) = unique_slice(
        &physical,
        types,
        aliases,
        promoted,
        AddressSpace::Global,
        scalar,
    ) {
        let Type::Slice(slice_type) = resolved_type(slice, types, aliases, promoted).unwrap()
        else {
            unreachable!();
        };
        if !access_mode_satisfies(slice_type.access, AccessMode::ReadOnly) {
            return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
                incomplete,
            ));
        }
        let pointer = emitter.fresh(
            Type::pointer(
                Type::Scalar(scalar),
                AddressSpace::Global,
                slice_type.access,
            ),
            OperationKind::SliceData { slice },
        )?;
        let length = emitter.fresh(Type::INDEX, OperationKind::SliceLength { slice })?;
        (
            pointer,
            length,
            Type::pointer(
                Type::Scalar(scalar),
                AddressSpace::Global,
                slice_type.access,
            ),
        )
    } else {
        let pointer = unique_pointer(
            &physical,
            types,
            aliases,
            promoted,
            AddressSpace::Global,
            scalar,
        )
        .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ))?;
        let extent = source_memory_extent(contract, types, dynamic_extents).ok_or(
            ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
        )?;
        let pointer_type = resolved_type(pointer, types, aliases, promoted)
            .cloned()
            .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
                incomplete,
            ))?;
        (pointer, emit_extent(&mut emitter, extent)?, pointer_type)
    };
    let rank = emit_flat_local_rank(&mut emitter, workgroup)?;
    let source_index = binary(
        &mut emitter,
        Type::INDEX,
        BinaryOp::Add,
        source_offset,
        rank,
    )?;
    let present = compare(
        &mut emitter,
        ComparePredicate::LessThan,
        source_index,
        source_extent,
    )?;
    let zero_index = index_constant(&mut emitter, 0)?;
    let safe_index = select(&mut emitter, Type::INDEX, present, source_index, zero_index)?;
    let source_location = emitter.fresh(
        source_pointer_type,
        OperationKind::GetElementPointer {
            base: source,
            offset: safe_index,
        },
    )?;
    let zero = emitter.fresh(
        Type::Scalar(scalar),
        OperationKind::Constant(zero_constant(scalar).ok_or(
            ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
        )?),
    )?;
    let value = emitter.fresh(
        Type::Scalar(scalar),
        OperationKind::GuardedLoad {
            pointer: source_location,
            predicate: present,
            fallback: zero,
            access: MemoryAccess::new(AddressSpace::Global, u32::from(layout.byte_alignment)),
        },
    )?;
    let destination_location = emitter.fresh(
        pointer_type(scalar, AddressSpace::Workgroup),
        OperationKind::GetElementPointer {
            base: destination,
            offset: rank,
        },
    )?;
    emitter.push(Operation::new(
        Vec::new(),
        OperationKind::Store {
            pointer: destination_location,
            value,
            access: MemoryAccess::new(AddressSpace::Workgroup, u32::from(layout.byte_alignment)),
        },
    ))?;

    let mut logical = results
        .iter()
        .filter(|result| result.ty.contains_logical_capability());
    let pending = logical
        .next()
        .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ))?;
    if logical.next().is_some() || !physical_results(&results).is_empty() {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    aliases.push((pending.id, destination));
    let state = PendingAsyncCopyProjectionV13 {
        destination,
        scalar,
        layout,
        elements,
    };
    if pending_async_copies.insert(pending.id, state).is_some() {
        return Err(ExecutionCapabilityProjectionErrorV13::Invalid(
            "async pending capability was projected more than once",
        ));
    }
    Ok(emitter.finish())
}

fn source_memory_extent(
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    dynamic_extents: &BTreeMap<ValueId, ValueId>,
) -> Option<MemoryExtentProjectionV13> {
    let mut extents = contract.operands.iter().filter_map(|operand| {
        let Type::ExecutionCapability(capability) = types.get(operand)? else {
            return None;
        };
        let fe2o3_kernel_ir::ExecutionCapabilityRoleV1::MemoryView {
            space: ExecutionMemoryAddressSpaceV1::Global,
            extent,
            ..
        } = capability.role
        else {
            return None;
        };
        match extent {
            ExecutionMemoryExtentV1::Static(elements) => {
                Some(MemoryExtentProjectionV13::Static(elements))
            }
            ExecutionMemoryExtentV1::Dynamic(_) => dynamic_extents
                .get(operand)
                .copied()
                .map(MemoryExtentProjectionV13::Dynamic),
        }
    });
    let extent = extents.next()?;
    extents.next().is_none().then_some(extent)
}

#[allow(clippy::too_many_arguments)]
fn project_async_wait(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    scalar_identities: &BTreeMap<fe2o3_kernel_ir::ExecutionTypeIdentityV1, ScalarType>,
    aliases: &mut Vec<(ValueId, ValueId)>,
    _promoted: &BTreeMap<ValueId, Type>,
    pending_async_copies: &mut BTreeMap<ValueId, PendingAsyncCopyProjectionV13>,
    element: fe2o3_kernel_ir::ExecutionTypeIdentityV1,
    layout: ExecutionElementLayoutV1,
    elements: u64,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = IncompleteExecutionCapabilityOperationV13::AsyncWait;
    let mut pending = contract.operands.iter().filter_map(|operand| {
        pending_async_copies
            .get(operand)
            .copied()
            .map(|state| (*operand, state))
    });
    let (pending_id, state) =
        pending
            .next()
            .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
                incomplete,
            ))?;
    if pending.next().is_some()
        || state.layout != layout
        || state.elements != elements
        || scalar_identities.get(&element).copied() != Some(state.scalar)
    {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    if !contract.operands.iter().any(|operand| {
        matches!(
            types.get(operand),
            Some(Type::ExecutionCapability(capability))
                if matches!(
                    capability.role,
                    fe2o3_kernel_ir::ExecutionCapabilityRoleV1::PendingAsyncCopy { .. }
                )
        )
    }) {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    if !physical_results(&results).is_empty() {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    let mut logical = 0_usize;
    for result in &results {
        if result.ty.contains_logical_capability() {
            logical += 1;
            aliases.push((result.id, state.destination));
        }
    }
    if logical == 0 {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    pending_async_copies.remove(&pending_id);
    Ok(vec![workgroup_barrier(ExecutionMemorySemanticsV1 {
        scope: fe2o3_kernel_ir::ExecutionMemoryScopeV1::Workgroup,
        ordering: fe2o3_kernel_ir::ExecutionMemoryOrderingV1::AcquireRelease,
        spaces: fe2o3_kernel_ir::ExecutionMemorySpacesV1::Workgroup,
    })])
}

fn emit_flat_local_rank(
    emitter: &mut Emitter<'_>,
    workgroup: fe2o3_kernel_ir::WorkgroupSize,
) -> Result<ValueId, ExecutionCapabilityProjectionErrorV13> {
    let x = emitter.fresh(
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    )?;
    if workgroup.y == 1 && workgroup.z == 1 {
        return Ok(x);
    }
    let y = emitter.fresh(
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::Y,
            },
            Type::INDEX,
        )),
    )?;
    let z = emitter.fresh(
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::Z,
            },
            Type::INDEX,
        )),
    )?;
    let size_x = index_constant(emitter, u64::from(workgroup.x))?;
    let size_y = index_constant(emitter, u64::from(workgroup.y))?;
    let zy = binary(emitter, Type::INDEX, BinaryOp::Multiply, z, size_y)?;
    let zy = binary(emitter, Type::INDEX, BinaryOp::Add, zy, y)?;
    let row = binary(emitter, Type::INDEX, BinaryOp::Multiply, zy, size_x)?;
    binary(emitter, Type::INDEX, BinaryOp::Add, row, x)
}

#[allow(clippy::too_many_arguments)]
fn project_raw_memory_bind(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    scalar_identities: &BTreeMap<fe2o3_kernel_ir::ExecutionTypeIdentityV1, ScalarType>,
    aliases: &mut Vec<(ValueId, ValueId)>,
    promoted: &BTreeMap<ValueId, Type>,
    dynamic_extents: &mut BTreeMap<ValueId, ValueId>,
    extent: ExecutionDynamicExtentV1,
    element: fe2o3_kernel_ir::ExecutionTypeIdentityV1,
    layout: ExecutionElementLayoutV1,
    space: ExecutionMemoryAddressSpaceV1,
    access: ExecutionMemoryAccessV1,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = IncompleteExecutionCapabilityOperationV13::RawMemoryBind;
    if extent.value_type != ScalarType::Index {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    let scalar = scalar_identities.get(&element).copied().ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    validate_layout(layout, scalar)?;
    let length = contract
        .operands
        .get(usize::from(extent.operand))
        .copied()
        .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ))?;
    if types.get(&length) != Some(&Type::INDEX) {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    let physical = physical_operands(contract, types, aliases, promoted)?;
    let pointer = unique_pointer(
        &physical,
        types,
        aliases,
        promoted,
        space.address_space(),
        scalar,
    )
    .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
        incomplete,
    ))?;
    let Some(Type::Pointer(pointer_type)) = resolved_type(pointer, types, aliases, promoted) else {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    };
    if !access_mode_satisfies(pointer_type.access, access.access_mode()) {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    let mut logical = results
        .iter()
        .filter(|result| result.ty.contains_logical_capability());
    let result = logical
        .next()
        .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ))?;
    if logical.next().is_some() || !physical_results(&results).is_empty() {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    aliases.push((result.id, pointer));
    if dynamic_extents.insert(result.id, length).is_some() {
        return Err(ExecutionCapabilityProjectionErrorV13::Invalid(
            "dynamic memory view was projected more than once",
        ));
    }
    Ok(Vec::new())
}

fn access_mode_satisfies(actual: AccessMode, required: AccessMode) -> bool {
    actual == required
        || matches!(
            (actual, required),
            (
                AccessMode::ReadWrite,
                AccessMode::ReadOnly | AccessMode::WriteOnly
            )
        )
}

#[allow(clippy::too_many_arguments)]
fn project_memory_load(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    aliases: &[(ValueId, ValueId)],
    promoted: &BTreeMap<ValueId, Type>,
    dynamic_extents: &BTreeMap<ValueId, ValueId>,
    next_value: &mut u32,
    layout: ExecutionElementLayoutV1,
    space: ExecutionMemoryAddressSpaceV1,
    access: ExecutionMemoryAccessV1,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = IncompleteExecutionCapabilityOperationV13::MemoryLoad;
    let extent = memory_view_extent(contract, types, dynamic_extents, space, access).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    project_bounded_load(
        results,
        contract,
        types,
        aliases,
        promoted,
        next_value,
        layout,
        extent,
        space.address_space(),
        incomplete,
    )
}

fn memory_view_extent(
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    dynamic_extents: &BTreeMap<ValueId, ValueId>,
    space: ExecutionMemoryAddressSpaceV1,
    access: ExecutionMemoryAccessV1,
) -> Option<MemoryExtentProjectionV13> {
    let mut views = contract.operands.iter().filter_map(|operand| {
        let Type::ExecutionCapability(capability) = types.get(operand)? else {
            return None;
        };
        let fe2o3_kernel_ir::ExecutionCapabilityRoleV1::MemoryView {
            space: role_space,
            access: role_access,
            extent,
            ..
        } = capability.role
        else {
            return None;
        };
        if role_space != space || role_access != access {
            return None;
        }
        match extent {
            ExecutionMemoryExtentV1::Static(elements) => {
                Some(MemoryExtentProjectionV13::Static(elements))
            }
            ExecutionMemoryExtentV1::Dynamic(_) => dynamic_extents
                .get(operand)
                .copied()
                .map(MemoryExtentProjectionV13::Dynamic),
        }
    });
    let extent = views.next()?;
    views.next().is_none().then_some(extent)
}

#[allow(clippy::too_many_arguments)]
fn project_memory_store(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    aliases: &[(ValueId, ValueId)],
    promoted: &BTreeMap<ValueId, Type>,
    dynamic_extents: &BTreeMap<ValueId, ValueId>,
    next_value: &mut u32,
    layout: ExecutionElementLayoutV1,
    space: ExecutionMemoryAddressSpaceV1,
    access: ExecutionMemoryAccessV1,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = IncompleteExecutionCapabilityOperationV13::MemoryStore;
    let extent = memory_view_extent(contract, types, dynamic_extents, space, access).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    let physical = physical_operands(contract, types, aliases, promoted)?;
    let (pointer, scalar) = physical
        .iter()
        .find_map(
            |value| match resolved_type(*value, types, aliases, promoted) {
                Some(Type::Pointer(pointer))
                    if pointer.address_space == space.address_space()
                        && pointer.pointee.as_scalar().is_some() =>
                {
                    Some((*value, pointer.pointee.as_scalar().unwrap()))
                }
                _ => None,
            },
        )
        .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ))?;
    validate_layout(layout, scalar)?;
    let index = unique_resolved_scalar(&physical, types, aliases, promoted, ScalarType::Index)
        .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ))?;
    let value = unique_scalar(&physical, types, scalar).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    let physical_result_types = physical_results(&results);
    if physical_result_types != vec![Type::BOOL] {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    let present = results
        .iter()
        .find(|result| result.ty == Type::BOOL)
        .unwrap()
        .id;
    let mut emitter = Emitter::new(next_value);
    let extent = emit_extent(&mut emitter, extent)?;
    emitter.with_id(
        present,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: index,
            rhs: extent,
        },
    )?;
    let zero = emitter.fresh(Type::INDEX, OperationKind::Constant(Constant::Index(0)))?;
    let safe_index = emitter.fresh(
        Type::INDEX,
        OperationKind::Select {
            condition: present,
            true_value: index,
            false_value: zero,
        },
    )?;
    let location = emitter.fresh(
        pointer_type(scalar, space.address_space()),
        OperationKind::GetElementPointer {
            base: pointer,
            offset: safe_index,
        },
    )?;
    emitter.push(Operation::new(
        Vec::new(),
        OperationKind::GuardedStore {
            pointer: location,
            predicate: present,
            value,
            access: MemoryAccess::new(space.address_space(), u32::from(layout.byte_alignment)),
        },
    ))?;
    Ok(emitter.finish())
}

#[allow(clippy::too_many_arguments)]
fn project_bounded_load(
    results: Vec<ValueDef>,
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    aliases: &[(ValueId, ValueId)],
    promoted: &BTreeMap<ValueId, Type>,
    next_value: &mut u32,
    layout: ExecutionElementLayoutV1,
    extent: impl Into<MemoryExtentProjectionV13>,
    space: AddressSpace,
    incomplete: IncompleteExecutionCapabilityOperationV13,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let physical = physical_operands(contract, types, aliases, promoted)?;
    let (pointer, scalar) = physical
        .iter()
        .find_map(
            |value| match resolved_type(*value, types, aliases, promoted) {
                Some(Type::Pointer(pointer))
                    if pointer.address_space == space && pointer.pointee.as_scalar().is_some() =>
                {
                    Some((*value, pointer.pointee.as_scalar().unwrap()))
                }
                _ => None,
            },
        )
        .ok_or(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ))?;
    validate_layout(layout, scalar)?;
    let index = unique_scalar(&physical, types, ScalarType::Index).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    let value_result = sole_scalar_result(&results, scalar).ok_or(
        ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
    )?;
    let present_result = results
        .iter()
        .filter(|result| result.ty == Type::BOOL)
        .map(|result| result.id)
        .next();
    if present_result.is_none()
        || physical_results(&results) != vec![Type::Scalar(scalar), Type::BOOL]
            && physical_results(&results) != vec![Type::BOOL, Type::Scalar(scalar)]
    {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    }
    let mut emitter = Emitter::new(next_value);
    let extent = emit_extent(&mut emitter, extent.into())?;
    let present = present_result.unwrap();
    emitter.with_id(
        present,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: index,
            rhs: extent,
        },
    )?;
    let zero_index = emitter.fresh(Type::INDEX, OperationKind::Constant(Constant::Index(0)))?;
    let safe_index = emitter.fresh(
        Type::INDEX,
        OperationKind::Select {
            condition: present,
            true_value: index,
            false_value: zero_index,
        },
    )?;
    let location = emitter.fresh(
        pointer_type(scalar, space),
        OperationKind::GetElementPointer {
            base: pointer,
            offset: safe_index,
        },
    )?;
    let zero = emitter.fresh(
        Type::Scalar(scalar),
        OperationKind::Constant(zero_constant(scalar).ok_or(
            ExecutionCapabilityProjectionErrorV13::Incomplete(incomplete),
        )?),
    )?;
    emitter.with_id(
        value_result,
        Type::Scalar(scalar),
        OperationKind::GuardedLoad {
            pointer: location,
            predicate: present,
            fallback: zero,
            access: MemoryAccess::new(space, u32::from(layout.byte_alignment)),
        },
    )?;
    Ok(emitter.finish())
}

fn project_workgroup_index(
    mut results: Vec<ValueDef>,
    promoted: &mut BTreeMap<ValueId, Type>,
    next_value: &mut u32,
) -> Result<Vec<Operation>, ExecutionCapabilityProjectionErrorV13> {
    let incomplete = IncompleteExecutionCapabilityOperationV13::WorkgroupMemoryIndex;
    let Some(witness) = sole_logical_result_mut(&mut results) else {
        return Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            incomplete,
        ));
    };
    let rank = witness.id;
    witness.ty = Type::INDEX;
    promoted.insert(rank, Type::INDEX);
    let mut emitter = Emitter::new(next_value);
    emitter.with_id(
        rank,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    )?;
    Ok(emitter.finish())
}

fn physical_operands(
    contract: &ExecutionCapabilityOpV1,
    types: &BTreeMap<ValueId, Type>,
    aliases: &[(ValueId, ValueId)],
    promoted: &BTreeMap<ValueId, Type>,
) -> Result<Vec<ValueId>, ExecutionCapabilityProjectionErrorV13> {
    let mut physical = Vec::new();
    physical
        .try_reserve(contract.operands.len())
        .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
    for operand in &contract.operands {
        let resolved = resolve_alias(*operand, aliases)?;
        let ty = types.get(operand).or_else(|| types.get(&resolved)).ok_or(
            ExecutionCapabilityProjectionErrorV13::Invalid(
                "execution capability operand has no SSA type",
            ),
        )?;
        if promoted.contains_key(operand)
            || promoted.contains_key(&resolved)
            || !matches!(
                ty,
                Type::KernelContext(_) | Type::ExecutionCapability(_) | Type::GlobalCapability(_)
            )
            || resolved != *operand
        {
            physical.push(resolved);
        }
    }
    let mut index = 0;
    while index < physical.len() {
        if physical[..index].contains(&physical[index]) {
            physical.remove(index);
        } else {
            index += 1;
        }
    }
    Ok(physical)
}

fn resolve_alias(
    mut value: ValueId,
    aliases: &[(ValueId, ValueId)],
) -> Result<ValueId, ExecutionCapabilityProjectionErrorV13> {
    for _ in 0..=aliases.len() {
        let Some((_, target)) = aliases.iter().find(|(source, _)| *source == value) else {
            return Ok(value);
        };
        value = *target;
    }
    Err(ExecutionCapabilityProjectionErrorV13::Invalid(
        "execution capability aliases form a cycle",
    ))
}

fn resolved_type<'a>(
    value: ValueId,
    types: &'a BTreeMap<ValueId, Type>,
    aliases: &[(ValueId, ValueId)],
    promoted: &'a BTreeMap<ValueId, Type>,
) -> Option<&'a Type> {
    let resolved = resolve_alias(value, aliases).ok()?;
    if let Some(ty) = promoted.get(&resolved).or_else(|| promoted.get(&value)) {
        return Some(ty);
    }
    types.get(&resolved)
}

fn unique_pointer(
    values: &[ValueId],
    types: &BTreeMap<ValueId, Type>,
    aliases: &[(ValueId, ValueId)],
    promoted: &BTreeMap<ValueId, Type>,
    space: AddressSpace,
    scalar: ScalarType,
) -> Option<ValueId> {
    let mut matches = values.iter().copied().filter(|value| {
        matches!(
            resolved_type(*value, types, aliases, promoted),
            Some(Type::Pointer(pointer))
                if pointer.address_space == space && pointer.pointee.as_scalar() == Some(scalar)
        )
    });
    let value = matches.next()?;
    matches.next().is_none().then_some(value)
}

fn unique_slice(
    values: &[ValueId],
    types: &BTreeMap<ValueId, Type>,
    aliases: &[(ValueId, ValueId)],
    promoted: &BTreeMap<ValueId, Type>,
    space: AddressSpace,
    scalar: ScalarType,
) -> Option<ValueId> {
    let mut matches = values.iter().copied().filter(|value| {
        matches!(
            resolved_type(*value, types, aliases, promoted),
            Some(Type::Slice(slice))
                if slice.address_space == space && slice.element.as_scalar() == Some(scalar)
        )
    });
    let value = matches.next()?;
    matches.next().is_none().then_some(value)
}

fn unique_scalar(
    values: &[ValueId],
    types: &BTreeMap<ValueId, Type>,
    scalar: ScalarType,
) -> Option<ValueId> {
    let mut matches = values
        .iter()
        .copied()
        .filter(|value| types.get(value).and_then(Type::as_scalar) == Some(scalar));
    let value = matches.next()?;
    matches.next().is_none().then_some(value)
}

fn unique_resolved_scalar(
    values: &[ValueId],
    types: &BTreeMap<ValueId, Type>,
    aliases: &[(ValueId, ValueId)],
    promoted: &BTreeMap<ValueId, Type>,
    scalar: ScalarType,
) -> Option<ValueId> {
    let mut matches = values.iter().copied().filter(|value| {
        resolved_type(*value, types, aliases, promoted).and_then(Type::as_scalar) == Some(scalar)
    });
    let value = matches.next()?;
    matches.next().is_none().then_some(value)
}

fn physical_results(results: &[ValueDef]) -> Vec<Type> {
    results
        .iter()
        .filter(|result| !result.ty.contains_logical_capability())
        .map(|result| result.ty.clone())
        .collect()
}

fn sole_scalar_result(results: &[ValueDef], scalar: ScalarType) -> Option<ValueId> {
    let mut results = results
        .iter()
        .filter(|result| result.ty == Type::Scalar(scalar));
    let result = results.next()?.id;
    results.next().is_none().then_some(result)
}

fn pointer_type(scalar: ScalarType, space: AddressSpace) -> Type {
    Type::pointer(Type::Scalar(scalar), space, AccessMode::ReadWrite)
}

const fn wave_width(width: u32) -> Option<WaveWidth> {
    match width {
        32 => Some(WaveWidth::Wave32),
        64 => Some(WaveWidth::Wave64),
        _ => None,
    }
}

fn zero_constant(scalar: ScalarType) -> Option<Constant> {
    match scalar {
        ScalarType::U32 => Some(Constant::U32(0)),
        ScalarType::I32 => Some(Constant::I32(0)),
        ScalarType::F32 => Some(Constant::F32Bits(0)),
        ScalarType::U64 => Some(Constant::U64(0)),
        ScalarType::I64 => Some(Constant::I64(0)),
        _ => None,
    }
}

fn emit_extent(
    emitter: &mut Emitter<'_>,
    extent: MemoryExtentProjectionV13,
) -> Result<ValueId, ExecutionCapabilityProjectionErrorV13> {
    match extent {
        MemoryExtentProjectionV13::Static(elements) => emitter.fresh(
            Type::INDEX,
            OperationKind::Constant(Constant::Index(elements)),
        ),
        MemoryExtentProjectionV13::Dynamic(value) => Ok(value),
    }
}

struct Emitter<'a> {
    next: &'a mut u32,
    operations: Vec<Operation>,
}

impl<'a> Emitter<'a> {
    fn new(next: &'a mut u32) -> Self {
        Self {
            next,
            operations: Vec::new(),
        }
    }

    fn fresh(
        &mut self,
        ty: Type,
        kind: OperationKind,
    ) -> Result<ValueId, ExecutionCapabilityProjectionErrorV13> {
        let value = ValueId(*self.next);
        *self.next =
            (*self.next)
                .checked_add(1)
                .ok_or(ExecutionCapabilityProjectionErrorV13::Invalid(
                    "SSA value identity overflow during capability projection",
                ))?;
        self.with_id(value, ty, kind)?;
        Ok(value)
    }

    fn with_id(
        &mut self,
        value: ValueId,
        ty: Type,
        kind: OperationKind,
    ) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
        self.push(Operation::effect_free(ValueDef::new(value, ty), kind))
    }

    fn push(&mut self, operation: Operation) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
        self.operations
            .try_reserve(1)
            .map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
        self.operations.push(operation);
        Ok(())
    }

    fn finish(self) -> Vec<Operation> {
        self.operations
    }
}

fn collective_barrier(
    emitter: &mut Emitter<'_>,
) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
    emitter.push(Operation::new(
        Vec::new(),
        OperationKind::WorkgroupBarrier(fe2o3_kernel_ir::WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                fe2o3_kernel_ir::MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    ))
}

#[allow(clippy::too_many_arguments)]
fn emit_workgroup_collective(
    emitter: &mut Emitter<'_>,
    scratch: ValueId,
    value: ValueId,
    output: ValueId,
    scalar: ScalarType,
    size: u32,
    kind: ExecutionCollectiveKindV1,
    alignment: u32,
) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
    let rank = emitter.fresh(
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    )?;
    workgroup_store(emitter, scratch, rank, value, scalar, alignment)?;
    collective_barrier(emitter)?;

    match kind {
        ExecutionCollectiveKindV1::ReduceSum => {
            let mut offset = size >> 1;
            while offset != 0 {
                let offset_value = index_constant(emitter, u64::from(offset))?;
                let active = compare(emitter, ComparePredicate::LessThan, rank, offset_value)?;
                let pair = binary(emitter, Type::INDEX, BinaryOp::Add, rank, offset_value)?;
                let zero = index_constant(emitter, 0)?;
                let safe_pair = select(emitter, Type::INDEX, active, pair, zero)?;
                let lhs = workgroup_load(emitter, scratch, rank, scalar, alignment, None)?;
                let rhs = workgroup_load(emitter, scratch, safe_pair, scalar, alignment, None)?;
                let sum = binary(emitter, Type::Scalar(scalar), BinaryOp::Add, lhs, rhs)?;
                let next = select(emitter, Type::Scalar(scalar), active, sum, lhs)?;
                collective_barrier(emitter)?;
                workgroup_store(emitter, scratch, rank, next, scalar, alignment)?;
                collective_barrier(emitter)?;
                offset >>= 1;
            }
            let zero = index_constant(emitter, 0)?;
            workgroup_load(emitter, scratch, zero, scalar, alignment, Some(output))?;
        }
        ExecutionCollectiveKindV1::InclusiveScanSum
        | ExecutionCollectiveKindV1::ExclusiveScanSum => {
            let mut offset = 1_u32;
            while offset < size {
                let offset_value = index_constant(emitter, u64::from(offset))?;
                let active = compare(
                    emitter,
                    ComparePredicate::GreaterThanOrEqual,
                    rank,
                    offset_value,
                )?;
                let safe_rank = select(emitter, Type::INDEX, active, rank, offset_value)?;
                let source = binary(
                    emitter,
                    Type::INDEX,
                    BinaryOp::Subtract,
                    safe_rank,
                    offset_value,
                )?;
                let current = workgroup_load(emitter, scratch, rank, scalar, alignment, None)?;
                let prefix = workgroup_load(emitter, scratch, source, scalar, alignment, None)?;
                let sum = binary(
                    emitter,
                    Type::Scalar(scalar),
                    BinaryOp::Add,
                    prefix,
                    current,
                )?;
                let next = select(emitter, Type::Scalar(scalar), active, sum, current)?;
                collective_barrier(emitter)?;
                workgroup_store(emitter, scratch, rank, next, scalar, alignment)?;
                collective_barrier(emitter)?;
                offset <<= 1;
            }
            if kind == ExecutionCollectiveKindV1::InclusiveScanSum {
                workgroup_load(emitter, scratch, rank, scalar, alignment, Some(output))?;
            } else {
                let one = index_constant(emitter, 1)?;
                let present = compare(emitter, ComparePredicate::GreaterThanOrEqual, rank, one)?;
                let safe_rank = select(emitter, Type::INDEX, present, rank, one)?;
                let predecessor = binary(emitter, Type::INDEX, BinaryOp::Subtract, safe_rank, one)?;
                let prior = workgroup_load(emitter, scratch, predecessor, scalar, alignment, None)?;
                let zero = emitter.fresh(
                    Type::Scalar(scalar),
                    OperationKind::Constant(zero_constant(scalar).unwrap()),
                )?;
                emitter.with_id(
                    output,
                    Type::Scalar(scalar),
                    OperationKind::Select {
                        condition: present,
                        true_value: prior,
                        false_value: zero,
                    },
                )?;
            }
        }
    }
    collective_barrier(emitter)
}

fn emit_subgroup_collective(
    emitter: &mut Emitter<'_>,
    value: ValueId,
    output: ValueId,
    scalar: ScalarType,
    width: u32,
    wave: WaveWidth,
    kind: ExecutionCollectiveKindV1,
) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
    if kind == ExecutionCollectiveKindV1::ReduceSum && scalar == ScalarType::F32 {
        emitter.with_id(
            output,
            Type::Scalar(scalar),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::ReduceF32 {
                    value,
                    tile_width: width,
                    kind: fe2o3_kernel_ir::WaveF32ReductionKindV1::Sum,
                },
                wave,
            )),
        )?;
        return Ok(());
    }

    let lane = emitter.fresh(
        Type::Scalar(ScalarType::U32),
        OperationKind::Wave(WaveOperation::full(WaveOperationKind::LaneId, wave)),
    )?;
    let mut current = value;
    let mut offset = if kind == ExecutionCollectiveKindV1::ReduceSum {
        width >> 1
    } else {
        1
    };
    loop {
        if (kind == ExecutionCollectiveKindV1::ReduceSum && offset == 0)
            || (kind != ExecutionCollectiveKindV1::ReduceSum && offset >= width)
        {
            break;
        }
        let offset_value = emitter.fresh(
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(offset)),
        )?;
        let (active, source) = if kind == ExecutionCollectiveKindV1::ReduceSum {
            let active = compare(emitter, ComparePredicate::LessThan, lane, offset_value)?;
            let pair = binary(
                emitter,
                Type::Scalar(ScalarType::U32),
                BinaryOp::Add,
                lane,
                offset_value,
            )?;
            let zero = emitter.fresh(
                Type::Scalar(ScalarType::U32),
                OperationKind::Constant(Constant::U32(0)),
            )?;
            (
                active,
                select(emitter, Type::Scalar(ScalarType::U32), active, pair, zero)?,
            )
        } else {
            let active = compare(
                emitter,
                ComparePredicate::GreaterThanOrEqual,
                lane,
                offset_value,
            )?;
            let safe = select(
                emitter,
                Type::Scalar(ScalarType::U32),
                active,
                lane,
                offset_value,
            )?;
            let source = binary(
                emitter,
                Type::Scalar(ScalarType::U32),
                BinaryOp::Subtract,
                safe,
                offset_value,
            )?;
            (active, source)
        };
        let shuffle_input = if scalar == ScalarType::F32 {
            emitter.fresh(
                Type::Scalar(ScalarType::U32),
                OperationKind::Cast {
                    kind: fe2o3_kernel_ir::CastKind::Bitcast,
                    value: current,
                    to: Type::Scalar(ScalarType::U32),
                },
            )?
        } else {
            current
        };
        let shuffled_bits = emitter.fresh(
            Type::Scalar(if scalar == ScalarType::I32 {
                ScalarType::I32
            } else {
                ScalarType::U32
            }),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::ShuffleIndex {
                    value: shuffle_input,
                    source_lane: source,
                    tile_width: width,
                },
                wave,
            )),
        )?;
        let shuffled = if scalar == ScalarType::F32 {
            emitter.fresh(
                Type::Scalar(ScalarType::F32),
                OperationKind::Cast {
                    kind: fe2o3_kernel_ir::CastKind::Bitcast,
                    value: shuffled_bits,
                    to: Type::Scalar(ScalarType::F32),
                },
            )?
        } else {
            shuffled_bits
        };
        let sum = binary(
            emitter,
            Type::Scalar(scalar),
            BinaryOp::Add,
            current,
            shuffled,
        )?;
        current = select(emitter, Type::Scalar(scalar), active, sum, current)?;
        if kind == ExecutionCollectiveKindV1::ReduceSum {
            offset >>= 1;
        } else {
            offset <<= 1;
        }
    }

    let source = match kind {
        ExecutionCollectiveKindV1::ReduceSum => 0,
        ExecutionCollectiveKindV1::InclusiveScanSum => {
            let always =
                emitter.fresh(Type::BOOL, OperationKind::Constant(Constant::Bool(true)))?;
            emitter.with_id(
                output,
                Type::Scalar(scalar),
                OperationKind::Select {
                    condition: always,
                    true_value: current,
                    false_value: current,
                },
            )?;
            return Ok(());
        }
        ExecutionCollectiveKindV1::ExclusiveScanSum => {
            let one = emitter.fresh(
                Type::Scalar(ScalarType::U32),
                OperationKind::Constant(Constant::U32(1)),
            )?;
            let present = compare(emitter, ComparePredicate::GreaterThanOrEqual, lane, one)?;
            let safe = select(emitter, Type::Scalar(ScalarType::U32), present, lane, one)?;
            let predecessor = binary(
                emitter,
                Type::Scalar(ScalarType::U32),
                BinaryOp::Subtract,
                safe,
                one,
            )?;
            let prior = subgroup_shuffle(emitter, current, scalar, predecessor, width, wave)?;
            let zero = emitter.fresh(
                Type::Scalar(scalar),
                OperationKind::Constant(zero_constant(scalar).unwrap()),
            )?;
            emitter.with_id(
                output,
                Type::Scalar(scalar),
                OperationKind::Select {
                    condition: present,
                    true_value: prior,
                    false_value: zero,
                },
            )?;
            return Ok(());
        }
    };
    let source = emitter.fresh(
        Type::Scalar(ScalarType::U32),
        OperationKind::Constant(Constant::U32(source)),
    )?;
    let reduced = subgroup_shuffle(emitter, current, scalar, source, width, wave)?;
    let always = emitter.fresh(Type::BOOL, OperationKind::Constant(Constant::Bool(true)))?;
    emitter.with_id(
        output,
        Type::Scalar(scalar),
        OperationKind::Select {
            condition: always,
            true_value: reduced,
            false_value: reduced,
        },
    )
}

fn subgroup_shuffle(
    emitter: &mut Emitter<'_>,
    value: ValueId,
    scalar: ScalarType,
    source: ValueId,
    width: u32,
    wave: WaveWidth,
) -> Result<ValueId, ExecutionCapabilityProjectionErrorV13> {
    let input = if scalar == ScalarType::F32 {
        emitter.fresh(
            Type::Scalar(ScalarType::U32),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::Bitcast,
                value,
                to: Type::Scalar(ScalarType::U32),
            },
        )?
    } else {
        value
    };
    let bits = emitter.fresh(
        Type::Scalar(if scalar == ScalarType::I32 {
            ScalarType::I32
        } else {
            ScalarType::U32
        }),
        OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::ShuffleIndex {
                value: input,
                source_lane: source,
                tile_width: width,
            },
            wave,
        )),
    )?;
    if scalar == ScalarType::F32 {
        emitter.fresh(
            Type::Scalar(ScalarType::F32),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::Bitcast,
                value: bits,
                to: Type::Scalar(ScalarType::F32),
            },
        )
    } else {
        Ok(bits)
    }
}

fn index_constant(
    emitter: &mut Emitter<'_>,
    value: u64,
) -> Result<ValueId, ExecutionCapabilityProjectionErrorV13> {
    emitter.fresh(Type::INDEX, OperationKind::Constant(Constant::Index(value)))
}

fn compare(
    emitter: &mut Emitter<'_>,
    predicate: ComparePredicate,
    lhs: ValueId,
    rhs: ValueId,
) -> Result<ValueId, ExecutionCapabilityProjectionErrorV13> {
    emitter.fresh(
        Type::BOOL,
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        },
    )
}

fn binary(
    emitter: &mut Emitter<'_>,
    ty: Type,
    op: BinaryOp,
    lhs: ValueId,
    rhs: ValueId,
) -> Result<ValueId, ExecutionCapabilityProjectionErrorV13> {
    emitter.fresh(ty, OperationKind::Binary { op, lhs, rhs })
}

fn select(
    emitter: &mut Emitter<'_>,
    ty: Type,
    condition: ValueId,
    true_value: ValueId,
    false_value: ValueId,
) -> Result<ValueId, ExecutionCapabilityProjectionErrorV13> {
    emitter.fresh(
        ty,
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        },
    )
}

fn workgroup_pointer(
    emitter: &mut Emitter<'_>,
    base: ValueId,
    offset: ValueId,
    scalar: ScalarType,
) -> Result<ValueId, ExecutionCapabilityProjectionErrorV13> {
    emitter.fresh(
        pointer_type(scalar, AddressSpace::Workgroup),
        OperationKind::GetElementPointer { base, offset },
    )
}

fn workgroup_store(
    emitter: &mut Emitter<'_>,
    base: ValueId,
    offset: ValueId,
    value: ValueId,
    scalar: ScalarType,
    alignment: u32,
) -> Result<(), ExecutionCapabilityProjectionErrorV13> {
    let pointer = workgroup_pointer(emitter, base, offset, scalar)?;
    emitter.push(Operation::new(
        Vec::new(),
        OperationKind::Store {
            pointer,
            value,
            access: MemoryAccess::new(AddressSpace::Workgroup, alignment),
        },
    ))
}

fn workgroup_load(
    emitter: &mut Emitter<'_>,
    base: ValueId,
    offset: ValueId,
    scalar: ScalarType,
    alignment: u32,
    output: Option<ValueId>,
) -> Result<ValueId, ExecutionCapabilityProjectionErrorV13> {
    let pointer = workgroup_pointer(emitter, base, offset, scalar)?;
    let kind = OperationKind::Load {
        pointer,
        access: MemoryAccess::new(AddressSpace::Workgroup, alignment),
    };
    if let Some(output) = output {
        emitter.with_id(output, Type::Scalar(scalar), kind)?;
        Ok(output)
    } else {
        emitter.fresh(Type::Scalar(scalar), kind)
    }
}
