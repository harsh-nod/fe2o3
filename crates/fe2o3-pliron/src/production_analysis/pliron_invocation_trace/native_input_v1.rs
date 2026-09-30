//! Total supported-operation census and exact root-qualified trace input.
use super::*;
use crate::kir_bridge_v1::canonical_trace_v1::{NativeFunctionV1, NativeSubjectV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Kernel, LaunchExtent, OperationKind,
    Terminator, VerifiedCanonicalKernelIrModuleV12,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTraceRefusalV1 {
    UnsupportedCall,
    UnsupportedEvent,
    UnsupportedTermination,
    UnsupportedValue,
    MissingHierarchy,
    Correspondence,
    Context,
}

/// Pending obligations remain pending even when event ordering is complete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeTraceObligationsV1 {
    pub source_correspondence: bool,
    pub runtime_launch: bool,
    pub byte_bounds_and_alignment: bool,
    pub provenance_and_alias: bool,
    pub memory_order_and_atomic_outcomes: bool,
    pub initialization_and_lifetime: bool,
}

impl NativeTraceObligationsV1 {
    const fn new(memory: bool) -> Self {
        Self {
            source_correspondence: true,
            runtime_launch: true,
            byte_bounds_and_alignment: memory,
            provenance_and_alias: memory,
            memory_order_and_atomic_outcomes: memory,
            initialization_and_lifetime: memory,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeTraceKeyV1 {
    owner: usize,
    context: usize,
    epoch: u64,
    function: usize,
    root: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeTraceGeometryV1 {
    pub global_extents: [u64; 3],
    pub workgroup_extents: Option<[u64; 3]>,
    pub subgroup_size: Option<u64>,
}

pub(crate) struct NativeTraceInputV1<'a, 'g> {
    pub(crate) key: NativeTraceKeyV1,
    pub(crate) function: &'a NativeFunctionV1<'g>,
    pub(crate) geometry: NativeTraceGeometryV1,
    pub(crate) obligations: NativeTraceObligationsV1,
}

impl<'a, 'g> NativeTraceInputV1<'a, 'g> {
    pub(crate) fn derive(
        owner: &VerifiedCanonicalKernelIrModuleV12,
        context: &Context,
        function: &'a NativeFunctionV1<'g>,
        root: usize,
        epoch: u64,
        budget: &mut Budget<'_>,
    ) -> Result<Self, PlironTraceFailureV1> {
        budget
            .charge_work(1)
            .map_err(PlironTraceFailureV1::NativeResource)?;
        let kernel: &Kernel = owner
            .module()
            .kernels
            .get(root)
            .ok_or_else(|| failure(0, 0, NativeTraceRefusalV1::Context))?;
        let canonical = owner
            .module()
            .functions
            .get(function.ordinal)
            .ok_or_else(|| failure(0, 0, NativeTraceRefusalV1::Context))?;
        if !std::ptr::eq(function.owner, owner)
            || canonical.id != kernel.entry
            || function
                .pointer
                .is_none_or(|pointer| pointer.try_deref(context).is_err())
            || context.ir_mutation_attempt_epoch().map(|v| v.value()).ok() != Some(epoch)
        {
            return Err(failure(0, 0, NativeTraceRefusalV1::Context));
        }
        let mut global = [1; 3];
        for (dimension, extent) in kernel.domain.extents().enumerate() {
            budget
                .charge_work(1)
                .map_err(PlironTraceFailureV1::NativeResource)?;
            match extent {
                LaunchExtent::Static(value) if value != 0 => global[dimension] = u64::from(value),
                _ => return Err(PlironTraceFailureV1::DynamicLaunch { dimension }),
            }
        }
        let geometry = NativeTraceGeometryV1 {
            global_extents: global,
            workgroup_extents: kernel
                .workgroup_size
                .map(|v| [u64::from(v.x), u64::from(v.y), u64::from(v.z)]),
            // Target capability tags do not authenticate an execution layout.
            subgroup_size: None,
        };
        let mut memory = false;
        for occurrence in &function.occurrences {
            budget
                .charge_work(1)
                .map_err(PlironTraceFailureV1::NativeResource)?;
            let refuse = |reason| failure(occurrence.block, occurrence.operation, reason);
            memory |= classify(occurrence.subject).map_err(refuse)?;
            if let NativeSubjectV1::Operation(operation) = occurrence.subject
                && let OperationKind::Barrier(barrier) = &operation.kind
            {
                require_scope(
                    barrier.execution_scope,
                    geometry,
                    occurrence.block,
                    occurrence.operation,
                )?;
            }
        }
        Ok(Self {
            key: NativeTraceKeyV1 {
                owner: std::ptr::from_ref(owner) as usize,
                context: std::ptr::from_ref(context) as usize,
                epoch,
                function: function.ordinal,
                root,
            },
            function,
            geometry,
            obligations: NativeTraceObligationsV1::new(memory),
        })
    }

    pub(crate) fn authenticate(
        &self,
        context: &Context,
        function: &pliron::builtin::ops::FuncOp,
    ) -> Result<(), PlironTraceFailureV1> {
        if self.key.context != std::ptr::from_ref(context) as usize
            || self.function.pointer != Some(function.get_operation())
            || context.ir_mutation_attempt_epoch().map(|v| v.value()).ok() != Some(self.key.epoch)
        {
            return Err(failure(0, 0, NativeTraceRefusalV1::Context));
        }
        Ok(())
    }
}

pub(crate) fn classify(subject: NativeSubjectV1<'_>) -> Result<bool, NativeTraceRefusalV1> {
    match subject {
        NativeSubjectV1::Operation(operation) => match &operation.kind {
            OperationKind::Constant(_)
            | OperationKind::Unary { .. }
            | OperationKind::Binary { .. }
            | OperationKind::Compare { .. }
            | OperationKind::Cast { .. }
            | OperationKind::Select { .. }
            | OperationKind::SliceLength { .. }
            | OperationKind::SliceData { .. }
            | OperationKind::GetElementPointer { .. }
            | OperationKind::Intrinsic(_) => Ok(false),
            OperationKind::Load { .. }
            | OperationKind::Store { .. }
            | OperationKind::Atomic(_)
            | OperationKind::Alloca { .. }
            | OperationKind::Barrier(_)
            | OperationKind::Fence(_) => Ok(true),
            OperationKind::Call { .. } => Err(NativeTraceRefusalV1::UnsupportedCall),
            _ => Err(NativeTraceRefusalV1::UnsupportedEvent),
        },
        NativeSubjectV1::Terminator(end) => match end {
            Terminator::Return { .. }
            | Terminator::Branch { .. }
            | Terminator::ConditionalBranch { .. }
            | Terminator::Switch { .. }
            | Terminator::IntegerSwitch { .. } => Ok(false),
            _ => Err(NativeTraceRefusalV1::UnsupportedTermination),
        },
    }
}

pub(crate) fn failure(
    block: usize,
    operation: usize,
    reason: NativeTraceRefusalV1,
) -> PlironTraceFailureV1 {
    PlironTraceFailureV1::Native {
        block,
        operation,
        reason,
    }
}

fn require_scope(
    scope: fe2o3_kernel_ir::SynchronizationScope,
    geometry: NativeTraceGeometryV1,
    block: usize,
    operation: usize,
) -> Result<(), PlironTraceFailureV1> {
    use fe2o3_kernel_ir::SynchronizationScope as Scope;
    if matches!(scope, Scope::Device | Scope::System) {
        return Err(PlironTraceFailureV1::UnsupportedGridSynchronization { block, operation });
    }
    if scope != Scope::Workgroup || geometry.workgroup_extents.is_none() {
        return Err(failure(
            block,
            operation,
            NativeTraceRefusalV1::MissingHierarchy,
        ));
    }
    let workgroup = geometry
        .workgroup_extents
        .ok_or(PlironTraceFailureV1::MissingExecutionLayout)?;
    for (dimension, (global_extent, workgroup_extent)) in geometry
        .global_extents
        .into_iter()
        .zip(workgroup)
        .enumerate()
    {
        if workgroup_extent == 0 {
            return Err(PlironTraceFailureV1::InvalidExecutionLayout);
        }
        if !global_extent.is_multiple_of(workgroup_extent) {
            return Err(PlironTraceFailureV1::PartialBarrierParticipants {
                scope: HierarchyAttr::Workgroup,
                dimension,
                global_extent,
                workgroup_extent,
            });
        }
    }
    Ok(())
}

pub(crate) fn native_resource(error: Resource) -> PlironTraceFailureV1 {
    PlironTraceFailureV1::NativeResource(error)
}
