//! Bounded, target-neutral `gpu.*` Pliron semantics.
//!
//! This shell represents abstract execution hierarchy, address spaces, and
//! synchronization. It is representation-only and cannot publish, load, or
//! authorize runtime work.

#![forbid(unsafe_code)]

use std::{error::Error, fmt};

use pliron::{
    attribute::Attribute,
    builtin::{
        ATTR_KEY_DEBUG_INFO,
        op_interfaces::{NOpdsInterface, NRegionsInterface, NResultsInterface, OneResultInterface},
    },
    common_traits::Verify,
    context::Context,
    derive::{op_interface, pliron_attr, pliron_op, pliron_type},
    dialect::{Dialect, DialectName},
    op::Op,
    operation::Operation,
    result::Result,
    r#type::{Type, Typed},
    verify_err,
};

mod general_gemm;

pub use general_gemm::{
    GeneralGemmEpilogueAttr, GeneralGemmEpilogueOp, GeneralGemmEpochAttr, GeneralGemmEpochOp,
    GeneralGemmGlobalTransferAttr, GeneralGemmGlobalTransferOp, GeneralGemmGridMappingAttr,
    GeneralGemmGridMappingOp, GeneralGemmLdsTransferAttr, GeneralGemmLdsTransferOp,
    GeneralGemmMfmaAttr, GeneralGemmMfmaOp, GeneralGemmPhaseLoopAttr, GeneralGemmPhaseLoopOp,
    GeneralGemmRuntimeAbiAttr, GeneralGemmRuntimeAbiOp,
};

/// Pliron dialect name.
pub const DIALECT_NAME: &str = "gpu";

pliron::dict_key!(
    GPU_REGISTRATION_KEY,
    "fe2o3_dialect_gpu_explicit_registration"
);

#[derive(Debug)]
struct RegistrationMarker;

/// Result of explicitly registering this dialect in a context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationOutcome {
    /// The complete dialect surface was explicitly registered.
    Registered,
    /// The same complete surface was already registered by this crate.
    AlreadyRegistered,
}

/// A fail-closed explicit registration error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationError {
    /// Another typed value already claimed this crate's marker key.
    MarkerCollision,
    /// The marker map referenced absent auxiliary data.
    CorruptMarker,
}

impl fmt::Display for RegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MarkerCollision => formatter.write_str("gpu registration marker collision"),
            Self::CorruptMarker => formatter.write_str("gpu registration marker is corrupt"),
        }
    }
}

impl Error for RegistrationError {}

/// Abstract GPU execution hierarchy.
#[pliron_attr(name = "gpu.hierarchy", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HierarchyAttr {
    Grid,
    Workgroup,
    Subgroup,
    Lane,
}

/// Target-neutral GPU memory spaces.
#[pliron_attr(name = "gpu.address_space", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AddressSpaceAttr {
    Private,
    Workgroup,
    Global,
    Constant,
}

/// Visibility scope for synchronization.
#[pliron_attr(name = "gpu.memory_scope", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MemoryScopeAttr {
    Subgroup,
    Workgroup,
    Device,
    System,
}

/// Memory ordering carried by a target-neutral synchronization operation.
#[pliron_attr(name = "gpu.memory_order", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MemoryOrderAttr {
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

impl MemoryOrderAttr {
    fn includes_release(self) -> bool {
        matches!(
            self,
            Self::Release | Self::AcquireRelease | Self::SequentiallyConsistent
        )
    }
}

/// SSA type for an abstract hierarchy coordinate.
#[pliron_type(
    name = "gpu.hierarchy_index",
    format = "$0",
    generate_get = true,
    verifier = "succ"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct HierarchyIndexType(HierarchyAttr);

impl HierarchyIndexType {
    pub const fn hierarchy(&self) -> HierarchyAttr {
        self.0
    }
}

/// SSA type naming an abstract address space without a target ABI.
#[pliron_type(
    name = "gpu.memory_space",
    format = "$0",
    generate_get = true,
    verifier = "succ"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MemorySpaceType(AddressSpaceAttr);

impl MemorySpaceType {
    pub const fn address_space(&self) -> AddressSpaceAttr {
        self.0
    }
}

/// Common interface for inert target-neutral GPU operations.
#[op_interface]
pub trait TargetNeutralGpuOpInterface {
    fn verify(_op: &dyn Op, _context: &Context) -> Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }

    fn is_target_neutral(&self) -> bool {
        true
    }

    fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Marker interface for operations with synchronization semantics.
#[op_interface]
pub trait SynchronizationOpInterface: TargetNeutralGpuOpInterface {
    fn verify(_op: &dyn Op, _context: &Context) -> Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }

    fn is_synchronization(&self) -> bool {
        true
    }
}

/// Materializes one abstract hierarchy coordinate.
#[pliron_op(
    name = "gpu.hierarchy_id",
    format = "attr($gpu_hierarchy_id_hierarchy, $HierarchyAttr) ` : ` type($0)",
    interfaces = [
        TargetNeutralGpuOpInterface,
        NOpdsInterface<0>,
        OneResultInterface,
        NRegionsInterface<0>
    ],
    attributes = (gpu_hierarchy_id_hierarchy: HierarchyAttr)
)]
pub struct HierarchyIdOp;

impl HierarchyIdOp {
    pub fn new(context: &mut Context, hierarchy: HierarchyAttr) -> Self {
        let result_type = HierarchyIndexType::get(context, hierarchy).into();
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![result_type],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_gpu_hierarchy_id_hierarchy(context, hierarchy);
        op
    }
}

impl Verify for HierarchyIdOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 0, 1, 1)?;
        let hierarchy = self
            .get_attr_gpu_hierarchy_id_hierarchy(context)
            .ok_or_else(|| {
                verification_error(self, context, "missing typed hierarchy attribute")
            })?;
        let result_type = self
            .get_operation()
            .deref(context)
            .get_result(0)
            .get_type(context);
        let result_type = result_type.deref(context);
        let Some(result_type) = result_type.downcast_ref::<HierarchyIndexType>() else {
            return verify_err!(
                self.loc(context),
                "gpu.hierarchy_id result must have gpu.hierarchy_index type"
            );
        };
        if result_type.hierarchy() != *hierarchy {
            return verify_err!(
                self.loc(context),
                "gpu.hierarchy_id attribute and result hierarchy must match"
            );
        }
        Ok(())
    }
}

/// Materializes one abstract memory-space value.
#[pliron_op(
    name = "gpu.memory_space",
    format = "attr($gpu_memory_space_address_space, $AddressSpaceAttr) ` : ` type($0)",
    interfaces = [
        TargetNeutralGpuOpInterface,
        NOpdsInterface<0>,
        OneResultInterface,
        NRegionsInterface<0>
    ],
    attributes = (gpu_memory_space_address_space: AddressSpaceAttr)
)]
pub struct MemorySpaceOp;

impl MemorySpaceOp {
    pub fn new(context: &mut Context, address_space: AddressSpaceAttr) -> Self {
        let result_type = MemorySpaceType::get(context, address_space).into();
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![result_type],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_gpu_memory_space_address_space(context, address_space);
        op
    }
}

impl Verify for MemorySpaceOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 0, 1, 1)?;
        let address_space = self
            .get_attr_gpu_memory_space_address_space(context)
            .ok_or_else(|| {
                verification_error(self, context, "missing typed address_space attribute")
            })?;
        let result_type = self
            .get_operation()
            .deref(context)
            .get_result(0)
            .get_type(context);
        let result_type = result_type.deref(context);
        let Some(result_type) = result_type.downcast_ref::<MemorySpaceType>() else {
            return verify_err!(
                self.loc(context),
                "gpu.memory_space result must have gpu.memory_space type"
            );
        };
        if result_type.address_space() != *address_space {
            return verify_err!(
                self.loc(context),
                "gpu.memory_space attribute and result address space must match"
            );
        }
        Ok(())
    }
}

/// Collective execution and memory barrier.
#[pliron_op(
    name = "gpu.barrier",
    format = "attr($gpu_barrier_execution_scope, $HierarchyAttr) ` ` attr($gpu_barrier_memory_scope, $MemoryScopeAttr) ` ` attr($gpu_barrier_address_space, $AddressSpaceAttr) ` ` attr($gpu_barrier_order, $MemoryOrderAttr)",
    interfaces = [
        TargetNeutralGpuOpInterface,
        SynchronizationOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
        NRegionsInterface<0>
    ],
    attributes = (
        gpu_barrier_execution_scope: HierarchyAttr,
        gpu_barrier_memory_scope: MemoryScopeAttr,
        gpu_barrier_address_space: AddressSpaceAttr,
        gpu_barrier_order: MemoryOrderAttr
    )
)]
pub struct BarrierOp;

impl BarrierOp {
    pub fn new(
        context: &mut Context,
        execution_scope: HierarchyAttr,
        memory_scope: MemoryScopeAttr,
        address_space: AddressSpaceAttr,
        order: MemoryOrderAttr,
    ) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_gpu_barrier_execution_scope(context, execution_scope);
        op.set_attr_gpu_barrier_memory_scope(context, memory_scope);
        op.set_attr_gpu_barrier_address_space(context, address_space);
        op.set_attr_gpu_barrier_order(context, order);
        op
    }
}

impl Verify for BarrierOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 0, 0, 4)?;
        let execution_scope = required_attr(
            self,
            context,
            self.get_attr_gpu_barrier_execution_scope(context),
            "execution_scope",
        )?;
        let memory_scope = required_attr(
            self,
            context,
            self.get_attr_gpu_barrier_memory_scope(context),
            "memory_scope",
        )?;
        let address_space = required_attr(
            self,
            context,
            self.get_attr_gpu_barrier_address_space(context),
            "address_space",
        )?;
        let order = required_attr(
            self,
            context,
            self.get_attr_gpu_barrier_order(context),
            "order",
        )?;

        if !matches!(
            execution_scope,
            HierarchyAttr::Workgroup | HierarchyAttr::Subgroup
        ) {
            return verify_err!(
                self.loc(context),
                "gpu.barrier execution scope must be workgroup or subgroup"
            );
        }
        if memory_scope_rank(memory_scope) < hierarchy_memory_rank(execution_scope) {
            return verify_err!(
                self.loc(context),
                "gpu.barrier memory scope cannot be narrower than its execution scope"
            );
        }
        if address_space == AddressSpaceAttr::Private {
            return verify_err!(
                self.loc(context),
                "gpu.barrier cannot synchronize private memory"
            );
        }
        if !matches!(
            order,
            MemoryOrderAttr::AcquireRelease | MemoryOrderAttr::SequentiallyConsistent
        ) {
            return verify_err!(
                self.loc(context),
                "gpu.barrier requires acquire-release or sequentially-consistent ordering"
            );
        }
        if address_space == AddressSpaceAttr::Constant && order.includes_release() {
            return verify_err!(
                self.loc(context),
                "gpu.barrier cannot release to constant memory"
            );
        }
        Ok(())
    }
}

/// Non-collective target-neutral memory fence.
#[pliron_op(
    name = "gpu.fence",
    format = "attr($gpu_fence_memory_scope, $MemoryScopeAttr) ` ` attr($gpu_fence_address_space, $AddressSpaceAttr) ` ` attr($gpu_fence_order, $MemoryOrderAttr)",
    interfaces = [
        TargetNeutralGpuOpInterface,
        SynchronizationOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
        NRegionsInterface<0>
    ],
    attributes = (
        gpu_fence_memory_scope: MemoryScopeAttr,
        gpu_fence_address_space: AddressSpaceAttr,
        gpu_fence_order: MemoryOrderAttr
    )
)]
pub struct FenceOp;

impl FenceOp {
    pub fn new(
        context: &mut Context,
        memory_scope: MemoryScopeAttr,
        address_space: AddressSpaceAttr,
        order: MemoryOrderAttr,
    ) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_gpu_fence_memory_scope(context, memory_scope);
        op.set_attr_gpu_fence_address_space(context, address_space);
        op.set_attr_gpu_fence_order(context, order);
        op
    }
}

impl Verify for FenceOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 0, 0, 3)?;
        required_attr(
            self,
            context,
            self.get_attr_gpu_fence_memory_scope(context),
            "memory_scope",
        )?;
        let address_space = required_attr(
            self,
            context,
            self.get_attr_gpu_fence_address_space(context),
            "address_space",
        )?;
        let order = required_attr(
            self,
            context,
            self.get_attr_gpu_fence_order(context),
            "order",
        )?;
        if address_space == AddressSpaceAttr::Private {
            return verify_err!(
                self.loc(context),
                "gpu.fence cannot synchronize private memory"
            );
        }
        if address_space == AddressSpaceAttr::Constant && order.includes_release() {
            return verify_err!(
                self.loc(context),
                "gpu.fence cannot release to constant memory"
            );
        }
        Ok(())
    }
}

fn verification_error(op: &dyn Op, context: &Context, message: &str) -> pliron::result::Error {
    pliron::verify_error!(op.loc(context), "{message}")
}

fn required_attr<T: Copy>(
    op: &dyn Op,
    context: &Context,
    value: Option<std::cell::Ref<'_, T>>,
    name: &str,
) -> Result<T> {
    value
        .map(|value| *value)
        .ok_or_else(|| verification_error(op, context, &format!("missing typed {name} attribute")))
}

fn verify_closed_shape(
    op: &dyn Op,
    context: &Context,
    operands: usize,
    results: usize,
    attributes: usize,
) -> Result<()> {
    let operation = op.get_operation();
    let operation = operation.deref(context);
    let debug_info = operation.attributes.0.get(&*ATTR_KEY_DEBUG_INFO);
    let debug_info_is_valid = debug_info
        .map(|attribute| results != 0 && is_debug_info(attribute.as_ref()))
        .unwrap_or(true);
    let expected_attributes = attributes + usize::from(debug_info.is_some());
    if operation.get_num_operands() != operands
        || operation.get_num_results() != results
        || operation.get_num_successors() != 0
        || operation.num_regions() != 0
        || operation.attributes.0.len() != expected_attributes
        || !debug_info_is_valid
    {
        return verify_err!(
            op.loc(context),
            "{} has malformed or unbounded structural payload",
            op.get_opid()
        );
    }
    Ok(())
}

fn is_debug_info(attribute: &dyn Attribute) -> bool {
    let id = attribute.get_attr_id();
    id.dialect.as_ref() == "builtin" && AsRef::<str>::as_ref(&id.name) == "debug_info"
}

const fn hierarchy_memory_rank(hierarchy: HierarchyAttr) -> u8 {
    match hierarchy {
        HierarchyAttr::Lane => 0,
        HierarchyAttr::Subgroup => 1,
        HierarchyAttr::Workgroup => 2,
        HierarchyAttr::Grid => 3,
    }
}

const fn memory_scope_rank(scope: MemoryScopeAttr) -> u8 {
    match scope {
        MemoryScopeAttr::Subgroup => 1,
        MemoryScopeAttr::Workgroup => 2,
        MemoryScopeAttr::Device => 3,
        MemoryScopeAttr::System => 4,
    }
}

/// Explicitly registers every `gpu.*` type, attribute, and operation.
///
/// Repeated calls are side-effect free and report [`RegistrationOutcome::AlreadyRegistered`].
pub fn register_dialect(
    context: &mut Context,
) -> std::result::Result<RegistrationOutcome, RegistrationError> {
    if let Some(index) = context.aux_data_map.get(&*GPU_REGISTRATION_KEY).copied() {
        return match context.aux_data.get(index) {
            Some(marker) if marker.downcast_ref::<RegistrationMarker>().is_some() => {
                Ok(RegistrationOutcome::AlreadyRegistered)
            }
            Some(_) => Err(RegistrationError::MarkerCollision),
            None => Err(RegistrationError::CorruptMarker),
        };
    }

    let dialect_name = DialectName::try_new(DIALECT_NAME).expect("static gpu dialect name");
    Dialect::register(context, &dialect_name);

    <HierarchyAttr as Attribute>::register::<HierarchyAttr>(context);
    <AddressSpaceAttr as Attribute>::register::<AddressSpaceAttr>(context);
    <MemoryScopeAttr as Attribute>::register::<MemoryScopeAttr>(context);
    <MemoryOrderAttr as Attribute>::register::<MemoryOrderAttr>(context);
    <GeneralGemmRuntimeAbiAttr as Attribute>::register::<GeneralGemmRuntimeAbiAttr>(context);
    <GeneralGemmGridMappingAttr as Attribute>::register::<GeneralGemmGridMappingAttr>(context);
    <GeneralGemmPhaseLoopAttr as Attribute>::register::<GeneralGemmPhaseLoopAttr>(context);
    <GeneralGemmGlobalTransferAttr as Attribute>::register::<GeneralGemmGlobalTransferAttr>(
        context,
    );
    <GeneralGemmLdsTransferAttr as Attribute>::register::<GeneralGemmLdsTransferAttr>(context);
    <GeneralGemmEpochAttr as Attribute>::register::<GeneralGemmEpochAttr>(context);
    <GeneralGemmMfmaAttr as Attribute>::register::<GeneralGemmMfmaAttr>(context);
    <GeneralGemmEpilogueAttr as Attribute>::register::<GeneralGemmEpilogueAttr>(context);
    <HierarchyIndexType as Type>::register(context);
    <MemorySpaceType as Type>::register(context);
    <HierarchyIdOp as Op>::register(context);
    <MemorySpaceOp as Op>::register(context);
    <BarrierOp as Op>::register(context);
    <FenceOp as Op>::register(context);
    <GeneralGemmRuntimeAbiOp as Op>::register(context);
    <GeneralGemmGridMappingOp as Op>::register(context);
    <GeneralGemmPhaseLoopOp as Op>::register(context);
    <GeneralGemmGlobalTransferOp as Op>::register(context);
    <GeneralGemmLdsTransferOp as Op>::register(context);
    <GeneralGemmEpochOp as Op>::register(context);
    <GeneralGemmMfmaOp as Op>::register(context);
    <GeneralGemmEpilogueOp as Op>::register(context);

    let marker = context.aux_data.insert(Box::new(RegistrationMarker));
    context
        .aux_data_map
        .insert(GPU_REGISTRATION_KEY.clone(), marker);
    Ok(RegistrationOutcome::Registered)
}
