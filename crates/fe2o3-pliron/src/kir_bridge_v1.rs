//! Typed bridge between canonical Kernel IR and Pliron.
//!
//! This bridge is deliberately not a textual import path.  It constructs a
//! live operation graph and extracts either an exact O0 replay or a rewritten
//! Kernel IR module from that graph. Bridge receipts bind structural identity;
//! they do not by themselves prove semantic preservation of optimization.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt,
    num::NonZero,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[cfg(feature = "internal-test-context-access")]
use dialect_gpu::CanonicalKirSafetyOpInterface;
use dialect_gpu::{
    AddressSpaceAttr, AllocaOp as PlironAllocaOp, AtomicOp as PlironAtomicOp,
    CanonicalBarrierOp as PlironCanonicalBarrierOp, CanonicalFenceOp as PlironCanonicalFenceOp,
    CanonicalKirOperationAttr, CanonicalKirOperationCarrier, CanonicalKirSwitchCarrier,
    ExecutionCapabilityOp as PlironExecutionCapabilityOp,
    Gfx950LdsTransposeOp as PlironGfx950LdsTransposeOp, GuardedLoadOp as PlironGuardedLoadOp,
    GuardedStoreOp as PlironGuardedStoreOp, InlineAssemblyOp as PlironInlineAssemblyOp,
    IntegerSwitchOp as PlironIntegerSwitchOp, IntrinsicOp as PlironIntrinsicOp,
    MatrixOp as PlironMatrixOp, MemoryIntrinsicOp as PlironMemoryIntrinsicOp,
    SwitchOp as PlironSwitchOp, UnreachableOp as PlironUnreachableOp, WaveOp as PlironWaveOp,
    WorkgroupBarrierOp as PlironWorkgroupBarrierOp, WorkgroupMemoryOp as PlironWorkgroupMemoryOp,
    optimization_v1::{
        AccessModeAttr, BFloat16Attr, BFloat16Type, BinaryKindAttr, BinaryOp as PlironBinaryOp,
        BranchOp, CallOp, CastKindAttr, CastOp, CompareOp as PlironCompareOp, ComparePredicateAttr,
        CondBranchOp, ConstantOp as PlironConstantOp, GetElementPointerOp, IndexAttr, IndexType,
        LoadOp, PointerType as PlironPointerType, ReturnOp, SelectOp as PlironSelectOp,
        SliceDataOp, SliceLengthOp, SliceType as PlironSliceType, StoreOp, UnaryKindAttr,
        UnaryOp as PlironUnaryOp,
    },
    remap_canonical_operation, remap_canonical_switch,
};
use dialect_kernel::{
    CanonicalIdentityAttr, ExecutionCapabilityType as PlironExecutionCapabilityType,
    ExecutionRequirementOp, GraphContractOp, KernelContextIssueOp,
    KernelContextType as PlironKernelContextType, ReturnOp as RankedReturnOp, SourceCoordinateAttr,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, BlockId, CastKind, Constant, FunctionId, Module,
    Operation as KirOperation, OperationKind, ScalarType, TargetCapability, Terminator, Type,
    UnaryOp, ValueId, VerifiedCanonicalKernelIrV9, VerifiedCanonicalKernelIrV10,
    VerifiedCanonicalKernelIrV11, VerifiedCanonicalKernelIrV12, VerifiedCanonicalKernelIrV13,
};
use pliron::{
    attribute::{AttrObj, attr_cast},
    basic_block::BasicBlock,
    builtin::{
        attr_interfaces::TypedAttrInterface,
        attributes::StringAttr,
        attributes::{FPDoubleAttr, FPHalfAttr, FPSingleAttr, IntegerAttr},
        op_interfaces::{BranchOpInterface, SingleBlockRegionInterface},
        op_interfaces::{NOpdsInterface, NRegionsInterface, OneResultInterface},
        ops::{ConstantOp as BuiltinConstantOp, FuncOp, ModuleOp},
        type_interfaces::FunctionTypeInterface,
        types::{FP16Type, FP32Type, FP64Type, FunctionType, IntegerType, Signedness, UnitType},
    },
    common_traits::Verify,
    context::{Context, Ptr},
    derive::{pliron_attr, pliron_op, pliron_type},
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    result::Result as PlironResult,
    r#type::{Type as PlironType, TypeHandle, Typed, TypedHandle, verify_type},
    utils::{
        apfloat::{Double, Float, Half, Single},
        apint::APInt,
    },
    value::Value,
    verify_err, verify_err_noloc,
};

#[cfg(feature = "internal-test-context-access")]
use pliron::printable::Printable;

use crate::{HARD_MAX_OPERATION_TREE_ITEMS, OperationHandle, OperationHandleError, PlironSession};

// `ModuleOp::new` creates one operation containing one region and one block.
const BUILTIN_MODULE_ROOT_TREE_WORK_V1: usize = 3;

const GLOBAL_ACCESS_CONTRACT_CODEC_VERSION_V1: u8 = 1;
const MAX_GLOBAL_ACCESS_CONTRACT_BYTES_V1: usize = 67;

/// Exact logical access contract retained by the V12 bridge.
///
/// This is a typed, bounded binary codec rather than a display label. The
/// payload is decoded and validated whenever the enclosing type or operation
/// is verified.
#[pliron_attr(name = "fe2o3_bridge.global_access_contract", format = "$0")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PlironGlobalAccessContractAttr(StringAttr);

impl PlironGlobalAccessContractAttr {
    fn new(role: fe2o3_kernel_ir::GlobalCapabilityRoleV1) -> Self {
        Self(StringAttr::new(encode_global_access_contract_v1(role)))
    }

    fn role(&self) -> Option<fe2o3_kernel_ir::GlobalCapabilityRoleV1> {
        decode_global_access_contract_v1(self.0.as_str())
    }
}

impl pliron::common_traits::Verify for PlironGlobalAccessContractAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        match self.role() {
            Some(
                fe2o3_kernel_ir::GlobalCapabilityRoleV1::ReadOnly
                | fe2o3_kernel_ir::GlobalCapabilityRoleV1::ExclusiveReadWrite,
            ) => Ok(()),
            Some(fe2o3_kernel_ir::GlobalCapabilityRoleV1::DisjointWrite(contract))
                if contract.is_complete() =>
            {
                Ok(())
            }
            _ => verify_err_noloc!("global access contract is not exact canonical V1 data"),
        }
    }
}

/// A distinct logical global-memory authority. It is never represented as a
/// physical slice inside the PLIRON graph.
#[pliron_type(
    name = "fe2o3_bridge.global_capability",
    format = "`<` $element `,` $kernel_context `,` $access_contract `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PlironGlobalCapabilityType {
    element: TypeHandle,
    kernel_context: TypeHandle,
    access_contract: PlironGlobalAccessContractAttr,
}

impl PlironGlobalCapabilityType {
    fn get(
        context: &Context,
        element: TypeHandle,
        kernel_context: TypedHandle<PlironKernelContextType>,
        role: fe2o3_kernel_ir::GlobalCapabilityRoleV1,
    ) -> TypedHandle<Self> {
        Self::instantiate(
            Self {
                element,
                kernel_context: kernel_context.into(),
                access_contract: PlironGlobalAccessContractAttr::new(role),
            },
            context,
        )
    }

    fn element(&self) -> TypeHandle {
        self.element
    }

    fn kernel_context(&self) -> TypeHandle {
        self.kernel_context
    }

    fn role(&self) -> Option<fe2o3_kernel_ir::GlobalCapabilityRoleV1> {
        self.access_contract.role()
    }
}

impl pliron::common_traits::Verify for PlironGlobalCapabilityType {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_type(&*self.element.deref(context), context)?;
        verify_type(&*self.kernel_context.deref(context), context)?;
        self.access_contract.verify(context)?;
        if self
            .kernel_context
            .deref(context)
            .downcast_ref::<PlironKernelContextType>()
            .is_none()
            || global_capability_type_from_pliron(context, self).is_err()
        {
            return verify_err_noloc!("global capability type is incomplete or malformed");
        }
        Ok(())
    }
}

#[pliron_op(
    name = "fe2o3_bridge.global_capability_bind",
    format,
    interfaces = [NOpdsInterface<2>, OneResultInterface, NRegionsInterface<0>],
    operands = (context: PlironKernelContextType, physical: PlironSliceType),
    results = (capability: PlironGlobalCapabilityType)
)]
struct PlironGlobalCapabilityBindOp;

impl PlironGlobalCapabilityBindOp {
    fn new(
        context: &mut Context,
        logical_type: TypedHandle<PlironGlobalCapabilityType>,
        kernel_context: Value,
        physical: Value,
    ) -> Self {
        Self::from_operation(Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![logical_type.into()],
            vec![kernel_context, physical],
            vec![],
            0,
        ))
    }
}

impl pliron::common_traits::Verify for PlironGlobalCapabilityBindOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_bridge_operation_shape(self, context, 2, 1, 0)?;
        let raw = self.get_operation().deref(context);
        let result_type = raw.get_type(0);
        let result = result_type.deref(context);
        let Some(capability) = result.downcast_ref::<PlironGlobalCapabilityType>() else {
            return verify_err!(
                self.loc(context),
                "global capability bind result is not logical authority"
            );
        };
        if raw.get_operand(0).get_type(context) != capability.kernel_context() {
            return verify_err!(
                self.loc(context),
                "global capability bind changed its kernel context"
            );
        }
        let Some(role) = capability.role() else {
            return verify_err!(
                self.loc(context),
                "global capability bind has no exact access role"
            );
        };
        let expected_physical = PlironSliceType::get(
            context,
            capability.element(),
            AddressSpaceAttr::Global,
            access_mode_to_pliron(role.access()),
        );
        if raw.get_operand(1).get_type(context) != expected_physical.into() {
            return verify_err!(
                self.loc(context),
                "global capability bind changed its physical slice contract"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "fe2o3_bridge.global_capability_index",
    format,
    interfaces = [NOpdsInterface<2>, OneResultInterface, NRegionsInterface<0>],
    operands = (capability: PlironGlobalCapabilityType, index: IndexType),
    results = (projected_index: IndexType),
    attributes = (global_access_contract: PlironGlobalAccessContractAttr)
)]
struct PlironGlobalCapabilityIndexOp;

impl PlironGlobalCapabilityIndexOp {
    fn new(
        context: &mut Context,
        capability: Value,
        index: Value,
        role: fe2o3_kernel_ir::GlobalCapabilityRoleV1,
    ) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![IndexType::get(context).into()],
            vec![capability, index],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_global_access_contract(context, PlironGlobalAccessContractAttr::new(role));
        op
    }

    fn role(&self, context: &Context) -> Option<fe2o3_kernel_ir::GlobalCapabilityRoleV1> {
        self.get_attr_global_access_contract(context)?.role()
    }
}

impl pliron::common_traits::Verify for PlironGlobalCapabilityIndexOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_bridge_operation_shape(self, context, 2, 1, 1)?;
        let raw = self.get_operation().deref(context);
        let capability_type = raw.get_operand(0).get_type(context);
        let capability_type = capability_type.deref(context);
        let Some(capability) = capability_type.downcast_ref::<PlironGlobalCapabilityType>() else {
            return verify_err!(
                self.loc(context),
                "global capability index operand is not logical authority"
            );
        };
        if self.role(context).is_none() || self.role(context) != capability.role() {
            return verify_err!(
                self.loc(context),
                "global capability index substituted its access contract"
            );
        }
        if raw.get_operand(1).get_type(context) != IndexType::get(context).into()
            || raw.get_type(0) != IndexType::get(context).into()
        {
            return verify_err!(
                self.loc(context),
                "global capability index requires exact index types"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "fe2o3_bridge.global_capability_length",
    format,
    interfaces = [NOpdsInterface<1>, OneResultInterface, NRegionsInterface<0>],
    operands = (capability: PlironGlobalCapabilityType),
    results = (length: IndexType)
)]
struct PlironGlobalCapabilityLengthOp;

impl PlironGlobalCapabilityLengthOp {
    fn new(context: &mut Context, capability: Value) -> Self {
        Self::from_operation(Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![IndexType::get(context).into()],
            vec![capability],
            vec![],
            0,
        ))
    }
}

impl pliron::common_traits::Verify for PlironGlobalCapabilityLengthOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_bridge_operation_shape(self, context, 1, 1, 0)?;
        if self.get_operation().deref(context).get_type(0) != IndexType::get(context).into() {
            return verify_err!(
                self.loc(context),
                "global capability length must produce an index"
            );
        }
        Ok(())
    }
}

#[pliron_op(
    name = "fe2o3_bridge.global_capability_data",
    format,
    interfaces = [NOpdsInterface<1>, OneResultInterface, NRegionsInterface<0>],
    operands = (capability: PlironGlobalCapabilityType)
)]
struct PlironGlobalCapabilityDataOp;

impl PlironGlobalCapabilityDataOp {
    fn new(context: &mut Context, capability: Value) -> Option<Self> {
        let capability_type = capability.get_type(context);
        let (element, role) = {
            let capability_type = capability_type.deref(context);
            let capability_type = capability_type.downcast_ref::<PlironGlobalCapabilityType>()?;
            (capability_type.element(), capability_type.role()?)
        };
        let pointer = PlironPointerType::get(
            context,
            element,
            AddressSpaceAttr::Global,
            access_mode_to_pliron(role.access()),
        );
        Some(Self::from_operation(Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![pointer.into()],
            vec![capability],
            vec![],
            0,
        )))
    }
}

impl pliron::common_traits::Verify for PlironGlobalCapabilityDataOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_bridge_operation_shape(self, context, 1, 1, 0)?;
        let raw = self.get_operation().deref(context);
        let capability_type = raw.get_operand(0).get_type(context);
        let capability_type = capability_type.deref(context);
        let Some(capability) = capability_type.downcast_ref::<PlironGlobalCapabilityType>() else {
            return verify_err!(
                self.loc(context),
                "global capability data operand is not logical authority"
            );
        };
        let Some(role) = capability.role() else {
            return verify_err!(
                self.loc(context),
                "global capability data has no exact access role"
            );
        };
        let expected = PlironPointerType::get(
            context,
            capability.element(),
            AddressSpaceAttr::Global,
            access_mode_to_pliron(role.access()),
        );
        if raw.get_type(0) != expected.into() {
            return verify_err!(
                self.loc(context),
                "global capability data changed its pointer contract"
            );
        }
        Ok(())
    }
}

fn verify_bridge_operation_shape(
    op: &dyn Op,
    context: &Context,
    operands: usize,
    results: usize,
    attributes: usize,
) -> PlironResult<()> {
    let raw = op.get_operation().deref(context);
    if raw.get_num_operands() != operands
        || raw.get_num_results() != results
        || raw.get_num_successors() != 0
        || raw.num_regions() != 0
        || raw.attributes.0.len() != attributes
    {
        return verify_err!(
            op.loc(context),
            "{} has malformed bridge payload",
            op.get_opid()
        );
    }
    Ok(())
}

fn encode_global_access_contract_v1(role: fe2o3_kernel_ir::GlobalCapabilityRoleV1) -> String {
    use fe2o3_kernel_ir::GlobalCapabilityRoleV1;

    let mut bytes = Vec::with_capacity(MAX_GLOBAL_ACCESS_CONTRACT_BYTES_V1);
    bytes.push(GLOBAL_ACCESS_CONTRACT_CODEC_VERSION_V1);
    match role {
        GlobalCapabilityRoleV1::ReadOnly => bytes.push(0),
        GlobalCapabilityRoleV1::DisjointWrite(contract) => {
            bytes.push(1);
            bytes.extend_from_slice(&contract.nominal_identity());
            encode_global_index_space_v1(&mut bytes, contract.mapping());
        }
        GlobalCapabilityRoleV1::ExclusiveReadWrite => bytes.push(2),
    }
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        write!(&mut text, "{byte:02x}").expect("writing to String cannot fail");
    }
    text
}

fn encode_global_index_space_v1(
    bytes: &mut Vec<u8>,
    mapping: fe2o3_kernel_ir::GlobalDisjointIndexSpaceV1,
) {
    use fe2o3_kernel_ir::GlobalDisjointIndexSpaceV1;

    match mapping {
        GlobalDisjointIndexSpaceV1::Index1d => bytes.push(0),
        GlobalDisjointIndexSpaceV1::ShiftedIndex1d { offset } => {
            bytes.push(1);
            bytes.extend_from_slice(&offset.to_le_bytes());
        }
        GlobalDisjointIndexSpaceV1::BlockedIndex1d {
            lanes_per_block,
            elements_per_lane,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&lanes_per_block.to_le_bytes());
            bytes.extend_from_slice(&elements_per_lane.to_le_bytes());
        }
        GlobalDisjointIndexSpaceV1::Tiled2dIndex1d {
            lanes_per_tile,
            tile_rows,
            tile_columns,
            elements_per_lane,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&lanes_per_tile.to_le_bytes());
            bytes.extend_from_slice(&tile_rows.to_le_bytes());
            bytes.extend_from_slice(&tile_columns.to_le_bytes());
            bytes.extend_from_slice(&elements_per_lane.to_le_bytes());
        }
        GlobalDisjointIndexSpaceV1::RowStriped2dIndex1d {
            lanes_per_row,
            elements_per_lane,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&lanes_per_row.to_le_bytes());
            bytes.extend_from_slice(&elements_per_lane.to_le_bytes());
        }
        GlobalDisjointIndexSpaceV1::GridExclusive => bytes.push(5),
    }
}

fn decode_global_access_contract_v1(text: &str) -> Option<fe2o3_kernel_ir::GlobalCapabilityRoleV1> {
    use fe2o3_kernel_ir::{
        GlobalCapabilityRoleV1, GlobalDisjointIndexContractV1, GlobalDisjointIndexSpaceV1,
    };

    let bytes = decode_bounded_hex_v1(text, MAX_GLOBAL_ACCESS_CONTRACT_BYTES_V1)?;
    let mut cursor = GlobalAccessContractCursorV1::new(&bytes);
    if cursor.byte()? != GLOBAL_ACCESS_CONTRACT_CODEC_VERSION_V1 {
        return None;
    }
    let role = match cursor.byte()? {
        0 => GlobalCapabilityRoleV1::ReadOnly,
        1 => {
            let nominal_identity = cursor.array_32()?;
            let mapping = match cursor.byte()? {
                0 => GlobalDisjointIndexSpaceV1::Index1d,
                1 => GlobalDisjointIndexSpaceV1::ShiftedIndex1d {
                    offset: cursor.u64()?,
                },
                2 => GlobalDisjointIndexSpaceV1::BlockedIndex1d {
                    lanes_per_block: cursor.u64()?,
                    elements_per_lane: cursor.u64()?,
                },
                3 => GlobalDisjointIndexSpaceV1::Tiled2dIndex1d {
                    lanes_per_tile: cursor.u64()?,
                    tile_rows: cursor.u64()?,
                    tile_columns: cursor.u64()?,
                    elements_per_lane: cursor.u64()?,
                },
                4 => GlobalDisjointIndexSpaceV1::RowStriped2dIndex1d {
                    lanes_per_row: cursor.u64()?,
                    elements_per_lane: cursor.u64()?,
                },
                5 => GlobalDisjointIndexSpaceV1::GridExclusive,
                _ => return None,
            };
            GlobalCapabilityRoleV1::DisjointWrite(GlobalDisjointIndexContractV1::new(
                nominal_identity,
                mapping,
            ))
        }
        2 => GlobalCapabilityRoleV1::ExclusiveReadWrite,
        _ => return None,
    };
    cursor.is_exhausted().then_some(role)
}

fn decode_bounded_hex_v1(text: &str, max_bytes: usize) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(2) || bytes.len() > max_bytes.checked_mul(2)? {
        return None;
    }
    bytes
        .chunks_exact(2)
        .map(|pair| Some((hex_nibble_v1(pair[0])? << 4) | hex_nibble_v1(pair[1])?))
        .collect()
}

const fn hex_nibble_v1(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

struct GlobalAccessContractCursorV1<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> GlobalAccessContractCursorV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn byte(&mut self) -> Option<u8> {
        let byte = *self.bytes.get(self.position)?;
        self.position += 1;
        Some(byte)
    }

    fn u64(&mut self) -> Option<u64> {
        let end = self.position.checked_add(8)?;
        let bytes: [u8; 8] = self.bytes.get(self.position..end)?.try_into().ok()?;
        self.position = end;
        Some(u64::from_le_bytes(bytes))
    }

    fn array_32(&mut self) -> Option<[u8; 32]> {
        let end = self.position.checked_add(32)?;
        let bytes = self.bytes.get(self.position..end)?.try_into().ok()?;
        self.position = end;
        Some(bytes)
    }

    const fn is_exhausted(&self) -> bool {
        self.position == self.bytes.len()
    }
}

/// Domain separator for identities of exact canonical Kernel IR at this bridge.
pub const KIR_PLIRON_BRIDGE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/CANONICAL-KIR-V9/V1\0";

/// Domain separator for V10 endpoint identities, including memory intrinsics.
pub const KIR_PLIRON_BRIDGE_V10_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/CANONICAL-KIR-V10/V1\0";
pub const KIR_PLIRON_BRIDGE_V11_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/CANONICAL-KIR-V11/V1\0";
pub const KIR_PLIRON_BRIDGE_V12_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/CANONICAL-KIR-V12/V1\0";
pub const KIR_PLIRON_BRIDGE_V13_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/CANONICAL-KIR-V13/V1\0";

const KIR_PLIRON_CONTEXT_OPERATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/KERNEL-CONTEXT-OPERATION/V1\0";
const KIR_PLIRON_EXECUTION_OPERATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/EXECUTION-CAPABILITY-OPERATION/V1\0";
const KIR_PLIRON_EXECUTION_CONTRACT_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/EXECUTION-CAPABILITY-CONTRACT/V1\0";

/// Domain separator for the canonical bridge-correspondence transcript.
pub const KIR_PLIRON_BRIDGE_CORRESPONDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/CORRESPONDENCE/V1\0";

/// Stable identity of one canonical Kernel IR endpoint of the bridge.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KirBridgeDigestV1 {
    digest: [u8; 32],
    canonical_bytes: u64,
}

impl KirBridgeDigestV1 {
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub const fn canonical_bytes(self) -> u64 {
        self.canonical_bytes
    }
}

/// Source coordinate associated with one typed Pliron node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KirBridgeCoordinateV1 {
    Function {
        function: u32,
    },
    Block {
        function: u32,
        block: u32,
    },
    Operation {
        function: u32,
        block: u32,
        operation: u32,
    },
    Terminator {
        function: u32,
        block: u32,
    },
}

/// Opaque ordinal-to-Kernel-IR correspondence for one live graph node.
///
/// `pliron_ordinal` is a deterministic preorder number, not a pointer, arena
/// index, or authority to mutate the session.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KirBridgeCorrespondenceV1 {
    pliron_ordinal: u64,
    coordinate: KirBridgeCoordinateV1,
}

impl KirBridgeCorrespondenceV1 {
    pub const fn pliron_ordinal(self) -> u64 {
        self.pliron_ordinal
    }

    pub const fn coordinate(self) -> KirBridgeCoordinateV1 {
        self.coordinate
    }
}

/// Stable identity of the complete ordered Pliron-to-Kernel-IR correspondence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KirBridgeCorrespondenceDigestV1 {
    digest: [u8; 32],
    count: u64,
}

impl KirBridgeCorrespondenceDigestV1 {
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub const fn count(self) -> u64 {
        self.count
    }
}

/// Stable identity of one execution operation and its exact logical result
/// types in a verified final V13 graph.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KirBridgeExecutionCapabilityIdentityV1 {
    operation: [u8; 32],
    source_operation: [u8; 32],
    contract: [u8; 32],
    result_types: Box<[[u8; 32]]>,
    coordinate: KirBridgeCoordinateV1,
}

impl KirBridgeExecutionCapabilityIdentityV1 {
    pub const fn operation(&self) -> [u8; 32] {
        self.operation
    }

    pub const fn source_operation(&self) -> [u8; 32] {
        self.source_operation
    }

    pub const fn contract(&self) -> [u8; 32] {
        self.contract
    }

    pub fn result_types(&self) -> &[[u8; 32]] {
        &self.result_types
    }

    pub const fn coordinate(&self) -> KirBridgeCoordinateV1 {
        self.coordinate
    }
}

/// Exact final-graph epoch and execution vocabulary visible to W4 analyses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KirBridgeExecutionCapabilityWitnessV1 {
    final_graph_epoch: [u8; 32],
    operations: Box<[KirBridgeExecutionCapabilityIdentityV1]>,
}

impl KirBridgeExecutionCapabilityWitnessV1 {
    pub const fn final_graph_epoch(&self) -> [u8; 32] {
        self.final_graph_epoch
    }

    pub fn operations(&self) -> &[KirBridgeExecutionCapabilityIdentityV1] {
        &self.operations
    }

    pub fn preservation_from(&self, input: &Self) -> KirBridgeExecutionCapabilityPreservationV1 {
        if self == input {
            return KirBridgeExecutionCapabilityPreservationV1::ExactGraph;
        }
        let mut before = input.operations.to_vec();
        let mut after = self.operations.to_vec();
        before.sort_by_key(|record| record.source_operation);
        after.sort_by_key(|record| record.source_operation);
        for records in [&mut before, &mut after] {
            for record in records {
                record.coordinate = KirBridgeCoordinateV1::Operation {
                    function: 0,
                    block: 0,
                    operation: 0,
                };
            }
        }
        if before == after {
            KirBridgeExecutionCapabilityPreservationV1::ContractsPreservedGraphChanged
        } else {
            KirBridgeExecutionCapabilityPreservationV1::Invalidated
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KirBridgeExecutionCapabilityPreservationV1 {
    ExactGraph,
    ContractsPreservedGraphChanged,
    Invalidated,
}

/// Packages the exact final V13 execution graph for downstream analysis.
pub fn kir_v13_execution_capability_witness_v1(
    final_graph: &VerifiedCanonicalKernelIrV13,
) -> Result<KirBridgeExecutionCapabilityWitnessV1, KirBridgeErrorV1> {
    final_graph
        .revalidate()
        .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
    let module = fe2o3_kernel_ir::decode_module_v13(final_graph.canonical_bytes())
        .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
    let final_graph_epoch = digest(
        final_graph.canonical_bytes(),
        KirBridgeCanonicalVersionV1::V13,
    )?
    .digest;
    let mut operations = Vec::new();
    for (function_index, function) in module.functions.iter().enumerate() {
        let Some(body) = &function.body else {
            continue;
        };
        for (block_index, block) in body.blocks.iter().enumerate() {
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let OperationKind::ExecutionCapability(contract) = &operation.kind else {
                    continue;
                };
                let contract_bytes =
                    fe2o3_kernel_ir::encode_execution_capability_contract_v1(contract)
                        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
                let result_types = operation
                    .results
                    .iter()
                    .filter_map(|result| match &result.ty {
                        Type::ExecutionCapability(capability) => Some(capability),
                        _ => None,
                    })
                    .map(execution_type_contract_identity)
                    .collect::<Result<Vec<_>, _>>()?;
                operations.push(KirBridgeExecutionCapabilityIdentityV1 {
                    operation: execution_operation_identity(contract)?,
                    source_operation: contract.source.operation,
                    contract: identity_for_bytes(
                        KIR_PLIRON_EXECUTION_CONTRACT_IDENTITY_DOMAIN_V1,
                        &contract_bytes,
                    )?,
                    result_types: result_types.into_boxed_slice(),
                    coordinate: KirBridgeCoordinateV1::Operation {
                        function: to_u32(function_index)?,
                        block: to_u32(block_index)?,
                        operation: to_u32(operation_index)?,
                    },
                });
            }
        }
    }
    Ok(KirBridgeExecutionCapabilityWitnessV1 {
        final_graph_epoch,
        operations: operations.into_boxed_slice(),
    })
}

/// Exact O0 import/extraction result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KirBridgeRoundTripReportV1 {
    input: KirBridgeDigestV1,
    output: KirBridgeDigestV1,
    correspondence: Vec<KirBridgeCorrespondenceV1>,
}

/// Receipt for a verified canonical module extracted after live graph rewrites.
///
/// This binds exact before/after bytes and current structural correspondence.
/// It does not, by itself, prove that the rewrite preserved semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KirBridgeOptimizedReceiptV1 {
    input: KirBridgeDigestV1,
    output: KirBridgeDigestV1,
    correspondence: Vec<KirBridgeCorrespondenceV1>,
}

impl KirBridgeOptimizedReceiptV1 {
    pub const fn input(&self) -> KirBridgeDigestV1 {
        self.input
    }

    pub const fn output(&self) -> KirBridgeDigestV1 {
        self.output
    }

    pub fn correspondence(&self) -> &[KirBridgeCorrespondenceV1] {
        &self.correspondence
    }

    pub fn correspondence_digest(&self) -> KirBridgeCorrespondenceDigestV1 {
        correspondence_digest_v1(&self.correspondence)
    }

    pub fn changed(&self) -> bool {
        self.input != self.output
    }
}

impl KirBridgeRoundTripReportV1 {
    pub const fn input(&self) -> KirBridgeDigestV1 {
        self.input
    }

    pub const fn output(&self) -> KirBridgeDigestV1 {
        self.output
    }

    pub fn correspondence(&self) -> &[KirBridgeCorrespondenceV1] {
        &self.correspondence
    }

    pub fn correspondence_digest(&self) -> KirBridgeCorrespondenceDigestV1 {
        correspondence_digest_v1(&self.correspondence)
    }

    pub fn is_exact(&self) -> bool {
        self.input == self.output
    }
}

fn correspondence_digest_v1(
    correspondence: &[KirBridgeCorrespondenceV1],
) -> KirBridgeCorrespondenceDigestV1 {
    use sha2::{Digest, Sha256};

    // Import preflight bounds correspondence well below u64::MAX.
    let count = u64::try_from(correspondence.len())
        .expect("bridge correspondence count is bounded by the operation-tree limit");
    let mut hasher = Sha256::new();
    hasher.update(KIR_PLIRON_BRIDGE_CORRESPONDENCE_DOMAIN_V1);
    hasher.update(count.to_le_bytes());
    for record in correspondence {
        hasher.update(record.pliron_ordinal.to_le_bytes());
        match record.coordinate {
            KirBridgeCoordinateV1::Function { function } => {
                hasher.update([1]);
                hasher.update(function.to_le_bytes());
            }
            KirBridgeCoordinateV1::Block { function, block } => {
                hasher.update([2]);
                hasher.update(function.to_le_bytes());
                hasher.update(block.to_le_bytes());
            }
            KirBridgeCoordinateV1::Operation {
                function,
                block,
                operation,
            } => {
                hasher.update([3]);
                hasher.update(function.to_le_bytes());
                hasher.update(block.to_le_bytes());
                hasher.update(operation.to_le_bytes());
            }
            KirBridgeCoordinateV1::Terminator { function, block } => {
                hasher.update([4]);
                hasher.update(function.to_le_bytes());
                hasher.update(block.to_le_bytes());
            }
        }
    }
    KirBridgeCorrespondenceDigestV1 {
        digest: hasher.finalize().into(),
        count,
    }
}

/// Owner token for a typed Kernel IR graph held by one [`PlironSession`].
///
/// The metadata snapshot contains only facts that builtin Pliron containers do
/// not represent (kernel declarations, roles, and capability declarations).
/// Function bodies are always extracted from the live graph.
#[derive(Clone)]
pub struct KirPlironGraphV1 {
    root: OperationHandle,
    metadata: Module,
    input: KirBridgeDigestV1,
    canonical_version: KirBridgeCanonicalVersionV1,
    correspondence: Vec<KirBridgeCorrespondenceV1>,
    origins: KirBridgeOriginsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KirBridgeCanonicalVersionV1 {
    V9,
    V10,
    V11,
    V12,
    V13,
}

#[derive(Clone, Default)]
struct KirBridgeOriginsV1 {
    functions: HashMap<Ptr<Operation>, usize>,
    blocks: HashMap<Ptr<BasicBlock>, (usize, BlockId)>,
    values: HashMap<Value, ValueId>,
    logical_types: HashMap<Value, Type>,
}

impl KirPlironGraphV1 {
    pub const fn root(&self) -> &OperationHandle {
        &self.root
    }

    pub const fn input(&self) -> KirBridgeDigestV1 {
        self.input
    }

    pub fn correspondence(&self) -> &[KirBridgeCorrespondenceV1] {
        &self.correspondence
    }
}

/// Why a canonical Kernel IR module could not cross the typed bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KirBridgeErrorV1 {
    CanonicalInputRejected,
    Session(OperationHandleError),
    SizeOverflow,
    UnsupportedType,
    UnsupportedOperation { coordinate: KirBridgeCoordinateV1 },
    UnsupportedTerminator { coordinate: KirBridgeCoordinateV1 },
    MissingFunctionBody { function: u32 },
    MissingValue { function: u32, value: u32 },
    MissingBlock { function: u32, block: u32 },
    MalformedGraph,
    GraphIdentityMismatch,
    NonExactRoundTrip,
    UpstreamPanicked,
}

impl fmt::Display for KirBridgeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CanonicalInputRejected => {
                formatter.write_str("canonical Kernel IR input failed revalidation")
            }
            Self::Session(error) => {
                write!(formatter, "Pliron session rejected bridge graph: {error}")
            }
            Self::SizeOverflow => formatter.write_str("bridge graph size exceeds its fixed bounds"),
            Self::UnsupportedType => {
                formatter.write_str("Kernel IR type is unsupported by the bridge")
            }
            Self::UnsupportedOperation { coordinate } => {
                write!(
                    formatter,
                    "unsupported Kernel IR operation at {coordinate:?}"
                )
            }
            Self::UnsupportedTerminator { coordinate } => {
                write!(
                    formatter,
                    "unsupported Kernel IR terminator at {coordinate:?}"
                )
            }
            Self::MissingFunctionBody { function } => {
                write!(
                    formatter,
                    "defined Kernel IR function {function} has no body"
                )
            }
            Self::MissingValue { function, value } => write!(
                formatter,
                "Kernel IR value %{value} is unavailable in function {function}"
            ),
            Self::MissingBlock { function, block } => write!(
                formatter,
                "Kernel IR block bb{block} is unavailable in function {function}"
            ),
            Self::MalformedGraph => formatter.write_str("typed Pliron graph is malformed"),
            Self::GraphIdentityMismatch => {
                formatter.write_str("typed Pliron graph does not belong to this bridge owner")
            }
            Self::NonExactRoundTrip => {
                formatter.write_str("O0 Pliron extraction changed canonical Kernel IR")
            }
            Self::UpstreamPanicked => {
                formatter.write_str("Pliron panicked while bridging Kernel IR")
            }
        }
    }
}

impl Error for KirBridgeErrorV1 {}

impl From<OperationHandleError> for KirBridgeErrorV1 {
    fn from(error: OperationHandleError) -> Self {
        Self::Session(error)
    }
}

impl PlironSession {
    /// Constructs a typed live Pliron graph from verified canonical Kernel IR.
    pub fn import_canonical_kir_v9_o0(
        &mut self,
        input: &VerifiedCanonicalKernelIrV9,
    ) -> Result<KirPlironGraphV1, KirBridgeErrorV1> {
        input
            .revalidate()
            .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        let module = fe2o3_kernel_ir::decode_module_v9(input.canonical_bytes())
            .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        import_module(
            self,
            input.canonical_bytes(),
            KirBridgeCanonicalVersionV1::V9,
            module,
        )
    }

    /// Constructs a typed live Pliron graph from verified canonical KIR V10.
    pub fn import_canonical_kir_v10_o0(
        &mut self,
        input: &VerifiedCanonicalKernelIrV10,
    ) -> Result<KirPlironGraphV1, KirBridgeErrorV1> {
        input
            .revalidate()
            .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        let module = fe2o3_kernel_ir::decode_module_v10(input.canonical_bytes())
            .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        import_module(
            self,
            input.canonical_bytes(),
            KirBridgeCanonicalVersionV1::V10,
            module,
        )
    }

    /// Constructs a typed live Pliron graph from verified canonical KIR V11.
    pub fn import_canonical_kir_v11_o0(
        &mut self,
        input: &VerifiedCanonicalKernelIrV11,
    ) -> Result<KirPlironGraphV1, KirBridgeErrorV1> {
        input
            .revalidate()
            .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        let module = fe2o3_kernel_ir::decode_module_v11(input.canonical_bytes())
            .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        import_module(
            self,
            input.canonical_bytes(),
            KirBridgeCanonicalVersionV1::V11,
            module,
        )
    }

    /// Constructs the one typed live graph from verified canonical KIR V12.
    pub fn import_canonical_kir_v12_o0(
        &mut self,
        input: &VerifiedCanonicalKernelIrV12,
    ) -> Result<KirPlironGraphV1, KirBridgeErrorV1> {
        input
            .revalidate()
            .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        let module = fe2o3_kernel_ir::decode_module_v12(input.canonical_bytes())
            .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        import_module(
            self,
            input.canonical_bytes(),
            KirBridgeCanonicalVersionV1::V12,
            module,
        )
    }

    /// Constructs the exact typed live graph from verified canonical KIR V13.
    pub fn import_canonical_kir_v13_o0(
        &mut self,
        input: &VerifiedCanonicalKernelIrV13,
    ) -> Result<KirPlironGraphV1, KirBridgeErrorV1> {
        input
            .revalidate()
            .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        let module = fe2o3_kernel_ir::decode_module_v13(input.canonical_bytes())
            .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        import_module(
            self,
            input.canonical_bytes(),
            KirBridgeCanonicalVersionV1::V13,
            module,
        )
    }

    /// Extracts canonical Kernel IR from a typed live graph and requires an
    /// exact O0 round trip.
    pub fn extract_canonical_kir_v9_o0(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV9, KirBridgeRoundTripReportV1), KirBridgeErrorV1> {
        if graph.canonical_version != KirBridgeCanonicalVersionV1::V9 {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let output = extract_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV9::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), KirBridgeCanonicalVersionV1::V9)?;
        if output_digest != graph.input {
            return Err(KirBridgeErrorV1::NonExactRoundTrip);
        }
        let report = KirBridgeRoundTripReportV1 {
            input: graph.input,
            output: output_digest,
            correspondence: graph.correspondence.clone(),
        };
        Ok((output, report))
    }

    /// Extracts the current supported live graph into verified canonical V9.
    ///
    /// Unlike [`Self::extract_canonical_kir_v9_o0`], this permits structural
    /// changes and deterministically assigns fresh IDs to operation results.
    /// The receipt is transformation replay evidence, not a semantic proof.
    pub fn extract_optimized_canonical_kir_v9_v1(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV9, KirBridgeOptimizedReceiptV1), KirBridgeErrorV1> {
        if graph.canonical_version != KirBridgeCanonicalVersionV1::V9 {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let (output, correspondence) = extract_optimized_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV9::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), KirBridgeCanonicalVersionV1::V9)?;
        let receipt = KirBridgeOptimizedReceiptV1 {
            input: graph.input,
            output: output_digest,
            correspondence,
        };
        Ok((output, receipt))
    }

    /// Extracts KIR V10 from a typed graph and requires an exact O0 replay.
    pub fn extract_canonical_kir_v10_o0(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV10, KirBridgeRoundTripReportV1), KirBridgeErrorV1> {
        if graph.canonical_version != KirBridgeCanonicalVersionV1::V10 {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let output = extract_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV10::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), KirBridgeCanonicalVersionV1::V10)?;
        if output_digest != graph.input {
            return Err(KirBridgeErrorV1::NonExactRoundTrip);
        }
        let report = KirBridgeRoundTripReportV1 {
            input: graph.input,
            output: output_digest,
            correspondence: graph.correspondence.clone(),
        };
        Ok((output, report))
    }

    /// Extracts the current supported live graph into verified canonical V10.
    pub fn extract_optimized_canonical_kir_v10_v1(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV10, KirBridgeOptimizedReceiptV1), KirBridgeErrorV1> {
        if graph.canonical_version != KirBridgeCanonicalVersionV1::V10 {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let (output, correspondence) = extract_optimized_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV10::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), KirBridgeCanonicalVersionV1::V10)?;
        let receipt = KirBridgeOptimizedReceiptV1 {
            input: graph.input,
            output: output_digest,
            correspondence,
        };
        Ok((output, receipt))
    }

    /// Extracts KIR V11 from a typed graph and requires an exact O0 replay.
    pub fn extract_canonical_kir_v11_o0(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV11, KirBridgeRoundTripReportV1), KirBridgeErrorV1> {
        if graph.canonical_version != KirBridgeCanonicalVersionV1::V11 {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let output = extract_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV11::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), KirBridgeCanonicalVersionV1::V11)?;
        if output_digest != graph.input {
            return Err(KirBridgeErrorV1::NonExactRoundTrip);
        }
        let report = KirBridgeRoundTripReportV1 {
            input: graph.input,
            output: output_digest,
            correspondence: graph.correspondence.clone(),
        };
        Ok((output, report))
    }

    /// Extracts the current supported live graph into verified canonical V11.
    pub fn extract_optimized_canonical_kir_v11_v1(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV11, KirBridgeOptimizedReceiptV1), KirBridgeErrorV1> {
        if graph.canonical_version != KirBridgeCanonicalVersionV1::V11 {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let (output, correspondence) = extract_optimized_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV11::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), KirBridgeCanonicalVersionV1::V11)?;
        let receipt = KirBridgeOptimizedReceiptV1 {
            input: graph.input,
            output: output_digest,
            correspondence,
        };
        Ok((output, receipt))
    }

    /// Extracts KIR V12 and requires exact context, requirement, and epoch replay.
    pub fn extract_canonical_kir_v12_o0(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV12, KirBridgeRoundTripReportV1), KirBridgeErrorV1> {
        if graph.canonical_version != KirBridgeCanonicalVersionV1::V12 {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let output = extract_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV12::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), KirBridgeCanonicalVersionV1::V12)?;
        if output_digest != graph.input {
            return Err(KirBridgeErrorV1::NonExactRoundTrip);
        }
        let report = KirBridgeRoundTripReportV1 {
            input: graph.input,
            output: output_digest,
            correspondence: graph.correspondence.clone(),
        };
        Ok((output, report))
    }

    /// Extracts the current live V12 graph after checked rewrites.
    pub fn extract_optimized_canonical_kir_v12_v1(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV12, KirBridgeOptimizedReceiptV1), KirBridgeErrorV1> {
        if graph.canonical_version != KirBridgeCanonicalVersionV1::V12 {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let (output, correspondence) = extract_optimized_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV12::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), KirBridgeCanonicalVersionV1::V12)?;
        let receipt = KirBridgeOptimizedReceiptV1 {
            input: graph.input,
            output: output_digest,
            correspondence,
        };
        Ok((output, receipt))
    }

    /// Extracts KIR V13 and requires exact execution-operation graph replay.
    pub fn extract_canonical_kir_v13_o0(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV13, KirBridgeRoundTripReportV1), KirBridgeErrorV1> {
        if graph.canonical_version != KirBridgeCanonicalVersionV1::V13 {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let output = extract_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV13::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), KirBridgeCanonicalVersionV1::V13)?;
        if output_digest != graph.input {
            return Err(KirBridgeErrorV1::NonExactRoundTrip);
        }
        let report = KirBridgeRoundTripReportV1 {
            input: graph.input,
            output: output_digest,
            correspondence: graph.correspondence.clone(),
        };
        Ok((output, report))
    }

    /// Extracts the current live V13 graph after checked rewrites.
    pub fn extract_optimized_canonical_kir_v13_v1(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV13, KirBridgeOptimizedReceiptV1), KirBridgeErrorV1> {
        if graph.canonical_version != KirBridgeCanonicalVersionV1::V13 {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let (output, correspondence) = extract_optimized_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV13::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), KirBridgeCanonicalVersionV1::V13)?;
        let receipt = KirBridgeOptimizedReceiptV1 {
            input: graph.input,
            output: output_digest,
            correspondence,
        };
        Ok((output, receipt))
    }

    pub(crate) fn with_canonical_kir_v13_functions<T, E>(
        &mut self,
        graph: &KirPlironGraphV1,
        action: impl FnOnce(&Context, &[FuncOp]) -> Result<T, E>,
    ) -> Result<Result<T, E>, KirBridgeErrorV1> {
        self.with_canonical_functions(graph, KirBridgeCanonicalVersionV1::V13, action)
    }

    pub(crate) fn with_canonical_kir_v12_functions<T, E>(
        &mut self,
        graph: &KirPlironGraphV1,
        action: impl FnOnce(&Context, &[FuncOp]) -> Result<T, E>,
    ) -> Result<Result<T, E>, KirBridgeErrorV1> {
        if !matches!(
            graph.canonical_version,
            KirBridgeCanonicalVersionV1::V12 | KirBridgeCanonicalVersionV1::V13
        ) {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        self.with_operation(graph.root(), |root, context| {
            if !Operation::is_op::<ModuleOp>(root, context)
                || root.deref(context).num_regions() != 1
            {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            let region = root.deref(context).get_region(0);
            let blocks = region.deref(context).iter(context).collect::<Vec<_>>();
            let [block] = blocks.as_slice() else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            let live = validated_live_functions(
                context,
                *block,
                &graph.metadata,
                graph.canonical_version,
                graph.input.digest,
            )?;
            let mut functions = Vec::with_capacity(live.len());
            for function_index in graph
                .metadata
                .functions
                .iter()
                .enumerate()
                .filter_map(|(index, function)| function.body.as_ref().map(|_| index))
            {
                let pointer = live
                    .iter()
                    .copied()
                    .find(|candidate| {
                        graph.origins.functions.get(candidate) == Some(&function_index)
                    })
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?;
                functions.push(
                    Operation::get_op::<FuncOp>(pointer, context)
                        .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                );
            }
            if functions.len() != live.len() {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            Ok(action(context, &functions))
        })?
    }

    fn with_canonical_functions<T, E>(
        &mut self,
        graph: &KirPlironGraphV1,
        expected_version: KirBridgeCanonicalVersionV1,
        action: impl FnOnce(&Context, &[FuncOp]) -> Result<T, E>,
    ) -> Result<Result<T, E>, KirBridgeErrorV1> {
        if graph.canonical_version != expected_version {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        self.with_canonical_kir_v12_functions(graph, action)
    }

    /// Serializes a live canonical graph for cross-context conformance tests.
    #[doc(hidden)]
    #[cfg(feature = "internal-test-context-access")]
    pub fn canonical_kir_text_for_test(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<String, KirBridgeErrorV1> {
        self.with_operation(graph.root(), |root, context| root.disp(context).to_string())
            .map_err(Into::into)
    }

    /// Inventories safety-significant operations through their typed interface.
    #[doc(hidden)]
    #[cfg(feature = "internal-test-context-access")]
    pub fn canonical_kir_safety_operation_ids_for_test(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<Vec<String>, KirBridgeErrorV1> {
        self.validate_identity()?;
        if graph.root.owner != self.identity {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let root = self
            .operations
            .get(&graph.root.identity)
            .copied()
            .ok_or(KirBridgeErrorV1::GraphIdentityMismatch)?;
        let (_, operations) = crate::inspect_operation_tree_details(root, &mut self.context)?;
        let mut ids = operations
            .into_iter()
            .filter_map(|operation| {
                let operation = Operation::get_op_dyn(operation, &self.context);
                pliron::op::op_cast::<dyn CanonicalKirSafetyOpInterface>(&*operation).map(
                    |interface| {
                        assert!(interface.is_self_contained_canonical_kir());
                        operation.get_opid().to_string()
                    },
                )
            })
            .collect::<Vec<_>>();
        ids.sort();
        Ok(ids)
    }

    /// Parses a printed graph in this fresh test session and reattaches only
    /// the pointer-free canonical template needed by exact O0 extraction.
    #[doc(hidden)]
    #[cfg(feature = "internal-test-context-access")]
    pub fn reparse_canonical_kir_text_for_test(
        &mut self,
        template: &KirPlironGraphV1,
        text: &str,
    ) -> Result<KirPlironGraphV1, KirBridgeErrorV1> {
        let root = self.import_operation_text_v1(text)?;
        Ok(KirPlironGraphV1 {
            root,
            metadata: template.metadata.clone(),
            input: template.input,
            canonical_version: template.canonical_version,
            correspondence: template.correspondence.clone(),
            origins: KirBridgeOriginsV1::default(),
        })
    }

    /// Grants a conformance test scoped mutable access to one authenticated graph.
    #[doc(hidden)]
    #[cfg(any(test, feature = "internal-test-context-access"))]
    pub fn with_canonical_kir_graph_mut_for_test(
        &mut self,
        graph: &KirPlironGraphV1,
        action: impl FnOnce(&mut Context, Ptr<Operation>),
    ) -> Result<(), KirBridgeErrorV1> {
        self.validate_identity()?;
        if graph.root.owner != self.identity {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        let root = self
            .operations
            .get(&graph.root.identity)
            .copied()
            .ok_or(KirBridgeErrorV1::GraphIdentityMismatch)?;
        action(&mut self.context, root);
        Ok(())
    }
}

fn digest(
    bytes: &[u8],
    version: KirBridgeCanonicalVersionV1,
) -> Result<KirBridgeDigestV1, KirBridgeErrorV1> {
    use sha2::{Digest, Sha256};

    let canonical_bytes = u64::try_from(bytes.len()).map_err(|_| KirBridgeErrorV1::SizeOverflow)?;
    let mut hasher = Sha256::new();
    let domain = match version {
        KirBridgeCanonicalVersionV1::V9 => KIR_PLIRON_BRIDGE_IDENTITY_DOMAIN_V1,
        KirBridgeCanonicalVersionV1::V10 => KIR_PLIRON_BRIDGE_V10_IDENTITY_DOMAIN_V1,
        KirBridgeCanonicalVersionV1::V11 => KIR_PLIRON_BRIDGE_V11_IDENTITY_DOMAIN_V1,
        KirBridgeCanonicalVersionV1::V12 => KIR_PLIRON_BRIDGE_V12_IDENTITY_DOMAIN_V1,
        KirBridgeCanonicalVersionV1::V13 => KIR_PLIRON_BRIDGE_V13_IDENTITY_DOMAIN_V1,
    };
    hasher.update(
        u32::try_from(domain.len())
            .map_err(|_| KirBridgeErrorV1::SizeOverflow)?
            .to_le_bytes(),
    );
    hasher.update(domain);
    hasher.update(canonical_bytes.to_le_bytes());
    hasher.update(bytes);
    Ok(KirBridgeDigestV1 {
        digest: hasher.finalize().into(),
        canonical_bytes,
    })
}

fn import_module(
    session: &mut PlironSession,
    input_bytes: &[u8],
    canonical_version: KirBridgeCanonicalVersionV1,
    module: Module,
) -> Result<KirPlironGraphV1, KirBridgeErrorV1> {
    let (mut tree_work, correspondence) = preflight(&module)?;
    if matches!(
        canonical_version,
        KirBridgeCanonicalVersionV1::V12 | KirBridgeCanonicalVersionV1::V13
    ) {
        let declarations = portable_requirements(&module)
            .len()
            .checked_add(1)
            .ok_or(KirBridgeErrorV1::SizeOverflow)?;
        add_tree_work(
            &mut tree_work,
            declarations
                .checked_mul(2)
                .ok_or(KirBridgeErrorV1::SizeOverflow)?,
        )?;
    }
    if tree_work > HARD_MAX_OPERATION_TREE_ITEMS {
        return Err(OperationHandleError::OperationTreeLimitExceeded.into());
    }
    session.require_internal_tree_capacity(tree_work)?;
    let root = session.create_module("kir_bridge_v1")?;
    let root_pointer = session
        .operations
        .get(&root.identity)
        .copied()
        .ok_or(KirBridgeErrorV1::GraphIdentityMismatch)?;
    let input = digest(input_bytes, canonical_version)?;
    let built = catch_unwind(AssertUnwindSafe(|| {
        build_module_graph(
            &mut session.context,
            root_pointer,
            &module,
            canonical_version,
            input.digest,
        )
    }));
    let origins = match built {
        Ok(Ok(origins)) => origins,
        Ok(Err(error)) => {
            session.poisoned = true;
            return Err(error);
        }
        Err(_) => {
            session.poisoned = true;
            return Err(KirBridgeErrorV1::UpstreamPanicked);
        }
    };
    session.finish_internal_root_construction(&root)?;
    Ok(KirPlironGraphV1 {
        root,
        metadata: module,
        input,
        canonical_version,
        correspondence,
        origins,
    })
}

fn extract_module(
    session: &mut PlironSession,
    graph: &KirPlironGraphV1,
) -> Result<Module, KirBridgeErrorV1> {
    session.validate_identity()?;
    if graph.root.owner != session.identity {
        return Err(KirBridgeErrorV1::GraphIdentityMismatch);
    }
    let root = session
        .operations
        .get(&graph.root.identity)
        .copied()
        .ok_or(KirBridgeErrorV1::GraphIdentityMismatch)?;
    match catch_unwind(AssertUnwindSafe(|| {
        extract_module_graph(
            &session.context,
            root,
            &graph.metadata,
            graph.canonical_version,
            graph.input.digest,
        )
    })) {
        Ok(result) => result,
        Err(_) => {
            session.poisoned = true;
            Err(KirBridgeErrorV1::UpstreamPanicked)
        }
    }
}

fn extract_optimized_module(
    session: &mut PlironSession,
    graph: &KirPlironGraphV1,
) -> Result<(Module, Vec<KirBridgeCorrespondenceV1>), KirBridgeErrorV1> {
    session.validate_identity()?;
    if graph.root.owner != session.identity {
        return Err(KirBridgeErrorV1::GraphIdentityMismatch);
    }
    let root = session
        .operations
        .get(&graph.root.identity)
        .copied()
        .ok_or(KirBridgeErrorV1::GraphIdentityMismatch)?;
    match catch_unwind(AssertUnwindSafe(|| {
        extract_optimized_module_graph(
            &session.context,
            root,
            &graph.metadata,
            &graph.origins,
            graph.canonical_version,
            graph.input.digest,
        )
    })) {
        Ok(result) => result,
        Err(_) => {
            session.poisoned = true;
            Err(KirBridgeErrorV1::UpstreamPanicked)
        }
    }
}

fn preflight(module: &Module) -> Result<(usize, Vec<KirBridgeCorrespondenceV1>), KirBridgeErrorV1> {
    let mut tree_work = BUILTIN_MODULE_ROOT_TREE_WORK_V1;
    let mut ordinal = 0_u64;
    let mut correspondence = Vec::new();
    for (function_index, function) in module.functions.iter().enumerate() {
        function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
            .try_for_each(preflight_type)?;
        let Some(body) = &function.body else {
            continue;
        };
        add_tree_work(&mut tree_work, 3)?;
        let function_index = to_u32(function_index)?;
        push_correspondence(
            &mut correspondence,
            &mut ordinal,
            KirBridgeCoordinateV1::Function {
                function: function_index,
            },
        )?;
        for (block_index, block) in body.blocks.iter().enumerate() {
            add_tree_work(&mut tree_work, 1)?;
            let block_index = to_u32(block_index)?;
            push_correspondence(
                &mut correspondence,
                &mut ordinal,
                KirBridgeCoordinateV1::Block {
                    function: function_index,
                    block: block_index,
                },
            )?;
            block
                .parameters
                .iter()
                .try_for_each(|value| preflight_type(&value.ty))?;
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let coordinate = KirBridgeCoordinateV1::Operation {
                    function: function_index,
                    block: block_index,
                    operation: to_u32(operation_index)?,
                };
                operation
                    .results
                    .iter()
                    .try_for_each(|value| preflight_type(&value.ty))?;
                preflight_operation(operation, coordinate)?;
                add_tree_work(&mut tree_work, 2)?;
                push_correspondence(&mut correspondence, &mut ordinal, coordinate)?;
            }
            let coordinate = KirBridgeCoordinateV1::Terminator {
                function: function_index,
                block: block_index,
            };
            preflight_terminator(block.terminator.as_ref(), coordinate)?;
            add_tree_work(&mut tree_work, 2)?;
            push_correspondence(&mut correspondence, &mut ordinal, coordinate)?;
        }
    }
    Ok((tree_work, correspondence))
}

fn add_tree_work(tree_work: &mut usize, additional: usize) -> Result<(), KirBridgeErrorV1> {
    *tree_work = tree_work
        .checked_add(additional)
        .ok_or(KirBridgeErrorV1::SizeOverflow)?;
    if *tree_work > HARD_MAX_OPERATION_TREE_ITEMS {
        return Err(OperationHandleError::OperationTreeLimitExceeded.into());
    }
    Ok(())
}

fn push_correspondence(
    correspondence: &mut Vec<KirBridgeCorrespondenceV1>,
    ordinal: &mut u64,
    coordinate: KirBridgeCoordinateV1,
) -> Result<(), KirBridgeErrorV1> {
    correspondence.push(KirBridgeCorrespondenceV1 {
        pliron_ordinal: *ordinal,
        coordinate,
    });
    *ordinal = ordinal
        .checked_add(1)
        .ok_or(KirBridgeErrorV1::SizeOverflow)?;
    Ok(())
}

fn to_u32(value: usize) -> Result<u32, KirBridgeErrorV1> {
    u32::try_from(value).map_err(|_| KirBridgeErrorV1::SizeOverflow)
}

fn preflight_type(ty: &Type) -> Result<(), KirBridgeErrorV1> {
    match ty {
        Type::Unit | Type::Scalar(_) => Ok(()),
        Type::Pointer(pointer) => {
            preflight_address_space(pointer.address_space)?;
            preflight_type(&pointer.pointee)
        }
        Type::Slice(slice) => {
            preflight_address_space(slice.address_space)?;
            preflight_type(&slice.element)
        }
        Type::KernelContext(context) => {
            if context.is_complete() {
                Ok(())
            } else {
                Err(KirBridgeErrorV1::UnsupportedType)
            }
        }
        Type::GlobalCapability(capability) => {
            if !capability.is_complete() {
                return Err(KirBridgeErrorV1::UnsupportedType);
            }
            preflight_type(capability.element())
        }
        Type::ExecutionCapability(capability) => {
            if capability.is_complete() {
                Ok(())
            } else {
                Err(KirBridgeErrorV1::UnsupportedType)
            }
        }
    }
}

fn preflight_address_space(address_space: AddressSpace) -> Result<(), KirBridgeErrorV1> {
    let _ = address_space;
    Ok(())
}

fn preflight_operation(
    operation: &KirOperation,
    coordinate: KirBridgeCoordinateV1,
) -> Result<(), KirBridgeErrorV1> {
    match &operation.kind {
        OperationKind::Constant(_)
        | OperationKind::Unary { .. }
        | OperationKind::Binary { .. }
        | OperationKind::Compare { .. }
        | OperationKind::Cast { .. }
        | OperationKind::Select { .. }
        | OperationKind::Call { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. }
        | OperationKind::Load { .. }
        | OperationKind::Store { .. }
        | OperationKind::KernelContextIssue(_)
        | OperationKind::GlobalCapabilityBind(_)
        | OperationKind::GlobalCapabilityIndex(_)
        | OperationKind::ExecutionCapability(_) => Ok(()),
        OperationKind::Intrinsic(_)
        | OperationKind::MemoryIntrinsic(_)
        | OperationKind::Alloca { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::GuardedStore { .. }
        | OperationKind::Barrier(_)
        | OperationKind::Atomic(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_)
        | OperationKind::Matrix(_)
        | OperationKind::Gfx950LdsTranspose(_)
        | OperationKind::Wave(_)
        | OperationKind::InlineAssembly(_) => CanonicalKirOperationAttr::new(operation)
            .map(|_| ())
            .ok_or(KirBridgeErrorV1::UnsupportedOperation { coordinate }),
    }
}

fn preflight_terminator(
    terminator: Option<&Terminator>,
    coordinate: KirBridgeCoordinateV1,
) -> Result<(), KirBridgeErrorV1> {
    match terminator {
        Some(
            Terminator::Branch { .. }
            | Terminator::ConditionalBranch { .. }
            | Terminator::Return { .. },
        ) => Ok(()),
        Some(
            terminator @ (Terminator::Switch { .. }
            | Terminator::IntegerSwitch { .. }
            | Terminator::Unreachable),
        ) => dialect_gpu::CanonicalKirTerminatorAttr::new(terminator)
            .map(|_| ())
            .ok_or(KirBridgeErrorV1::UnsupportedTerminator { coordinate }),
        _ => Err(KirBridgeErrorV1::UnsupportedTerminator { coordinate }),
    }
}

fn build_module_graph(
    context: &mut Context,
    root: Ptr<Operation>,
    module: &Module,
    canonical_version: KirBridgeCanonicalVersionV1,
    graph_epoch: [u8; 32],
) -> Result<KirBridgeOriginsV1, KirBridgeErrorV1> {
    if !Operation::is_op::<ModuleOp>(root, context) {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let root = ModuleOp::from_operation(root);
    let mut origins = KirBridgeOriginsV1::default();
    if matches!(
        canonical_version,
        KirBridgeCanonicalVersionV1::V12 | KirBridgeCanonicalVersionV1::V13
    ) {
        append_v12_module_contract(context, &root, module, graph_epoch)?;
    }
    for (function_index, function) in module.functions.iter().enumerate() {
        let Some(body) = &function.body else {
            continue;
        };
        let parameters = function
            .signature
            .parameters
            .iter()
            .map(|ty| type_to_pliron(context, ty))
            .collect::<Result<Vec<_>, _>>()?;
        let results = function
            .signature
            .results
            .iter()
            .map(|ty| type_to_pliron(context, ty))
            .collect::<Result<Vec<_>, _>>()?;
        let function_type = FunctionType::get(context, parameters, results);
        let name = Identifier::try_from(format!("kir_fn_{function_index}"))
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let function_op = FuncOp::new(context, name, function_type);
        root.append_operation(context, function_op.get_operation(), 0);
        origins
            .functions
            .insert(function_op.get_operation(), function_index);

        let mut blocks = BTreeMap::new();
        let mut values = BTreeMap::new();
        let Some(entry_source) = body.blocks.first() else {
            return Err(KirBridgeErrorV1::MissingFunctionBody {
                function: to_u32(function_index)?,
            });
        };
        let entry = function_op.get_entry_block(context);
        blocks.insert(entry_source.id, entry);
        origins
            .blocks
            .insert(entry, (function_index, entry_source.id));
        for parameter in &entry_source.parameters {
            let ty = type_to_pliron(context, &parameter.ty)?;
            BasicBlock::push_argument(entry, context, ty);
        }
        for block in body.blocks.iter().skip(1) {
            let argument_types = block
                .parameters
                .iter()
                .map(|value| type_to_pliron(context, &value.ty))
                .collect::<Result<Vec<_>, _>>()?;
            let label = Identifier::try_from(format!("kir_bb_{}", block.id.0))
                .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
            let live = BasicBlock::new(context, Some(label), argument_types);
            live.insert_at_back(
                function_op.get_operation().deref(context).get_region(0),
                context,
            );
            blocks.insert(block.id, live);
            origins.blocks.insert(live, (function_index, block.id));
        }

        if body.parameters.len() != function.signature.parameters.len() {
            return Err(KirBridgeErrorV1::MalformedGraph);
        }
        for (index, value_id) in body.parameters.iter().enumerate() {
            let live = entry.deref(context).get_argument(index);
            values.insert(*value_id, live);
            origins.values.insert(live, *value_id);
            origins
                .logical_types
                .insert(live, function.signature.parameters[index].clone());
        }
        for (block_index, block) in body.blocks.iter().enumerate() {
            let live = block_for(&blocks, function_index, block.id)?;
            let offset = usize::from(block_index == 0) * body.parameters.len();
            for (parameter_index, parameter) in block.parameters.iter().enumerate() {
                let live_value = live.deref(context).get_argument(offset + parameter_index);
                values.insert(parameter.id, live_value);
                origins.values.insert(live_value, parameter.id);
                origins
                    .logical_types
                    .insert(live_value, parameter.ty.clone());
            }
        }

        for (block_index, operation_index) in
            operation_build_schedule(body, function_index, &values)?
        {
            let block = &body.blocks[block_index];
            let operation = &block.operations[operation_index];
            let live_block = block_for(&blocks, function_index, block.id)?;
            let coordinate = KirBridgeCoordinateV1::Operation {
                function: to_u32(function_index)?,
                block: to_u32(block_index)?,
                operation: to_u32(operation_index)?,
            };
            let live = build_operation(
                context,
                function_index,
                operation,
                &values,
                coordinate,
                graph_epoch,
            )?;
            live.insert_at_back(live_block, context);
            let raw = live.deref(context);
            if raw.get_num_results() != operation.results.len() {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            for (index, result) in operation.results.iter().enumerate() {
                let expected = type_to_pliron(context, &result.ty)?;
                if raw.get_type(index) != expected {
                    return Err(KirBridgeErrorV1::MalformedGraph);
                }
                let live_value = raw.get_result(index);
                if values.insert(result.id, live_value).is_some()
                    || origins.values.insert(live_value, result.id).is_some()
                    || origins
                        .logical_types
                        .insert(live_value, result.ty.clone())
                        .is_some()
                {
                    return Err(KirBridgeErrorV1::MalformedGraph);
                }
            }
        }
        for (block_index, block) in body.blocks.iter().enumerate() {
            let live_block = block_for(&blocks, function_index, block.id)?;
            let terminator = build_terminator(
                context,
                block.terminator.as_ref(),
                TerminatorBuildContextV1 {
                    function: function_index,
                    block: block_index,
                    values: &values,
                    blocks: &blocks,
                    canonical_version,
                    graph_epoch,
                },
            )?;
            terminator.insert_at_back(live_block, context);
        }
    }
    Ok(origins)
}

fn portable_requirements(
    module: &Module,
) -> Vec<&fe2o3_kernel_ir::ExecutionCapabilityRequirementV1> {
    module
        .required_capabilities
        .iter()
        .filter_map(|capability| match capability {
            TargetCapability::Execution(requirement) => Some(requirement),
            TargetCapability::Float16
            | TargetCapability::BFloat16
            | TargetCapability::Float64
            | TargetCapability::Int64
            | TargetCapability::Subgroups
            | TargetCapability::SubgroupSize(_)
            | TargetCapability::WorkgroupMemory
            | TargetCapability::WorkgroupBarrier
            | TargetCapability::Atomic { .. }
            | TargetCapability::DynamicWorkgroupMemory
            | TargetCapability::Extension { .. }
            | TargetCapability::WaveWidth(_) => None,
        })
        .collect()
}

fn append_v12_module_contract(
    context: &mut Context,
    root: &ModuleOp,
    module: &Module,
    graph_epoch: [u8; 32],
) -> Result<(), KirBridgeErrorV1> {
    let requirements = portable_requirements(module);
    let requirement_count = to_u32(requirements.len())?;
    let graph_contract = GraphContractOp::new(
        context,
        CanonicalIdentityAttr::from_bytes(graph_epoch),
        requirement_count,
    )
    .get_operation();
    root.append_operation(context, graph_contract, 0);
    for (ordinal, requirement) in requirements.into_iter().enumerate() {
        let declaration = ExecutionRequirementOp::new(
            context,
            CanonicalIdentityAttr::from_bytes(graph_epoch),
            to_u32(ordinal)?,
            requirement,
        )
        .get_operation();
        root.append_operation(context, declaration, 0);
    }
    Ok(())
}

fn source_coordinate_attr(
    coordinate: KirBridgeCoordinateV1,
) -> Result<SourceCoordinateAttr, KirBridgeErrorV1> {
    match coordinate {
        KirBridgeCoordinateV1::Operation {
            function,
            block,
            operation,
        } => Ok(SourceCoordinateAttr::new(function, block, operation)),
        KirBridgeCoordinateV1::Function { .. }
        | KirBridgeCoordinateV1::Block { .. }
        | KirBridgeCoordinateV1::Terminator { .. } => Err(KirBridgeErrorV1::MalformedGraph),
    }
}

fn terminator_coordinate_attr(function: u32, block: u32) -> SourceCoordinateAttr {
    SourceCoordinateAttr::new(function, block, u32::MAX)
}

fn context_operation_identity(
    graph_epoch: [u8; 32],
    coordinate: KirBridgeCoordinateV1,
) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut digest = Sha256::new();
    digest.update(KIR_PLIRON_CONTEXT_OPERATION_IDENTITY_DOMAIN_V1);
    digest.update(graph_epoch);
    match coordinate {
        KirBridgeCoordinateV1::Operation {
            function,
            block,
            operation,
        } => {
            digest.update(function.to_le_bytes());
            digest.update(block.to_le_bytes());
            digest.update(operation.to_le_bytes());
        }
        KirBridgeCoordinateV1::Function { .. }
        | KirBridgeCoordinateV1::Block { .. }
        | KirBridgeCoordinateV1::Terminator { .. } => {
            unreachable!("kernel-context identity requires an operation coordinate")
        }
    }
    digest.finalize().into()
}

fn execution_operation_identity(
    contract: &fe2o3_kernel_ir::ExecutionCapabilityOpV1,
) -> Result<[u8; 32], KirBridgeErrorV1> {
    use sha2::{Digest, Sha256};

    let bytes = fe2o3_kernel_ir::encode_execution_capability_contract_v1(contract)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
    let mut digest = Sha256::new();
    digest.update(KIR_PLIRON_EXECUTION_OPERATION_IDENTITY_DOMAIN_V1);
    digest.update(contract.source.function);
    digest.update(contract.source.block.to_le_bytes());
    digest.update(contract.source.operation);
    digest.update(
        u64::try_from(bytes.len())
            .map_err(|_| KirBridgeErrorV1::SizeOverflow)?
            .to_le_bytes(),
    );
    digest.update(bytes);
    Ok(digest.finalize().into())
}

fn execution_type_contract_identity(
    capability: &fe2o3_kernel_ir::ExecutionCapabilityTypeV1,
) -> Result<[u8; 32], KirBridgeErrorV1> {
    let bytes = fe2o3_kernel_ir::encode_execution_capability_type_v1(capability)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
    identity_for_bytes(
        b"FE2O3/KIR-PLIRON-BRIDGE/EXECUTION-CAPABILITY-TYPE/V1\0",
        &bytes,
    )
}

fn identity_for_bytes(domain: &[u8], bytes: &[u8]) -> Result<[u8; 32], KirBridgeErrorV1> {
    use sha2::{Digest, Sha256};

    let mut digest = Sha256::new();
    digest.update(
        u32::try_from(domain.len())
            .map_err(|_| KirBridgeErrorV1::SizeOverflow)?
            .to_le_bytes(),
    );
    digest.update(domain);
    digest.update(
        u64::try_from(bytes.len())
            .map_err(|_| KirBridgeErrorV1::SizeOverflow)?
            .to_le_bytes(),
    );
    digest.update(bytes);
    Ok(digest.finalize().into())
}

fn block_for(
    blocks: &BTreeMap<BlockId, Ptr<BasicBlock>>,
    function: usize,
    block: BlockId,
) -> Result<Ptr<BasicBlock>, KirBridgeErrorV1> {
    blocks
        .get(&block)
        .copied()
        .ok_or(KirBridgeErrorV1::MissingBlock {
            function: to_u32(function)?,
            block: block.0,
        })
}

fn value_for(
    values: &BTreeMap<ValueId, Value>,
    function: usize,
    value: ValueId,
) -> Result<Value, KirBridgeErrorV1> {
    values
        .get(&value)
        .copied()
        .ok_or(KirBridgeErrorV1::MissingValue {
            function: to_u32(function)?,
            value: value.0,
        })
}

fn values_for(
    values: &BTreeMap<ValueId, Value>,
    function: usize,
    ids: &[ValueId],
) -> Result<Vec<Value>, KirBridgeErrorV1> {
    ids.iter()
        .map(|value| value_for(values, function, *value))
        .collect()
}

fn operation_build_schedule(
    body: &fe2o3_kernel_ir::FunctionBody,
    function: usize,
    prebound_values: &BTreeMap<ValueId, Value>,
) -> Result<Vec<(usize, usize)>, KirBridgeErrorV1> {
    let function = to_u32(function)?;
    let mut locations = Vec::new();
    let mut producers = BTreeMap::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let node = locations.len();
            locations.push((block_index, operation_index));
            for result in &operation.results {
                if prebound_values.contains_key(&result.id)
                    || producers.insert(result.id, node).is_some()
                {
                    return Err(KirBridgeErrorV1::MalformedGraph);
                }
            }
        }
    }

    let mut incoming = vec![0_usize; locations.len()];
    let mut outgoing = vec![Vec::new(); locations.len()];
    for (node, &(block_index, operation_index)) in locations.iter().enumerate() {
        let operation = &body.blocks[block_index].operations[operation_index];
        let mut dependencies = BTreeSet::new();
        if operation_index != 0 {
            dependencies.insert(node - 1);
        }
        for operand in operation.operands() {
            if prebound_values.contains_key(&operand) {
                continue;
            }
            let producer =
                producers
                    .get(&operand)
                    .copied()
                    .ok_or(KirBridgeErrorV1::MissingValue {
                        function,
                        value: operand.0,
                    })?;
            dependencies.insert(producer);
        }
        incoming[node] = dependencies.len();
        for dependency in dependencies {
            outgoing[dependency].push(node);
        }
    }

    let mut ready = incoming
        .iter()
        .enumerate()
        .filter_map(|(node, incoming)| (*incoming == 0).then_some(node))
        .collect::<BTreeSet<_>>();
    let mut schedule = Vec::with_capacity(locations.len());
    while let Some(node) = ready.iter().next().copied() {
        ready.remove(&node);
        schedule.push(locations[node]);
        for successor in outgoing[node].iter().copied() {
            incoming[successor] = incoming[successor]
                .checked_sub(1)
                .ok_or(KirBridgeErrorV1::MalformedGraph)?;
            if incoming[successor] == 0 {
                ready.insert(successor);
            }
        }
    }
    if schedule.len() != locations.len() {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    Ok(schedule)
}

fn build_operation(
    context: &mut Context,
    function: usize,
    operation: &KirOperation,
    values: &BTreeMap<ValueId, Value>,
    coordinate: KirBridgeCoordinateV1,
    graph_epoch: [u8; 32],
) -> Result<Ptr<Operation>, KirBridgeErrorV1> {
    macro_rules! canonical_operation {
        ($operation_type:ty) => {{
            let result_types = operation
                .results
                .iter()
                .map(|result| type_to_pliron(context, &result.ty))
                .collect::<Result<Vec<_>, _>>()?;
            <$operation_type>::new(
                context,
                operation,
                values_for(values, function, &operation.kind.operands())?,
                result_types,
                CanonicalIdentityAttr::from_bytes(graph_epoch),
                source_coordinate_attr(coordinate)?,
            )
            .ok_or(KirBridgeErrorV1::UnsupportedOperation { coordinate })?
            .get_operation()
        }};
    }
    let live = match &operation.kind {
        OperationKind::Constant(value) => {
            PlironConstantOp::new(context, constant_to_pliron(context, value)?).get_operation()
        }
        OperationKind::Unary { op, operand } => PlironUnaryOp::new(
            context,
            unary_to_pliron(*op),
            value_for(values, function, *operand)?,
        )
        .get_operation(),
        OperationKind::Binary { op, lhs, rhs } => PlironBinaryOp::new(
            context,
            binary_to_pliron(*op),
            value_for(values, function, *lhs)?,
            value_for(values, function, *rhs)?,
        )
        .get_operation(),
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } => PlironCompareOp::new(
            context,
            compare_to_pliron(*predicate),
            value_for(values, function, *lhs)?,
            value_for(values, function, *rhs)?,
        )
        .get_operation(),
        OperationKind::Cast { kind, value, to } => {
            let to = type_to_pliron(context, to)?;
            CastOp::new(
                context,
                cast_to_pliron(*kind),
                value_for(values, function, *value)?,
                to,
            )
            .get_operation()
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => PlironSelectOp::new(
            context,
            value_for(values, function, *condition)?,
            value_for(values, function, *true_value)?,
            value_for(values, function, *false_value)?,
        )
        .get_operation(),
        OperationKind::Call { callee, arguments } => {
            let result_types = operation
                .results
                .iter()
                .map(|result| type_to_pliron(context, &result.ty))
                .collect::<Result<Vec<_>, _>>()?;
            CallOp::new(
                context,
                callee.as_str(),
                values_for(values, function, arguments)?,
                result_types,
            )
            .get_operation()
        }
        OperationKind::SliceLength { slice } => {
            let slice = value_for(values, function, *slice)?;
            let is_capability = slice
                .get_type(context)
                .deref(context)
                .is::<PlironGlobalCapabilityType>();
            if is_capability {
                PlironGlobalCapabilityLengthOp::new(context, slice).get_operation()
            } else {
                SliceLengthOp::new(context, slice).get_operation()
            }
        }
        OperationKind::SliceData { slice } => {
            let slice = value_for(values, function, *slice)?;
            let is_capability = slice
                .get_type(context)
                .deref(context)
                .is::<PlironGlobalCapabilityType>();
            if is_capability {
                PlironGlobalCapabilityDataOp::new(context, slice)
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?
                    .get_operation()
            } else {
                SliceDataOp::new(context, slice)
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?
                    .get_operation()
            }
        }
        OperationKind::GetElementPointer { base, offset } => GetElementPointerOp::new(
            context,
            value_for(values, function, *base)?,
            value_for(values, function, *offset)?,
        )
        .get_operation(),
        OperationKind::Load { pointer, access } => LoadOp::new(
            context,
            value_for(values, function, *pointer)?,
            access.alignment,
            access.volatile,
        )
        .ok_or(KirBridgeErrorV1::MalformedGraph)?
        .get_operation(),
        OperationKind::Store {
            pointer,
            value,
            access,
        } => StoreOp::new(
            context,
            value_for(values, function, *pointer)?,
            value_for(values, function, *value)?,
            access.alignment,
            access.volatile,
        )
        .ok_or(KirBridgeErrorV1::MalformedGraph)?
        .get_operation(),
        OperationKind::KernelContextIssue(issue) => {
            let [result] = operation.results.as_slice() else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            let Type::KernelContext(context_type) = &result.ty else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            let result_type = kernel_context_type_to_pliron(context, context_type)?;
            let source = issue.source();
            KernelContextIssueOp::new(
                context,
                result_type,
                CanonicalIdentityAttr::from_bytes(graph_epoch),
                source_coordinate_attr(coordinate)?,
                CanonicalIdentityAttr::from_bytes(context_operation_identity(
                    graph_epoch,
                    coordinate,
                )),
                CanonicalIdentityAttr::from_bytes(source.frontend_unit()),
                CanonicalIdentityAttr::from_bytes(source.function()),
                CanonicalIdentityAttr::from_bytes(source.contract()),
                CanonicalIdentityAttr::from_bytes(source.issuance()),
            )
            .get_operation()
        }
        OperationKind::GlobalCapabilityBind(bind) => {
            let [result] = operation.results.as_slice() else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            let Type::GlobalCapability(capability) = &result.ty else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            let logical_type = PlironGlobalCapabilityType::get(
                context,
                type_to_pliron(context, capability.element())?,
                kernel_context_type_to_pliron(context, capability.context())?,
                capability.role(),
            );
            PlironGlobalCapabilityBindOp::new(
                context,
                logical_type,
                value_for(values, function, bind.context)?,
                value_for(values, function, bind.physical)?,
            )
            .get_operation()
        }
        OperationKind::GlobalCapabilityIndex(index) => {
            let [result] = operation.results.as_slice() else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            if result.ty != Type::INDEX {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            let capability = value_for(values, function, index.capability)?;
            let role = {
                let capability_type = capability.get_type(context);
                let capability_type = capability_type.deref(context);
                capability_type
                    .downcast_ref::<PlironGlobalCapabilityType>()
                    .and_then(PlironGlobalCapabilityType::role)
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?
            };
            PlironGlobalCapabilityIndexOp::new(
                context,
                capability,
                value_for(values, function, index.index)?,
                role,
            )
            .get_operation()
        }
        OperationKind::ExecutionCapability(contract) => {
            let result_types = operation
                .results
                .iter()
                .map(|result| type_to_pliron(context, &result.ty))
                .collect::<Result<Vec<_>, _>>()?;
            PlironExecutionCapabilityOp::new(
                context,
                contract,
                values_for(values, function, &contract.operands)?,
                result_types,
                CanonicalIdentityAttr::from_bytes(graph_epoch),
                source_coordinate_attr(coordinate)?,
                CanonicalIdentityAttr::from_bytes(execution_operation_identity(contract)?),
            )
            .ok_or(KirBridgeErrorV1::MalformedGraph)?
            .get_operation()
        }
        OperationKind::Intrinsic(_) => canonical_operation!(PlironIntrinsicOp),
        OperationKind::MemoryIntrinsic(_) => canonical_operation!(PlironMemoryIntrinsicOp),
        OperationKind::Alloca { .. } => canonical_operation!(PlironAllocaOp),
        OperationKind::GuardedLoad { .. } => canonical_operation!(PlironGuardedLoadOp),
        OperationKind::GuardedStore { .. } => canonical_operation!(PlironGuardedStoreOp),
        OperationKind::Barrier(_) => canonical_operation!(PlironCanonicalBarrierOp),
        OperationKind::Atomic(_) => canonical_operation!(PlironAtomicOp),
        OperationKind::Fence(_) => canonical_operation!(PlironCanonicalFenceOp),
        OperationKind::WorkgroupBarrier(_) => canonical_operation!(PlironWorkgroupBarrierOp),
        OperationKind::WorkgroupMemory(_) => canonical_operation!(PlironWorkgroupMemoryOp),
        OperationKind::Matrix(_) => canonical_operation!(PlironMatrixOp),
        OperationKind::Gfx950LdsTranspose(_) => {
            canonical_operation!(PlironGfx950LdsTransposeOp)
        }
        OperationKind::Wave(_) => canonical_operation!(PlironWaveOp),
        OperationKind::InlineAssembly(_) => canonical_operation!(PlironInlineAssemblyOp),
    };
    Ok(live)
}

struct TerminatorBuildContextV1<'a> {
    function: usize,
    block: usize,
    values: &'a BTreeMap<ValueId, Value>,
    blocks: &'a BTreeMap<BlockId, Ptr<BasicBlock>>,
    canonical_version: KirBridgeCanonicalVersionV1,
    graph_epoch: [u8; 32],
}

fn build_terminator(
    context: &mut Context,
    terminator: Option<&Terminator>,
    build: TerminatorBuildContextV1<'_>,
) -> Result<Ptr<Operation>, KirBridgeErrorV1> {
    let TerminatorBuildContextV1 {
        function,
        block: block_index,
        values,
        blocks,
        canonical_version,
        graph_epoch,
    } = build;
    match terminator {
        Some(Terminator::Branch { target, arguments }) => Ok(BranchOp::new(
            context,
            block_for(blocks, function, *target)?,
            values_for(values, function, arguments)?,
        )
        .get_operation()),
        Some(Terminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        }) => Ok(CondBranchOp::new(
            context,
            value_for(values, function, *condition)?,
            block_for(blocks, function, *then_target)?,
            values_for(values, function, then_arguments)?,
            block_for(blocks, function, *else_target)?,
            values_for(values, function, else_arguments)?,
        )
        .get_operation()),
        Some(Terminator::Return { values: returned })
            if matches!(
                canonical_version,
                KirBridgeCanonicalVersionV1::V12 | KirBridgeCanonicalVersionV1::V13
            ) && returned.is_empty() =>
        {
            Ok(RankedReturnOp::new(context).get_operation())
        }
        Some(Terminator::Return { values: returned }) => {
            Ok(ReturnOp::new(context, values_for(values, function, returned)?).get_operation())
        }
        Some(
            terminator @ Terminator::Switch {
                selector,
                cases,
                default_target,
                default_arguments,
            },
        ) => {
            let successors = cases
                .iter()
                .map(|case| block_for(blocks, function, case.target))
                .chain([block_for(blocks, function, *default_target)])
                .collect::<Result<Vec<_>, _>>()?;
            let arguments = cases
                .iter()
                .map(|case| values_for(values, function, &case.arguments))
                .chain([values_for(values, function, default_arguments)])
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PlironSwitchOp::new(
                context,
                terminator,
                value_for(values, function, *selector)?,
                successors,
                arguments,
                CanonicalIdentityAttr::from_bytes(graph_epoch),
                terminator_coordinate_attr(to_u32(function)?, to_u32(block_index)?),
            )
            .ok_or(KirBridgeErrorV1::UnsupportedTerminator {
                coordinate: KirBridgeCoordinateV1::Terminator {
                    function: to_u32(function)?,
                    block: to_u32(block_index)?,
                },
            })?
            .get_operation())
        }
        Some(
            terminator @ Terminator::IntegerSwitch {
                selector,
                cases,
                default_target,
                default_arguments,
            },
        ) => {
            let successors = cases
                .iter()
                .map(|case| block_for(blocks, function, case.target))
                .chain([block_for(blocks, function, *default_target)])
                .collect::<Result<Vec<_>, _>>()?;
            let arguments = cases
                .iter()
                .map(|case| values_for(values, function, &case.arguments))
                .chain([values_for(values, function, default_arguments)])
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PlironIntegerSwitchOp::new(
                context,
                terminator,
                value_for(values, function, *selector)?,
                successors,
                arguments,
                CanonicalIdentityAttr::from_bytes(graph_epoch),
                terminator_coordinate_attr(to_u32(function)?, to_u32(block_index)?),
            )
            .ok_or(KirBridgeErrorV1::UnsupportedTerminator {
                coordinate: KirBridgeCoordinateV1::Terminator {
                    function: to_u32(function)?,
                    block: to_u32(block_index)?,
                },
            })?
            .get_operation())
        }
        Some(Terminator::Unreachable) => Ok(PlironUnreachableOp::new(
            context,
            CanonicalIdentityAttr::from_bytes(graph_epoch),
            terminator_coordinate_attr(to_u32(function)?, to_u32(block_index)?),
        )
        .ok_or(KirBridgeErrorV1::UnsupportedTerminator {
            coordinate: KirBridgeCoordinateV1::Terminator {
                function: to_u32(function)?,
                block: to_u32(block_index)?,
            },
        })?
        .get_operation()),
        None => Err(KirBridgeErrorV1::MalformedGraph),
    }
}

fn type_to_pliron(context: &Context, ty: &Type) -> Result<TypeHandle, KirBridgeErrorV1> {
    Ok(match ty {
        Type::Unit => UnitType::get(context).into(),
        Type::Scalar(ScalarType::Bool) => IntegerType::get(context, 1, Signedness::Signless).into(),
        Type::Scalar(ScalarType::I8) => IntegerType::get(context, 8, Signedness::Signed).into(),
        Type::Scalar(ScalarType::I16) => IntegerType::get(context, 16, Signedness::Signed).into(),
        Type::Scalar(ScalarType::I32) => IntegerType::get(context, 32, Signedness::Signed).into(),
        Type::Scalar(ScalarType::I64) => IntegerType::get(context, 64, Signedness::Signed).into(),
        Type::Scalar(ScalarType::I128) => IntegerType::get(context, 128, Signedness::Signed).into(),
        Type::Scalar(ScalarType::U8) => IntegerType::get(context, 8, Signedness::Unsigned).into(),
        Type::Scalar(ScalarType::U16) => IntegerType::get(context, 16, Signedness::Unsigned).into(),
        Type::Scalar(ScalarType::U32) => IntegerType::get(context, 32, Signedness::Unsigned).into(),
        Type::Scalar(ScalarType::U64) => IntegerType::get(context, 64, Signedness::Unsigned).into(),
        Type::Scalar(ScalarType::U128) => {
            IntegerType::get(context, 128, Signedness::Unsigned).into()
        }
        Type::Scalar(ScalarType::Index) => IndexType::get(context).into(),
        Type::Scalar(ScalarType::F16) => FP16Type::get(context).into(),
        Type::Scalar(ScalarType::Bf16) => BFloat16Type::get(context).into(),
        Type::Scalar(ScalarType::F32) => FP32Type::get(context).into(),
        Type::Scalar(ScalarType::F64) => FP64Type::get(context).into(),
        Type::Pointer(pointer) => PlironPointerType::get(
            context,
            type_to_pliron(context, &pointer.pointee)?,
            address_space_to_pliron(pointer.address_space)?,
            access_mode_to_pliron(pointer.access),
        )
        .into(),
        Type::Slice(slice) => PlironSliceType::get(
            context,
            type_to_pliron(context, &slice.element)?,
            address_space_to_pliron(slice.address_space)?,
            access_mode_to_pliron(slice.access),
        )
        .into(),
        Type::KernelContext(kernel_context) => {
            kernel_context_type_to_pliron(context, kernel_context)?.into()
        }
        Type::GlobalCapability(capability) => PlironGlobalCapabilityType::get(
            context,
            type_to_pliron(context, capability.element())?,
            kernel_context_type_to_pliron(context, capability.context())?,
            capability.role(),
        )
        .into(),
        Type::ExecutionCapability(capability) => {
            PlironExecutionCapabilityType::get(context, capability)
                .ok_or(KirBridgeErrorV1::UnsupportedType)?
                .into()
        }
    })
}

fn kernel_context_type_to_pliron(
    context: &Context,
    kernel_context: &fe2o3_kernel_ir::KernelContextTypeV1,
) -> Result<pliron::r#type::TypedHandle<PlironKernelContextType>, KirBridgeErrorV1> {
    if !kernel_context.is_complete() {
        return Err(KirBridgeErrorV1::UnsupportedType);
    }
    Ok(PlironKernelContextType::get(
        context,
        kernel_context.root().as_str(),
        CanonicalIdentityAttr::from_bytes(*kernel_context.kernel_marker()),
        CanonicalIdentityAttr::from_bytes(*kernel_context.target()),
        CanonicalIdentityAttr::from_bytes(*kernel_context.launch()),
    ))
}

fn kernel_context_type_from_pliron(
    kernel_context: &PlironKernelContextType,
) -> Result<fe2o3_kernel_ir::KernelContextTypeV1, KirBridgeErrorV1> {
    let kernel_marker = kernel_context
        .kernel_marker()
        .bytes()
        .ok_or(KirBridgeErrorV1::UnsupportedType)?;
    let target = kernel_context
        .target()
        .bytes()
        .ok_or(KirBridgeErrorV1::UnsupportedType)?;
    let launch = kernel_context
        .launch()
        .bytes()
        .ok_or(KirBridgeErrorV1::UnsupportedType)?;
    Ok(fe2o3_kernel_ir::KernelContextTypeV1::new(
        kernel_context.root(),
        kernel_marker,
        target,
        launch,
    ))
}

fn global_capability_type_from_pliron(
    context: &Context,
    capability: &PlironGlobalCapabilityType,
) -> Result<fe2o3_kernel_ir::GlobalCapabilityTypeV1, KirBridgeErrorV1> {
    let context_type = capability.kernel_context().deref(context);
    let context_type = context_type
        .downcast_ref::<PlironKernelContextType>()
        .ok_or(KirBridgeErrorV1::UnsupportedType)?;
    let capability = fe2o3_kernel_ir::GlobalCapabilityTypeV1::new(
        type_from_pliron(context, capability.element())?,
        kernel_context_type_from_pliron(context_type)?,
        capability.role().ok_or(KirBridgeErrorV1::UnsupportedType)?,
    );
    if !capability.is_complete() {
        return Err(KirBridgeErrorV1::UnsupportedType);
    }
    Ok(capability)
}

fn address_space_to_pliron(
    address_space: AddressSpace,
) -> Result<AddressSpaceAttr, KirBridgeErrorV1> {
    match address_space {
        AddressSpace::Private => Ok(AddressSpaceAttr::Private),
        AddressSpace::Workgroup => Ok(AddressSpaceAttr::Workgroup),
        AddressSpace::Global => Ok(AddressSpaceAttr::Global),
        AddressSpace::Constant => Ok(AddressSpaceAttr::Constant),
        AddressSpace::Generic => Ok(AddressSpaceAttr::Generic),
    }
}

const fn access_mode_to_pliron(access: AccessMode) -> AccessModeAttr {
    match access {
        AccessMode::ReadOnly => AccessModeAttr::ReadOnly,
        AccessMode::WriteOnly => AccessModeAttr::WriteOnly,
        AccessMode::ReadWrite => AccessModeAttr::ReadWrite,
    }
}

fn constant_to_pliron(context: &Context, constant: &Constant) -> Result<AttrObj, KirBridgeErrorV1> {
    let width = |bits| NonZero::new(bits).ok_or(KirBridgeErrorV1::UnsupportedType);
    let integer = |bits, signedness, value| -> Result<AttrObj, KirBridgeErrorV1> {
        Ok(Box::new(IntegerAttr::new(
            IntegerType::get(context, bits, signedness),
            value,
        )))
    };
    match *constant {
        Constant::Bool(value) => integer(
            1,
            Signedness::Signless,
            APInt::from_u8(u8::from(value), width(1)?),
        ),
        Constant::I8(value) => integer(8, Signedness::Signed, APInt::from_i8(value, width(8)?)),
        Constant::I16(value) => integer(16, Signedness::Signed, APInt::from_i16(value, width(16)?)),
        Constant::I32(value) => integer(32, Signedness::Signed, APInt::from_i32(value, width(32)?)),
        Constant::I64(value) => integer(64, Signedness::Signed, APInt::from_i64(value, width(64)?)),
        Constant::U8(value) => integer(8, Signedness::Unsigned, APInt::from_u8(value, width(8)?)),
        Constant::U16(value) => {
            integer(16, Signedness::Unsigned, APInt::from_u16(value, width(16)?))
        }
        Constant::U32(value) => {
            integer(32, Signedness::Unsigned, APInt::from_u32(value, width(32)?))
        }
        Constant::U64(value) => {
            integer(64, Signedness::Unsigned, APInt::from_u64(value, width(64)?))
        }
        Constant::Index(value) => Ok(Box::new(IndexAttr(value))),
        Constant::F16Bits(value) => Ok(Box::new(FPHalfAttr(Half::from_bits(value.into())))),
        Constant::Bf16Bits(value) => Ok(Box::new(BFloat16Attr(value))),
        Constant::F32Bits(value) => Ok(Box::new(FPSingleAttr(Single::from_bits(value.into())))),
        Constant::F64Bits(value) => Ok(Box::new(FPDoubleAttr(Double::from_bits(value.into())))),
    }
}

const fn unary_to_pliron(op: UnaryOp) -> UnaryKindAttr {
    match op {
        UnaryOp::Negate => UnaryKindAttr::Negate,
        UnaryOp::Not => UnaryKindAttr::Not,
    }
}

const fn binary_to_pliron(op: BinaryOp) -> BinaryKindAttr {
    use fe2o3_kernel_ir::CheckedBinaryOperator;

    match op {
        BinaryOp::Add => BinaryKindAttr::Add,
        BinaryOp::Subtract => BinaryKindAttr::Subtract,
        BinaryOp::Multiply => BinaryKindAttr::Multiply,
        BinaryOp::Divide => BinaryKindAttr::Divide,
        BinaryOp::Remainder => BinaryKindAttr::Remainder,
        BinaryOp::BitAnd => BinaryKindAttr::BitAnd,
        BinaryOp::BitOr => BinaryKindAttr::BitOr,
        BinaryOp::BitXor => BinaryKindAttr::BitXor,
        BinaryOp::ShiftLeft => BinaryKindAttr::ShiftLeft,
        BinaryOp::ShiftRight => BinaryKindAttr::ShiftRight,
        BinaryOp::Checked(CheckedBinaryOperator::Add) => BinaryKindAttr::CheckedAdd,
        BinaryOp::Checked(CheckedBinaryOperator::Subtract) => BinaryKindAttr::CheckedSubtract,
        BinaryOp::Checked(CheckedBinaryOperator::Multiply) => BinaryKindAttr::CheckedMultiply,
    }
}

const fn compare_to_pliron(predicate: fe2o3_kernel_ir::ComparePredicate) -> ComparePredicateAttr {
    use fe2o3_kernel_ir::ComparePredicate;

    match predicate {
        ComparePredicate::Equal => ComparePredicateAttr::Equal,
        ComparePredicate::NotEqual => ComparePredicateAttr::NotEqual,
        ComparePredicate::LessThan => ComparePredicateAttr::LessThan,
        ComparePredicate::LessThanOrEqual => ComparePredicateAttr::LessThanOrEqual,
        ComparePredicate::GreaterThan => ComparePredicateAttr::GreaterThan,
        ComparePredicate::GreaterThanOrEqual => ComparePredicateAttr::GreaterThanOrEqual,
    }
}

const fn cast_to_pliron(kind: CastKind) -> CastKindAttr {
    match kind {
        CastKind::RestrictPointerAccess => CastKindAttr::RestrictPointerAccess,
        CastKind::Truncate => CastKindAttr::Truncate,
        CastKind::ZeroExtend => CastKindAttr::ZeroExtend,
        CastKind::SignExtend => CastKindAttr::SignExtend,
        CastKind::FloatExtend => CastKindAttr::FloatExtend,
        CastKind::FloatTruncate => CastKindAttr::FloatTruncate,
        CastKind::IntegerToFloat => CastKindAttr::IntegerToFloat,
        CastKind::FloatToInteger => CastKindAttr::FloatToInteger,
        CastKind::Bitcast => CastKindAttr::Bitcast,
    }
}

fn validated_live_functions(
    context: &Context,
    root_block: Ptr<BasicBlock>,
    metadata: &Module,
    canonical_version: KirBridgeCanonicalVersionV1,
    graph_epoch: [u8; 32],
) -> Result<Vec<Ptr<Operation>>, KirBridgeErrorV1> {
    let operations = root_block.deref(context).iter(context).collect::<Vec<_>>();
    let defined_functions = metadata
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .count();
    let prefix = match canonical_version {
        KirBridgeCanonicalVersionV1::V9
        | KirBridgeCanonicalVersionV1::V10
        | KirBridgeCanonicalVersionV1::V11 => 0,
        KirBridgeCanonicalVersionV1::V12 | KirBridgeCanonicalVersionV1::V13 => {
            let requirements = portable_requirements(metadata);
            let declaration_count = requirements
                .len()
                .checked_add(1)
                .ok_or(KirBridgeErrorV1::SizeOverflow)?;
            if operations.len()
                != defined_functions
                    .checked_add(declaration_count)
                    .ok_or(KirBridgeErrorV1::SizeOverflow)?
            {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            let graph_contract = Operation::get_op::<GraphContractOp>(operations[0], context)
                .ok_or(KirBridgeErrorV1::MalformedGraph)?;
            if graph_contract.epoch(context) != Some(graph_epoch)
                || graph_contract.requirement_count(context) != Some(to_u32(requirements.len())?)
            {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            for (ordinal, (live, expected)) in operations[1..declaration_count]
                .iter()
                .zip(requirements)
                .enumerate()
            {
                let declaration = Operation::get_op::<ExecutionRequirementOp>(*live, context)
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?;
                if declaration.epoch(context) != Some(graph_epoch)
                    || declaration.ordinal(context) != Some(to_u32(ordinal)?)
                    || declaration.requirement(context).as_ref() != Some(expected)
                {
                    return Err(KirBridgeErrorV1::MalformedGraph);
                }
            }
            declaration_count
        }
    };
    let functions = operations[prefix..].to_vec();
    if functions.len() != defined_functions
        || functions
            .iter()
            .any(|operation| !Operation::is_op::<FuncOp>(*operation, context))
    {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    Ok(functions)
}

fn extract_optimized_module_graph(
    context: &Context,
    root: Ptr<Operation>,
    metadata: &Module,
    origins: &KirBridgeOriginsV1,
    canonical_version: KirBridgeCanonicalVersionV1,
    graph_epoch: [u8; 32],
) -> Result<(Module, Vec<KirBridgeCorrespondenceV1>), KirBridgeErrorV1> {
    if !Operation::is_op::<ModuleOp>(root, context) || root.deref(context).num_regions() != 1 {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let root_region = root.deref(context).get_region(0);
    let root_blocks: Vec<_> = root_region.deref(context).iter(context).collect();
    let [root_block] = root_blocks.as_slice() else {
        return Err(KirBridgeErrorV1::MalformedGraph);
    };
    let live_functions = validated_live_functions(
        context,
        *root_block,
        metadata,
        canonical_version,
        graph_epoch,
    )?;
    if live_functions.len()
        != metadata
            .functions
            .iter()
            .filter(|function| function.body.is_some())
            .count()
    {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }

    let mut output = metadata.clone();
    let mut ordinal = 0_u64;
    let mut correspondence = Vec::new();
    for (function_index, function) in output.functions.iter_mut().enumerate() {
        let Some(source_body) = metadata.functions[function_index].body.as_ref() else {
            continue;
        };
        let function_number = to_u32(function_index)?;
        push_correspondence(
            &mut correspondence,
            &mut ordinal,
            KirBridgeCoordinateV1::Function {
                function: function_number,
            },
        )?;
        let live_function = live_functions
            .iter()
            .copied()
            .find(|live| origins.functions.get(live) == Some(&function_index))
            .ok_or(KirBridgeErrorV1::MalformedGraph)?;
        let Some(function_op) = Operation::get_op::<FuncOp>(live_function, context) else {
            return Err(KirBridgeErrorV1::MalformedGraph);
        };
        let function_type = function_op.get_type(context);
        let function_type_ref = function_type.deref(context);
        let Some(function_type) = function_type_ref.downcast_ref::<FunctionType>() else {
            return Err(KirBridgeErrorV1::MalformedGraph);
        };
        require_function_type_matches(context, function_type, &function.signature)?;
        if function.signature.parameters.len() != source_body.parameters.len()
            || live_function.deref(context).num_regions() != 1
        {
            return Err(KirBridgeErrorV1::MalformedGraph);
        }
        let live_blocks: Vec<_> = live_function
            .deref(context)
            .get_region(0)
            .deref(context)
            .iter(context)
            .collect();
        if live_blocks.is_empty() {
            return Err(KirBridgeErrorV1::MalformedGraph);
        }

        let mut reverse_values = HashMap::new();
        let mut reverse_blocks = HashMap::new();
        let mut occupied = BTreeSet::new();
        let reserved_block_origins = live_blocks
            .iter()
            .filter_map(|block| origins.blocks.get(block))
            .filter(|(origin_function, _)| *origin_function == function_index)
            .map(|(_, block)| *block)
            .collect::<BTreeSet<_>>();
        let mut reserved_origins = BTreeSet::new();
        for block in &live_blocks {
            for value in block.deref(context).arguments() {
                if let Some(origin) = origins.values.get(&value) {
                    reserved_origins.insert(*origin);
                }
            }
            for operation in block.deref(context).iter(context) {
                for value in operation.deref(context).results() {
                    if let Some(origin) = origins.values.get(&value) {
                        reserved_origins.insert(*origin);
                    }
                }
            }
        }
        let mut occupied_blocks = BTreeSet::new();
        let mut next_id = 0_u32;
        let mut next_block_id = 0_u32;
        let entry = *live_blocks
            .first()
            .ok_or(KirBridgeErrorV1::MalformedGraph)?;
        if entry.deref(context).get_num_arguments() < function.signature.parameters.len() {
            return Err(KirBridgeErrorV1::MalformedGraph);
        }
        let mut body_parameters = Vec::with_capacity(function.signature.parameters.len());
        for (index, ty) in function.signature.parameters.iter().enumerate() {
            let live = entry.deref(context).get_argument(index);
            if live.get_type(context) != type_to_pliron(context, ty)? {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            let id = origin_or_fresh_value_id(
                origins,
                live,
                &occupied,
                &reserved_origins,
                &mut next_id,
            )?;
            if reverse_values.insert(live, id).is_some() || !occupied.insert(id) {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            body_parameters.push(id);
        }
        for (block_index, live) in live_blocks.iter().enumerate() {
            let origin = origins
                .blocks
                .get(live)
                .filter(|(origin_function, _)| *origin_function == function_index)
                .map(|(_, block)| *block)
                .filter(|block| !occupied_blocks.contains(block));
            let block_id = match origin {
                Some(origin) => origin,
                None => fresh_block_id(
                    &occupied_blocks,
                    &reserved_block_origins,
                    &mut next_block_id,
                )?,
            };
            reverse_blocks.insert(*live, block_id);
            occupied_blocks.insert(block_id);
            let offset = usize::from(block_index == 0) * source_body.parameters.len();
            for live_value in live.deref(context).arguments().skip(offset) {
                let id = origin_or_fresh_value_id(
                    origins,
                    live_value,
                    &occupied,
                    &reserved_origins,
                    &mut next_id,
                )?;
                if reverse_values.insert(live_value, id).is_some() || !occupied.insert(id) {
                    return Err(KirBridgeErrorV1::MalformedGraph);
                }
            }
        }

        // Bind every live result before reading any operands. Physical block
        // order is not required to be dominance order in canonical KIR.
        for live in &live_blocks {
            let live_operations = live.deref(context).iter(context).collect::<Vec<_>>();
            let (_, body_operations) = live_operations
                .split_last()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?;
            for live_operation in body_operations {
                for live_result in live_operation.deref(context).results() {
                    let id = origin_or_fresh_value_id(
                        origins,
                        live_result,
                        &occupied,
                        &reserved_origins,
                        &mut next_id,
                    )?;
                    if reverse_values.insert(live_result, id).is_some() || !occupied.insert(id) {
                        return Err(KirBridgeErrorV1::MalformedGraph);
                    }
                }
            }
        }

        let mut blocks = Vec::with_capacity(live_blocks.len());
        for (block_index, live) in live_blocks.iter().enumerate() {
            let block_number = to_u32(block_index)?;
            push_correspondence(
                &mut correspondence,
                &mut ordinal,
                KirBridgeCoordinateV1::Block {
                    function: function_number,
                    block: block_number,
                },
            )?;
            let offset = usize::from(block_index == 0) * source_body.parameters.len();
            let mut block = fe2o3_kernel_ir::BasicBlock::new(block_id_for(&reverse_blocks, *live)?);
            block.parameters = live
                .deref(context)
                .arguments()
                .skip(offset)
                .map(|argument| {
                    Ok(fe2o3_kernel_ir::ValueDef::new(
                        id_for(&reverse_values, argument)?,
                        type_from_live_value(context, argument, origins)?,
                    ))
                })
                .collect::<Result<Vec<_>, KirBridgeErrorV1>>()?;
            let live_operations: Vec<_> = live.deref(context).iter(context).collect();
            let (live_terminator, body_operations) = live_operations
                .split_last()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?;
            for (operation_index, live_operation) in body_operations.iter().enumerate() {
                let coordinate = KirBridgeCoordinateV1::Operation {
                    function: function_number,
                    block: block_number,
                    operation: to_u32(operation_index)?,
                };
                push_correspondence(&mut correspondence, &mut ordinal, coordinate)?;
                let result_count = live_operation.deref(context).get_num_results();
                let mut results = Vec::with_capacity(result_count);
                for index in 0..result_count {
                    let live_result = live_operation.deref(context).get_result(index);
                    let id = id_for(&reverse_values, live_result)?;
                    results.push(fe2o3_kernel_ir::ValueDef::new(
                        id,
                        type_from_live_value(context, live_result, origins)?,
                    ));
                }
                let kind = extract_any_operation(
                    context,
                    *live_operation,
                    &reverse_values,
                    coordinate,
                    graph_epoch,
                )?;
                block.operations.push(KirOperation::new(results, kind));
            }
            let coordinate = KirBridgeCoordinateV1::Terminator {
                function: function_number,
                block: block_number,
            };
            push_correspondence(&mut correspondence, &mut ordinal, coordinate)?;
            block.terminator = Some(extract_terminator(
                context,
                *live_terminator,
                &reverse_values,
                &reverse_blocks,
                coordinate,
                graph_epoch,
            )?);
            blocks.push(block);
        }
        function.body = Some(fe2o3_kernel_ir::FunctionBody {
            parameters: body_parameters,
            blocks,
        });
    }
    Ok((output, correspondence))
}

fn fresh_value_id(
    occupied: &BTreeSet<ValueId>,
    reserved: &BTreeSet<ValueId>,
    next: &mut u32,
) -> Result<ValueId, KirBridgeErrorV1> {
    loop {
        let candidate = ValueId(*next);
        *next = next.checked_add(1).ok_or(KirBridgeErrorV1::SizeOverflow)?;
        if !occupied.contains(&candidate) && !reserved.contains(&candidate) {
            return Ok(candidate);
        }
    }
}

fn origin_or_fresh_value_id(
    origins: &KirBridgeOriginsV1,
    live: Value,
    occupied: &BTreeSet<ValueId>,
    reserved: &BTreeSet<ValueId>,
    next: &mut u32,
) -> Result<ValueId, KirBridgeErrorV1> {
    match origins
        .values
        .get(&live)
        .copied()
        .filter(|id| !occupied.contains(id))
    {
        Some(origin) => Ok(origin),
        None => fresh_value_id(occupied, reserved, next),
    }
}

fn fresh_block_id(
    occupied: &BTreeSet<BlockId>,
    reserved: &BTreeSet<BlockId>,
    next: &mut u32,
) -> Result<BlockId, KirBridgeErrorV1> {
    loop {
        let candidate = BlockId(*next);
        *next = next.checked_add(1).ok_or(KirBridgeErrorV1::SizeOverflow)?;
        if !occupied.contains(&candidate) && !reserved.contains(&candidate) {
            return Ok(candidate);
        }
    }
}

fn extract_canonical_operation<T>(
    context: &Context,
    live: Ptr<Operation>,
    reverse: &HashMap<Value, ValueId>,
    _coordinate: KirBridgeCoordinateV1,
    graph_epoch: [u8; 32],
) -> Result<KirOperation, KirBridgeErrorV1>
where
    T: CanonicalKirOperationCarrier + 'static,
{
    let operation =
        Operation::get_op::<T>(live, context).ok_or(KirBridgeErrorV1::MalformedGraph)?;
    operation
        .verify(context)
        .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
    // Optimization may move an operation to a different live ordinal.  The
    // carrier coordinate names its authenticated source operation, while the
    // `coordinate` argument names the reconstructed output position.  Keep
    // those identities distinct and let the typed carrier verifier bind its
    // source coordinate to the payload identity.
    if operation.canonical_graph_epoch(context) != Some(graph_epoch) {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let mut contract = operation
        .canonical_contract(context)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
    let raw = live.deref(context);
    if contract.results.len() != raw.get_num_results()
        || contract.results.iter().enumerate().any(|(index, result)| {
            type_to_pliron(context, &result.ty).ok() != Some(raw.get_type(index))
        })
    {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let operands = ids_for(reverse, operation.canonical_operands(context))?;
    contract.kind = remap_canonical_operation(contract.clone(), &operands)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?
        .kind;
    Ok(contract)
}

fn extract_expected_canonical_operation<T>(
    context: &Context,
    live: Ptr<Operation>,
    expected: &KirOperation,
    reverse: &HashMap<Value, ValueId>,
    coordinate: KirBridgeCoordinateV1,
    graph_epoch: [u8; 32],
) -> Result<OperationKind, KirBridgeErrorV1>
where
    T: CanonicalKirOperationCarrier + 'static,
{
    let actual = extract_canonical_operation::<T>(context, live, reverse, coordinate, graph_epoch)?;
    if &actual != expected {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    Ok(actual.kind)
}

fn extract_any_operation(
    context: &Context,
    live: Ptr<Operation>,
    reverse: &HashMap<Value, ValueId>,
    coordinate: KirBridgeCoordinateV1,
    graph_epoch: [u8; 32],
) -> Result<OperationKind, KirBridgeErrorV1> {
    let raw = live.deref(context);
    if let Some(operation) = Operation::get_op::<PlironConstantOp>(live, context) {
        return Ok(OperationKind::Constant(constant_from_pliron_untyped(
            context,
            &operation.value(context),
        )?));
    }
    if let Some(operation) = Operation::get_op::<BuiltinConstantOp>(live, context) {
        return Ok(OperationKind::Constant(constant_from_pliron_untyped(
            context,
            &operation.get_value(context),
        )?));
    }
    if let Some(operation) = Operation::get_op::<PlironUnaryOp>(live, context) {
        return Ok(OperationKind::Unary {
            op: unary_from_pliron(
                operation
                    .kind(context)
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?,
            ),
            operand: id_for(reverse, raw.get_operand(0))?,
        });
    }
    if let Some(operation) = Operation::get_op::<PlironBinaryOp>(live, context) {
        return Ok(OperationKind::Binary {
            op: binary_from_pliron(
                operation
                    .kind(context)
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?,
            ),
            lhs: id_for(reverse, raw.get_operand(0))?,
            rhs: id_for(reverse, raw.get_operand(1))?,
        });
    }
    if let Some(operation) = Operation::get_op::<PlironCompareOp>(live, context) {
        return Ok(OperationKind::Compare {
            predicate: compare_from_pliron(
                operation
                    .predicate(context)
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?,
            ),
            lhs: id_for(reverse, raw.get_operand(0))?,
            rhs: id_for(reverse, raw.get_operand(1))?,
        });
    }
    if let Some(operation) = Operation::get_op::<CastOp>(live, context) {
        return Ok(OperationKind::Cast {
            kind: cast_from_pliron(
                operation
                    .kind(context)
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?,
            ),
            value: id_for(reverse, raw.get_operand(0))?,
            to: type_from_pliron(context, raw.get_type(0))?,
        });
    }
    if Operation::is_op::<PlironSelectOp>(live, context) {
        return Ok(OperationKind::Select {
            condition: id_for(reverse, raw.get_operand(0))?,
            true_value: id_for(reverse, raw.get_operand(1))?,
            false_value: id_for(reverse, raw.get_operand(2))?,
        });
    }
    if let Some(operation) = Operation::get_op::<CallOp>(live, context) {
        return Ok(OperationKind::Call {
            callee: FunctionId::new(
                operation
                    .callee(context)
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?,
            ),
            arguments: ids_for(reverse, operation.arguments(context))?,
        });
    }
    if Operation::is_op::<SliceLengthOp>(live, context) {
        return Ok(OperationKind::SliceLength {
            slice: id_for(reverse, raw.get_operand(0))?,
        });
    }
    if Operation::is_op::<PlironGlobalCapabilityLengthOp>(live, context) {
        return Ok(OperationKind::SliceLength {
            slice: id_for(reverse, raw.get_operand(0))?,
        });
    }
    if Operation::is_op::<SliceDataOp>(live, context) {
        return Ok(OperationKind::SliceData {
            slice: id_for(reverse, raw.get_operand(0))?,
        });
    }
    if Operation::is_op::<PlironGlobalCapabilityDataOp>(live, context) {
        return Ok(OperationKind::SliceData {
            slice: id_for(reverse, raw.get_operand(0))?,
        });
    }
    if Operation::is_op::<GetElementPointerOp>(live, context) {
        return Ok(OperationKind::GetElementPointer {
            base: id_for(reverse, raw.get_operand(0))?,
            offset: id_for(reverse, raw.get_operand(1))?,
        });
    }
    if let Some(operation) = Operation::get_op::<LoadOp>(live, context) {
        return Ok(OperationKind::Load {
            pointer: id_for(reverse, raw.get_operand(0))?,
            access: memory_access_from_load(context, &operation)?,
        });
    }
    if let Some(operation) = Operation::get_op::<StoreOp>(live, context) {
        return Ok(OperationKind::Store {
            pointer: id_for(reverse, raw.get_operand(0))?,
            value: id_for(reverse, raw.get_operand(1))?,
            access: memory_access_from_store(context, &operation)?,
        });
    }
    macro_rules! extract_carrier {
        ($operation_type:ty) => {
            if Operation::is_op::<$operation_type>(live, context) {
                return Ok(extract_canonical_operation::<$operation_type>(
                    context,
                    live,
                    reverse,
                    coordinate,
                    graph_epoch,
                )?
                .kind);
            }
        };
    }
    extract_carrier!(PlironIntrinsicOp);
    extract_carrier!(PlironMemoryIntrinsicOp);
    extract_carrier!(PlironAllocaOp);
    extract_carrier!(PlironGuardedLoadOp);
    extract_carrier!(PlironGuardedStoreOp);
    extract_carrier!(PlironCanonicalBarrierOp);
    extract_carrier!(PlironAtomicOp);
    extract_carrier!(PlironCanonicalFenceOp);
    extract_carrier!(PlironWorkgroupBarrierOp);
    extract_carrier!(PlironWorkgroupMemoryOp);
    extract_carrier!(PlironMatrixOp);
    extract_carrier!(PlironGfx950LdsTransposeOp);
    extract_carrier!(PlironWaveOp);
    extract_carrier!(PlironInlineAssemblyOp);
    if Operation::is_op::<KernelContextIssueOp>(live, context) {
        return extract_kernel_context_issue(context, live, coordinate, graph_epoch);
    }
    if Operation::is_op::<PlironGlobalCapabilityBindOp>(live, context) {
        return extract_global_capability_bind(context, live, reverse);
    }
    if Operation::is_op::<PlironGlobalCapabilityIndexOp>(live, context) {
        return extract_global_capability_index(context, live, reverse);
    }
    if Operation::is_op::<PlironExecutionCapabilityOp>(live, context) {
        return extract_execution_capability(context, live, reverse, coordinate, graph_epoch);
    }
    Err(KirBridgeErrorV1::UnsupportedOperation { coordinate })
}

fn memory_access_from_load(
    context: &Context,
    operation: &LoadOp,
) -> Result<fe2o3_kernel_ir::MemoryAccess, KirBridgeErrorV1> {
    Ok(fe2o3_kernel_ir::MemoryAccess {
        address_space: address_space_from_pliron(
            operation
                .address_space(context)
                .ok_or(KirBridgeErrorV1::MalformedGraph)?,
        ),
        alignment: operation
            .alignment(context)
            .ok_or(KirBridgeErrorV1::MalformedGraph)?,
        volatile: operation
            .is_volatile(context)
            .ok_or(KirBridgeErrorV1::MalformedGraph)?,
    })
}

fn memory_access_from_store(
    context: &Context,
    operation: &StoreOp,
) -> Result<fe2o3_kernel_ir::MemoryAccess, KirBridgeErrorV1> {
    Ok(fe2o3_kernel_ir::MemoryAccess {
        address_space: address_space_from_pliron(
            operation
                .address_space(context)
                .ok_or(KirBridgeErrorV1::MalformedGraph)?,
        ),
        alignment: operation
            .alignment(context)
            .ok_or(KirBridgeErrorV1::MalformedGraph)?,
        volatile: operation
            .is_volatile(context)
            .ok_or(KirBridgeErrorV1::MalformedGraph)?,
    })
}

fn constant_from_pliron_untyped(
    context: &Context,
    attr: &AttrObj,
) -> Result<Constant, KirBridgeErrorV1> {
    let typed =
        attr_cast::<dyn TypedAttrInterface>(&**attr).ok_or(KirBridgeErrorV1::MalformedGraph)?;
    match type_from_pliron(context, typed.get_type(context))? {
        Type::Scalar(ScalarType::Bool) => Ok(Constant::Bool(
            attr.downcast_ref::<IntegerAttr>()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .value()
                .to_u8()
                != 0,
        )),
        Type::Scalar(ScalarType::I8) => Ok(Constant::I8(integer_value(attr)?.to_i8())),
        Type::Scalar(ScalarType::I16) => Ok(Constant::I16(integer_value(attr)?.to_i16())),
        Type::Scalar(ScalarType::I32) => Ok(Constant::I32(integer_value(attr)?.to_i32())),
        Type::Scalar(ScalarType::I64) => Ok(Constant::I64(integer_value(attr)?.to_i64())),
        Type::Scalar(ScalarType::U8) => Ok(Constant::U8(integer_value(attr)?.to_u8())),
        Type::Scalar(ScalarType::U16) => Ok(Constant::U16(integer_value(attr)?.to_u16())),
        Type::Scalar(ScalarType::U32) => Ok(Constant::U32(integer_value(attr)?.to_u32())),
        Type::Scalar(ScalarType::U64) => Ok(Constant::U64(integer_value(attr)?.to_u64())),
        Type::Scalar(ScalarType::Index) => Ok(Constant::Index(
            attr.downcast_ref::<IndexAttr>()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .0,
        )),
        Type::Scalar(ScalarType::F16) => Ok(Constant::F16Bits(
            attr.downcast_ref::<FPHalfAttr>()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .0
                .to_bits()
                .try_into()
                .map_err(|_| KirBridgeErrorV1::MalformedGraph)?,
        )),
        Type::Scalar(ScalarType::Bf16) => Ok(Constant::Bf16Bits(
            attr.downcast_ref::<BFloat16Attr>()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .0,
        )),
        Type::Scalar(ScalarType::F32) => Ok(Constant::F32Bits(
            attr.downcast_ref::<FPSingleAttr>()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .0
                .to_bits()
                .try_into()
                .map_err(|_| KirBridgeErrorV1::MalformedGraph)?,
        )),
        Type::Scalar(ScalarType::F64) => Ok(Constant::F64Bits(
            attr.downcast_ref::<FPDoubleAttr>()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .0
                .to_bits()
                .try_into()
                .map_err(|_| KirBridgeErrorV1::MalformedGraph)?,
        )),
        _ => Err(KirBridgeErrorV1::UnsupportedType),
    }
}

fn integer_value(attr: &AttrObj) -> Result<APInt, KirBridgeErrorV1> {
    attr.downcast_ref::<IntegerAttr>()
        .map(IntegerAttr::value)
        .ok_or(KirBridgeErrorV1::MalformedGraph)
}

fn extract_module_graph(
    context: &Context,
    root: Ptr<Operation>,
    metadata: &Module,
    canonical_version: KirBridgeCanonicalVersionV1,
    graph_epoch: [u8; 32],
) -> Result<Module, KirBridgeErrorV1> {
    if !Operation::is_op::<ModuleOp>(root, context) {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let root_raw = root.deref(context);
    if root_raw.num_regions() != 1 {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let root_region = root_raw.get_region(0);
    let root_blocks: Vec<_> = root_region.deref(context).iter(context).collect();
    let [root_block] = root_blocks.as_slice() else {
        return Err(KirBridgeErrorV1::MalformedGraph);
    };
    let live_functions = validated_live_functions(
        context,
        *root_block,
        metadata,
        canonical_version,
        graph_epoch,
    )?;
    let expected_definitions = metadata
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .count();
    if live_functions.len() != expected_definitions {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }

    let mut output = metadata.clone();
    let mut live_index = 0_usize;
    for (function_index, output_function) in output.functions.iter_mut().enumerate() {
        let Some(source_body) = metadata.functions[function_index].body.as_ref() else {
            continue;
        };
        let live_function = live_functions[live_index];
        live_index += 1;
        let Some(function_op) = Operation::get_op::<FuncOp>(live_function, context) else {
            return Err(KirBridgeErrorV1::MalformedGraph);
        };
        let function_type = function_op.get_type(context);
        let function_type_ref = function_type.deref(context);
        let Some(function_type) = function_type_ref.downcast_ref::<FunctionType>() else {
            return Err(KirBridgeErrorV1::MalformedGraph);
        };
        require_function_type_matches(context, function_type, &output_function.signature)?;

        let raw_function = live_function.deref(context);
        if raw_function.num_regions() != 1 {
            return Err(KirBridgeErrorV1::MalformedGraph);
        }
        let live_blocks: Vec<_> = raw_function
            .get_region(0)
            .deref(context)
            .iter(context)
            .collect();
        if live_blocks.len() != source_body.blocks.len() {
            return Err(KirBridgeErrorV1::MalformedGraph);
        }

        let mut reverse_values = HashMap::new();
        let mut reverse_blocks = HashMap::new();
        for (block_index, (source_block, live_block)) in
            source_body.blocks.iter().zip(&live_blocks).enumerate()
        {
            reverse_blocks.insert(*live_block, source_block.id);
            let parameter_offset = usize::from(block_index == 0) * source_body.parameters.len();
            let expected_arguments = parameter_offset
                .checked_add(source_block.parameters.len())
                .ok_or(KirBridgeErrorV1::SizeOverflow)?;
            if live_block.deref(context).get_num_arguments() != expected_arguments {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            if block_index == 0 {
                for (index, value_id) in source_body.parameters.iter().enumerate() {
                    bind_live_value(
                        context,
                        &mut reverse_values,
                        live_block.deref(context).get_argument(index),
                        *value_id,
                        &metadata.functions[function_index].signature.parameters[index],
                    )?;
                }
            }
            for (index, parameter) in source_block.parameters.iter().enumerate() {
                bind_live_value(
                    context,
                    &mut reverse_values,
                    live_block
                        .deref(context)
                        .get_argument(parameter_offset + index),
                    parameter.id,
                    &parameter.ty,
                )?;
            }
            let live_operations: Vec<_> = live_block.deref(context).iter(context).collect();
            if live_operations.len() != source_block.operations.len() + 1 {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            for (source_operation, live_operation) in
                source_block.operations.iter().zip(&live_operations)
            {
                let raw = live_operation.deref(context);
                if raw.get_num_results() != source_operation.results.len() {
                    return Err(KirBridgeErrorV1::MalformedGraph);
                }
                for (index, result) in source_operation.results.iter().enumerate() {
                    bind_live_value(
                        context,
                        &mut reverse_values,
                        raw.get_result(index),
                        result.id,
                        &result.ty,
                    )?;
                }
            }
        }

        let output_body = output_function
            .body
            .as_mut()
            .ok_or(KirBridgeErrorV1::MalformedGraph)?;
        for (block_index, ((source_block, output_block), live_block)) in source_body
            .blocks
            .iter()
            .zip(&mut output_body.blocks)
            .zip(&live_blocks)
            .enumerate()
        {
            let live_operations: Vec<_> = live_block.deref(context).iter(context).collect();
            for (operation_index, ((source_operation, output_operation), live_operation)) in
                source_block
                    .operations
                    .iter()
                    .zip(&mut output_block.operations)
                    .zip(&live_operations)
                    .enumerate()
            {
                output_operation.kind = extract_operation(
                    context,
                    *live_operation,
                    source_operation,
                    &reverse_values,
                    KirBridgeCoordinateV1::Operation {
                        function: to_u32(function_index)?,
                        block: to_u32(block_index)?,
                        operation: to_u32(operation_index)?,
                    },
                    graph_epoch,
                )?;
                for (index, result) in output_operation.results.iter().enumerate() {
                    if live_operation.deref(context).get_type(index)
                        != type_to_pliron(context, &result.ty)?
                    {
                        return Err(KirBridgeErrorV1::MalformedGraph);
                    }
                }
            }
            output_block.terminator = Some(extract_terminator(
                context,
                *live_operations
                    .last()
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                &reverse_values,
                &reverse_blocks,
                KirBridgeCoordinateV1::Terminator {
                    function: to_u32(function_index)?,
                    block: to_u32(block_index)?,
                },
                graph_epoch,
            )?);
            for (index, parameter) in output_block.parameters.iter().enumerate() {
                let offset = usize::from(output_block.id == source_body.blocks[0].id)
                    * source_body.parameters.len();
                if live_block
                    .deref(context)
                    .get_argument(offset + index)
                    .get_type(context)
                    != type_to_pliron(context, &parameter.ty)?
                {
                    return Err(KirBridgeErrorV1::MalformedGraph);
                }
            }
        }
    }
    Ok(output)
}

fn bind_live_value(
    context: &Context,
    reverse: &mut HashMap<Value, ValueId>,
    live: Value,
    id: ValueId,
    expected_type: &Type,
) -> Result<(), KirBridgeErrorV1> {
    if live.get_type(context) != type_to_pliron(context, expected_type)?
        || reverse.insert(live, id).is_some()
    {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    Ok(())
}

fn require_function_type_matches(
    context: &Context,
    live: &FunctionType,
    expected: &fe2o3_kernel_ir::Signature,
) -> Result<(), KirBridgeErrorV1> {
    let arguments = live.arg_types();
    let results = live.res_types();
    if arguments.len() != expected.parameters.len()
        || results.len() != expected.results.len()
        || arguments
            .into_iter()
            .zip(&expected.parameters)
            .any(|(live, expected)| type_to_pliron(context, expected) != Ok(live))
        || results
            .into_iter()
            .zip(&expected.results)
            .any(|(live, expected)| type_to_pliron(context, expected) != Ok(live))
    {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    Ok(())
}

fn type_from_live_value(
    context: &Context,
    live: Value,
    origins: &KirBridgeOriginsV1,
) -> Result<Type, KirBridgeErrorV1> {
    if let Some(logical) = origins.logical_types.get(&live) {
        if live.get_type(context) != type_to_pliron(context, logical)? {
            return Err(KirBridgeErrorV1::MalformedGraph);
        }
        return Ok(logical.clone());
    }
    type_from_pliron(context, live.get_type(context))
}

fn id_for(reverse: &HashMap<Value, ValueId>, value: Value) -> Result<ValueId, KirBridgeErrorV1> {
    reverse
        .get(&value)
        .copied()
        .ok_or(KirBridgeErrorV1::MalformedGraph)
}

fn ids_for(
    reverse: &HashMap<Value, ValueId>,
    values: impl IntoIterator<Item = Value>,
) -> Result<Vec<ValueId>, KirBridgeErrorV1> {
    values
        .into_iter()
        .map(|value| id_for(reverse, value))
        .collect()
}

fn extract_operation(
    context: &Context,
    live: Ptr<Operation>,
    expected: &KirOperation,
    reverse: &HashMap<Value, ValueId>,
    coordinate: KirBridgeCoordinateV1,
    graph_epoch: [u8; 32],
) -> Result<OperationKind, KirBridgeErrorV1> {
    let raw = live.deref(context);
    match &expected.kind {
        OperationKind::Constant(expected) => {
            let Some(operation) = Operation::get_op::<PlironConstantOp>(live, context) else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            Ok(OperationKind::Constant(constant_from_pliron(
                context,
                &operation.value(context),
                expected,
            )?))
        }
        OperationKind::Unary { .. } => {
            let Some(operation) = Operation::get_op::<PlironUnaryOp>(live, context) else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            Ok(OperationKind::Unary {
                op: unary_from_pliron(
                    operation
                        .kind(context)
                        .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                ),
                operand: id_for(reverse, raw.get_operand(0))?,
            })
        }
        OperationKind::Binary { .. } => {
            let Some(operation) = Operation::get_op::<PlironBinaryOp>(live, context) else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            Ok(OperationKind::Binary {
                op: binary_from_pliron(
                    operation
                        .kind(context)
                        .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                ),
                lhs: id_for(reverse, raw.get_operand(0))?,
                rhs: id_for(reverse, raw.get_operand(1))?,
            })
        }
        OperationKind::Compare { .. } => {
            let Some(operation) = Operation::get_op::<PlironCompareOp>(live, context) else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            Ok(OperationKind::Compare {
                predicate: compare_from_pliron(
                    operation
                        .predicate(context)
                        .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                ),
                lhs: id_for(reverse, raw.get_operand(0))?,
                rhs: id_for(reverse, raw.get_operand(1))?,
            })
        }
        OperationKind::Cast { .. } => {
            let Some(operation) = Operation::get_op::<CastOp>(live, context) else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            Ok(OperationKind::Cast {
                kind: cast_from_pliron(
                    operation
                        .kind(context)
                        .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                ),
                value: id_for(reverse, raw.get_operand(0))?,
                to: type_from_pliron(context, raw.get_type(0))?,
            })
        }
        OperationKind::Select { .. } => {
            if !Operation::is_op::<PlironSelectOp>(live, context) {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            Ok(OperationKind::Select {
                condition: id_for(reverse, raw.get_operand(0))?,
                true_value: id_for(reverse, raw.get_operand(1))?,
                false_value: id_for(reverse, raw.get_operand(2))?,
            })
        }
        OperationKind::Call { .. } => {
            let Some(operation) = Operation::get_op::<CallOp>(live, context) else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            Ok(OperationKind::Call {
                callee: FunctionId::new(
                    operation
                        .callee(context)
                        .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                ),
                arguments: ids_for(reverse, operation.arguments(context))?,
            })
        }
        OperationKind::SliceLength { .. } => {
            if !Operation::is_op::<SliceLengthOp>(live, context)
                && !Operation::is_op::<PlironGlobalCapabilityLengthOp>(live, context)
            {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            Ok(OperationKind::SliceLength {
                slice: id_for(reverse, raw.get_operand(0))?,
            })
        }
        OperationKind::SliceData { .. } => {
            if !Operation::is_op::<SliceDataOp>(live, context)
                && !Operation::is_op::<PlironGlobalCapabilityDataOp>(live, context)
            {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            Ok(OperationKind::SliceData {
                slice: id_for(reverse, raw.get_operand(0))?,
            })
        }
        OperationKind::GetElementPointer { .. } => {
            if !Operation::is_op::<GetElementPointerOp>(live, context) {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            Ok(OperationKind::GetElementPointer {
                base: id_for(reverse, raw.get_operand(0))?,
                offset: id_for(reverse, raw.get_operand(1))?,
            })
        }
        OperationKind::Load { .. } => {
            let Some(operation) = Operation::get_op::<LoadOp>(live, context) else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            Ok(OperationKind::Load {
                pointer: id_for(reverse, raw.get_operand(0))?,
                access: fe2o3_kernel_ir::MemoryAccess {
                    address_space: address_space_from_pliron(
                        operation
                            .address_space(context)
                            .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                    ),
                    alignment: operation
                        .alignment(context)
                        .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                    volatile: operation
                        .is_volatile(context)
                        .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                },
            })
        }
        OperationKind::Store { .. } => {
            let Some(operation) = Operation::get_op::<StoreOp>(live, context) else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            Ok(OperationKind::Store {
                pointer: id_for(reverse, raw.get_operand(0))?,
                value: id_for(reverse, raw.get_operand(1))?,
                access: fe2o3_kernel_ir::MemoryAccess {
                    address_space: address_space_from_pliron(
                        operation
                            .address_space(context)
                            .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                    ),
                    alignment: operation
                        .alignment(context)
                        .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                    volatile: operation
                        .is_volatile(context)
                        .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                },
            })
        }
        OperationKind::KernelContextIssue(expected) => {
            let actual = extract_kernel_context_issue(context, live, coordinate, graph_epoch)?;
            if actual != OperationKind::KernelContextIssue(*expected) {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            Ok(actual)
        }
        OperationKind::GlobalCapabilityBind(expected) => {
            let actual = extract_global_capability_bind(context, live, reverse)?;
            if actual != OperationKind::GlobalCapabilityBind(*expected) {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            Ok(actual)
        }
        OperationKind::GlobalCapabilityIndex(expected) => {
            let actual = extract_global_capability_index(context, live, reverse)?;
            if actual != OperationKind::GlobalCapabilityIndex(*expected) {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            Ok(actual)
        }
        OperationKind::ExecutionCapability(expected) => {
            let actual =
                extract_execution_capability(context, live, reverse, coordinate, graph_epoch)?;
            let OperationKind::ExecutionCapability(actual_contract) = &actual else {
                return Err(KirBridgeErrorV1::MalformedGraph);
            };
            if actual_contract != expected {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            Ok(actual)
        }
        OperationKind::Intrinsic(_) => extract_expected_canonical_operation::<PlironIntrinsicOp>(
            context,
            live,
            expected,
            reverse,
            coordinate,
            graph_epoch,
        ),
        OperationKind::MemoryIntrinsic(_) => {
            extract_expected_canonical_operation::<PlironMemoryIntrinsicOp>(
                context,
                live,
                expected,
                reverse,
                coordinate,
                graph_epoch,
            )
        }
        OperationKind::Alloca { .. } => extract_expected_canonical_operation::<PlironAllocaOp>(
            context,
            live,
            expected,
            reverse,
            coordinate,
            graph_epoch,
        ),
        OperationKind::GuardedLoad { .. } => {
            extract_expected_canonical_operation::<PlironGuardedLoadOp>(
                context,
                live,
                expected,
                reverse,
                coordinate,
                graph_epoch,
            )
        }
        OperationKind::GuardedStore { .. } => {
            extract_expected_canonical_operation::<PlironGuardedStoreOp>(
                context,
                live,
                expected,
                reverse,
                coordinate,
                graph_epoch,
            )
        }
        OperationKind::Barrier(_) => {
            extract_expected_canonical_operation::<PlironCanonicalBarrierOp>(
                context,
                live,
                expected,
                reverse,
                coordinate,
                graph_epoch,
            )
        }
        OperationKind::Atomic(_) => extract_expected_canonical_operation::<PlironAtomicOp>(
            context,
            live,
            expected,
            reverse,
            coordinate,
            graph_epoch,
        ),
        OperationKind::Fence(_) => extract_expected_canonical_operation::<PlironCanonicalFenceOp>(
            context,
            live,
            expected,
            reverse,
            coordinate,
            graph_epoch,
        ),
        OperationKind::WorkgroupBarrier(_) => {
            extract_expected_canonical_operation::<PlironWorkgroupBarrierOp>(
                context,
                live,
                expected,
                reverse,
                coordinate,
                graph_epoch,
            )
        }
        OperationKind::WorkgroupMemory(_) => {
            extract_expected_canonical_operation::<PlironWorkgroupMemoryOp>(
                context,
                live,
                expected,
                reverse,
                coordinate,
                graph_epoch,
            )
        }
        OperationKind::Matrix(_) => extract_expected_canonical_operation::<PlironMatrixOp>(
            context,
            live,
            expected,
            reverse,
            coordinate,
            graph_epoch,
        ),
        OperationKind::Gfx950LdsTranspose(_) => {
            extract_expected_canonical_operation::<PlironGfx950LdsTransposeOp>(
                context,
                live,
                expected,
                reverse,
                coordinate,
                graph_epoch,
            )
        }
        OperationKind::Wave(_) => extract_expected_canonical_operation::<PlironWaveOp>(
            context,
            live,
            expected,
            reverse,
            coordinate,
            graph_epoch,
        ),
        OperationKind::InlineAssembly(_) => {
            extract_expected_canonical_operation::<PlironInlineAssemblyOp>(
                context,
                live,
                expected,
                reverse,
                coordinate,
                graph_epoch,
            )
        }
    }
}

fn extract_execution_capability(
    context: &Context,
    live: Ptr<Operation>,
    reverse: &HashMap<Value, ValueId>,
    coordinate: KirBridgeCoordinateV1,
    graph_epoch: [u8; 32],
) -> Result<OperationKind, KirBridgeErrorV1> {
    let operation = Operation::get_op::<PlironExecutionCapabilityOp>(live, context)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
    operation
        .verify(context)
        .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
    let mut contract = operation
        .contract(context)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
    let operation_identity = execution_operation_identity(&contract)?;
    if operation.graph_epoch(context) != Some(graph_epoch)
        || operation.coordinate(context) != Some(source_coordinate_attr(coordinate)?)
        || operation.operation_identity(context) != Some(operation_identity)
        || operation.source_operation_identity(context) != Some(contract.source.operation)
    {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    contract.operands = ids_for(reverse, operation.operands(context))?;
    Ok(OperationKind::ExecutionCapability(contract))
}

fn extract_global_capability_bind(
    context: &Context,
    live: Ptr<Operation>,
    reverse: &HashMap<Value, ValueId>,
) -> Result<OperationKind, KirBridgeErrorV1> {
    let operation = Operation::get_op::<PlironGlobalCapabilityBindOp>(live, context)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
    operation
        .verify(context)
        .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
    let raw = live.deref(context);
    Ok(OperationKind::GlobalCapabilityBind(
        fe2o3_kernel_ir::GlobalCapabilityBindV1 {
            context: id_for(reverse, raw.get_operand(0))?,
            physical: id_for(reverse, raw.get_operand(1))?,
        },
    ))
}

fn extract_global_capability_index(
    context: &Context,
    live: Ptr<Operation>,
    reverse: &HashMap<Value, ValueId>,
) -> Result<OperationKind, KirBridgeErrorV1> {
    let operation = Operation::get_op::<PlironGlobalCapabilityIndexOp>(live, context)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
    operation
        .verify(context)
        .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
    let index_space = match operation
        .role(context)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?
    {
        fe2o3_kernel_ir::GlobalCapabilityRoleV1::ReadOnly
        | fe2o3_kernel_ir::GlobalCapabilityRoleV1::ExclusiveReadWrite => None,
        fe2o3_kernel_ir::GlobalCapabilityRoleV1::DisjointWrite(contract) => Some(contract),
    };
    let raw = live.deref(context);
    Ok(OperationKind::GlobalCapabilityIndex(
        fe2o3_kernel_ir::GlobalCapabilityIndexV1 {
            capability: id_for(reverse, raw.get_operand(0))?,
            index: id_for(reverse, raw.get_operand(1))?,
            index_space,
        },
    ))
}

fn extract_kernel_context_issue(
    context: &Context,
    live: Ptr<Operation>,
    coordinate: KirBridgeCoordinateV1,
    graph_epoch: [u8; 32],
) -> Result<OperationKind, KirBridgeErrorV1> {
    let operation = Operation::get_op::<KernelContextIssueOp>(live, context)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
    let source_coordinate = source_coordinate_attr(coordinate)?;
    let identities = operation
        .source_identities(context)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
    let source = fe2o3_kernel_ir::KernelContextSourceIdentityV1::new(
        identities[0],
        identities[1],
        identities[2],
        identities[3],
    );
    if operation.coordinate(context) != Some(source_coordinate)
        || operation.epoch(context) != Some(graph_epoch)
        || operation.operation_identity(context)
            != Some(context_operation_identity(graph_epoch, coordinate))
    {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    Ok(OperationKind::KernelContextIssue(
        fe2o3_kernel_ir::KernelContextIssueV1::new(source),
    ))
}

fn extract_canonical_switch<T>(
    context: &Context,
    live: Ptr<Operation>,
    reverse_values: &HashMap<Value, ValueId>,
    reverse_blocks: &HashMap<Ptr<BasicBlock>, BlockId>,
    coordinate: KirBridgeCoordinateV1,
    graph_epoch: [u8; 32],
) -> Result<Terminator, KirBridgeErrorV1>
where
    T: CanonicalKirSwitchCarrier + 'static,
{
    let operation =
        Operation::get_op::<T>(live, context).ok_or(KirBridgeErrorV1::MalformedGraph)?;
    operation
        .verify(context)
        .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
    let KirBridgeCoordinateV1::Terminator { function, block } = coordinate else {
        return Err(KirBridgeErrorV1::MalformedGraph);
    };
    if operation.canonical_coordinate(context) != Some(terminator_coordinate_attr(function, block))
        || operation.canonical_graph_epoch(context) != Some(graph_epoch)
    {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let raw = live.deref(context);
    let successors = raw
        .successors()
        .map(|block| block_id_for(reverse_blocks, block))
        .collect::<Result<Vec<_>, _>>()?;
    let successor_arguments = (0..raw.get_num_successors())
        .map(|index| ids_for(reverse_values, operation.successor_operands(context, index)))
        .collect::<Result<Vec<_>, _>>()?;
    remap_canonical_switch(
        operation
            .canonical_terminator(context)
            .ok_or(KirBridgeErrorV1::MalformedGraph)?,
        id_for(
            reverse_values,
            operation
                .canonical_selector(context)
                .ok_or(KirBridgeErrorV1::MalformedGraph)?,
        )?,
        &successors,
        &successor_arguments,
    )
    .ok_or(KirBridgeErrorV1::MalformedGraph)
}

fn extract_terminator(
    context: &Context,
    live: Ptr<Operation>,
    reverse_values: &HashMap<Value, ValueId>,
    reverse_blocks: &HashMap<Ptr<BasicBlock>, BlockId>,
    coordinate: KirBridgeCoordinateV1,
    graph_epoch: [u8; 32],
) -> Result<Terminator, KirBridgeErrorV1> {
    let raw = live.deref(context);
    if let Some(operation) = Operation::get_op::<BranchOp>(live, context) {
        return Ok(Terminator::Branch {
            target: block_id_for(reverse_blocks, raw.get_successor(0))?,
            arguments: ids_for(reverse_values, operation.successor_operands(context, 0))?,
        });
    }
    if let Some(operation) = Operation::get_op::<CondBranchOp>(live, context) {
        return Ok(Terminator::ConditionalBranch {
            condition: id_for(reverse_values, operation.condition(context))?,
            then_target: block_id_for(reverse_blocks, raw.get_successor(0))?,
            then_arguments: ids_for(reverse_values, operation.successor_operands(context, 0))?,
            else_target: block_id_for(reverse_blocks, raw.get_successor(1))?,
            else_arguments: ids_for(reverse_values, operation.successor_operands(context, 1))?,
        });
    }
    if let Some(operation) = Operation::get_op::<ReturnOp>(live, context) {
        return Ok(Terminator::Return {
            values: ids_for(reverse_values, operation.values(context))?,
        });
    }
    if Operation::is_op::<RankedReturnOp>(live, context) {
        return Ok(Terminator::Return { values: vec![] });
    }
    if Operation::is_op::<PlironSwitchOp>(live, context) {
        return extract_canonical_switch::<PlironSwitchOp>(
            context,
            live,
            reverse_values,
            reverse_blocks,
            coordinate,
            graph_epoch,
        );
    }
    if Operation::is_op::<PlironIntegerSwitchOp>(live, context) {
        return extract_canonical_switch::<PlironIntegerSwitchOp>(
            context,
            live,
            reverse_values,
            reverse_blocks,
            coordinate,
            graph_epoch,
        );
    }
    if let Some(operation) = Operation::get_op::<PlironUnreachableOp>(live, context) {
        operation
            .verify(context)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let KirBridgeCoordinateV1::Terminator { function, block } = coordinate else {
            return Err(KirBridgeErrorV1::MalformedGraph);
        };
        if operation.coordinate(context) != Some(terminator_coordinate_attr(function, block))
            || operation.graph_epoch(context) != Some(graph_epoch)
            || operation.contract(context) != Some(Terminator::Unreachable)
        {
            return Err(KirBridgeErrorV1::MalformedGraph);
        }
        return Ok(Terminator::Unreachable);
    }
    Err(KirBridgeErrorV1::MalformedGraph)
}

fn block_id_for(
    reverse: &HashMap<Ptr<BasicBlock>, BlockId>,
    block: Ptr<BasicBlock>,
) -> Result<BlockId, KirBridgeErrorV1> {
    reverse
        .get(&block)
        .copied()
        .ok_or(KirBridgeErrorV1::MalformedGraph)
}

fn type_from_pliron(context: &Context, ty: TypeHandle) -> Result<Type, KirBridgeErrorV1> {
    let raw = ty.deref(context);
    if raw.is::<UnitType>() {
        return Ok(Type::Unit);
    }
    if let Some(integer) = raw.downcast_ref::<IntegerType>() {
        return Ok(Type::Scalar(
            match (integer.width(), integer.signedness()) {
                (1, Signedness::Signless) => ScalarType::Bool,
                (8, Signedness::Signed) => ScalarType::I8,
                (16, Signedness::Signed) => ScalarType::I16,
                (32, Signedness::Signed) => ScalarType::I32,
                (64, Signedness::Signed) => ScalarType::I64,
                (128, Signedness::Signed) => ScalarType::I128,
                (8, Signedness::Unsigned) => ScalarType::U8,
                (16, Signedness::Unsigned) => ScalarType::U16,
                (32, Signedness::Unsigned) => ScalarType::U32,
                (64, Signedness::Unsigned) => ScalarType::U64,
                (128, Signedness::Unsigned) => ScalarType::U128,
                _ => return Err(KirBridgeErrorV1::UnsupportedType),
            },
        ));
    }
    if raw.is::<IndexType>() {
        return Ok(Type::Scalar(ScalarType::Index));
    }
    if raw.is::<FP16Type>() {
        return Ok(Type::Scalar(ScalarType::F16));
    }
    if raw.is::<BFloat16Type>() {
        return Ok(Type::Scalar(ScalarType::Bf16));
    }
    if raw.is::<FP32Type>() {
        return Ok(Type::Scalar(ScalarType::F32));
    }
    if raw.is::<FP64Type>() {
        return Ok(Type::Scalar(ScalarType::F64));
    }
    if let Some(pointer) = raw.downcast_ref::<PlironPointerType>() {
        return Ok(Type::pointer(
            type_from_pliron(context, pointer.pointee())?,
            address_space_from_pliron(pointer.address_space()),
            access_mode_from_pliron(pointer.access()),
        ));
    }
    if let Some(slice) = raw.downcast_ref::<PlironSliceType>() {
        return Ok(Type::slice(
            type_from_pliron(context, slice.element())?,
            address_space_from_pliron(slice.address_space()),
            access_mode_from_pliron(slice.access()),
        ));
    }
    if let Some(kernel_context) = raw.downcast_ref::<PlironKernelContextType>() {
        return Ok(Type::KernelContext(kernel_context_type_from_pliron(
            kernel_context,
        )?));
    }
    if let Some(capability) = raw.downcast_ref::<PlironGlobalCapabilityType>() {
        return Ok(Type::GlobalCapability(global_capability_type_from_pliron(
            context, capability,
        )?));
    }
    if let Some(capability) = raw.downcast_ref::<PlironExecutionCapabilityType>() {
        return Ok(Type::ExecutionCapability(
            capability
                .capability()
                .ok_or(KirBridgeErrorV1::UnsupportedType)?,
        ));
    }
    Err(KirBridgeErrorV1::UnsupportedType)
}

const fn address_space_from_pliron(address_space: AddressSpaceAttr) -> AddressSpace {
    match address_space {
        AddressSpaceAttr::Private => AddressSpace::Private,
        AddressSpaceAttr::Workgroup => AddressSpace::Workgroup,
        AddressSpaceAttr::Global => AddressSpace::Global,
        AddressSpaceAttr::Constant => AddressSpace::Constant,
        AddressSpaceAttr::Generic => AddressSpace::Generic,
    }
}

const fn access_mode_from_pliron(access: AccessModeAttr) -> AccessMode {
    match access {
        AccessModeAttr::ReadOnly => AccessMode::ReadOnly,
        AccessModeAttr::WriteOnly => AccessMode::WriteOnly,
        AccessModeAttr::ReadWrite => AccessMode::ReadWrite,
    }
}

fn constant_from_pliron(
    context: &Context,
    attr: &AttrObj,
    expected: &Constant,
) -> Result<Constant, KirBridgeErrorV1> {
    let integer = || {
        attr.downcast_ref::<IntegerAttr>()
            .map(IntegerAttr::value)
            .ok_or(KirBridgeErrorV1::MalformedGraph)
    };
    let result = match expected {
        Constant::Bool(_) => Constant::Bool(integer()?.to_u8() != 0),
        Constant::I8(_) => Constant::I8(integer()?.to_i8()),
        Constant::I16(_) => Constant::I16(integer()?.to_i16()),
        Constant::I32(_) => Constant::I32(integer()?.to_i32()),
        Constant::I64(_) => Constant::I64(integer()?.to_i64()),
        Constant::U8(_) => Constant::U8(integer()?.to_u8()),
        Constant::U16(_) => Constant::U16(integer()?.to_u16()),
        Constant::U32(_) => Constant::U32(integer()?.to_u32()),
        Constant::U64(_) => Constant::U64(integer()?.to_u64()),
        Constant::Index(_) => Constant::Index(
            attr.downcast_ref::<IndexAttr>()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .0,
        ),
        Constant::F16Bits(_) => Constant::F16Bits(
            attr.downcast_ref::<FPHalfAttr>()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .0
                .to_bits()
                .try_into()
                .map_err(|_| KirBridgeErrorV1::MalformedGraph)?,
        ),
        Constant::Bf16Bits(_) => Constant::Bf16Bits(
            attr.downcast_ref::<BFloat16Attr>()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .0,
        ),
        Constant::F32Bits(_) => Constant::F32Bits(
            attr.downcast_ref::<FPSingleAttr>()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .0
                .to_bits()
                .try_into()
                .map_err(|_| KirBridgeErrorV1::MalformedGraph)?,
        ),
        Constant::F64Bits(_) => Constant::F64Bits(
            attr.downcast_ref::<FPDoubleAttr>()
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .0
                .to_bits()
                .try_into()
                .map_err(|_| KirBridgeErrorV1::MalformedGraph)?,
        ),
    };
    if type_to_pliron(context, &result.ty())?
        != attr_cast::<dyn TypedAttrInterface>(&**attr)
            .map(|typed| typed.get_type(context))
            .ok_or(KirBridgeErrorV1::MalformedGraph)?
    {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    Ok(result)
}

const fn unary_from_pliron(op: UnaryKindAttr) -> UnaryOp {
    match op {
        UnaryKindAttr::Negate => UnaryOp::Negate,
        UnaryKindAttr::Not => UnaryOp::Not,
    }
}

const fn binary_from_pliron(op: BinaryKindAttr) -> BinaryOp {
    use fe2o3_kernel_ir::CheckedBinaryOperator;

    match op {
        BinaryKindAttr::Add => BinaryOp::Add,
        BinaryKindAttr::Subtract => BinaryOp::Subtract,
        BinaryKindAttr::Multiply => BinaryOp::Multiply,
        BinaryKindAttr::Divide => BinaryOp::Divide,
        BinaryKindAttr::Remainder => BinaryOp::Remainder,
        BinaryKindAttr::BitAnd => BinaryOp::BitAnd,
        BinaryKindAttr::BitOr => BinaryOp::BitOr,
        BinaryKindAttr::BitXor => BinaryOp::BitXor,
        BinaryKindAttr::ShiftLeft => BinaryOp::ShiftLeft,
        BinaryKindAttr::ShiftRight => BinaryOp::ShiftRight,
        BinaryKindAttr::CheckedAdd => BinaryOp::Checked(CheckedBinaryOperator::Add),
        BinaryKindAttr::CheckedSubtract => BinaryOp::Checked(CheckedBinaryOperator::Subtract),
        BinaryKindAttr::CheckedMultiply => BinaryOp::Checked(CheckedBinaryOperator::Multiply),
    }
}

const fn compare_from_pliron(predicate: ComparePredicateAttr) -> fe2o3_kernel_ir::ComparePredicate {
    use fe2o3_kernel_ir::ComparePredicate;

    match predicate {
        ComparePredicateAttr::Equal => ComparePredicate::Equal,
        ComparePredicateAttr::NotEqual => ComparePredicate::NotEqual,
        ComparePredicateAttr::LessThan => ComparePredicate::LessThan,
        ComparePredicateAttr::LessThanOrEqual => ComparePredicate::LessThanOrEqual,
        ComparePredicateAttr::GreaterThan => ComparePredicate::GreaterThan,
        ComparePredicateAttr::GreaterThanOrEqual => ComparePredicate::GreaterThanOrEqual,
    }
}

const fn cast_from_pliron(kind: CastKindAttr) -> CastKind {
    match kind {
        CastKindAttr::RestrictPointerAccess => CastKind::RestrictPointerAccess,
        CastKindAttr::Truncate => CastKind::Truncate,
        CastKindAttr::ZeroExtend => CastKind::ZeroExtend,
        CastKindAttr::SignExtend => CastKind::SignExtend,
        CastKindAttr::FloatExtend => CastKind::FloatExtend,
        CastKindAttr::FloatTruncate => CastKind::FloatTruncate,
        CastKindAttr::IntegerToFloat => CastKind::IntegerToFloat,
        CastKindAttr::FloatToInteger => CastKind::FloatToInteger,
        CastKindAttr::Bitcast => CastKind::Bitcast,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HARD_MAX_SESSION_OPERATION_TREE_ITEMS, ShellLimits};
    use fe2o3_kernel_ir::{Function, Signature, ValueDef};

    fn session() -> PlironSession {
        PlironSession::new(
            ShellLimits::default(),
            [
                dialect_gpu::dialect_registration().expect("valid gpu registration"),
                dialect_kernel::dialect_registration().expect("valid kernel registration"),
            ],
        )
        .expect("fresh Pliron session")
    }

    fn v12_capability_module() -> Module {
        let context = fe2o3_kernel_ir::KernelContextTypeV1::new("entry", [1; 32], [2; 32], [3; 32]);
        let source =
            fe2o3_kernel_ir::KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]);
        let mut helper_block = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
        helper_block.terminator = Some(Terminator::Return { values: vec![] });
        let helper = Function::internal_helper(
            "helper",
            Signature::new(vec![Type::KernelContext(context.clone())], vec![]),
            vec![ValueId(0)],
            vec![helper_block],
        );

        let mut entry_block = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
        entry_block
            .operations
            .push(KirOperation::kernel_context_issue(
                ValueId(0),
                context,
                source,
            ));
        entry_block.operations.push(KirOperation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new("helper"),
                arguments: vec![ValueId(0)],
            },
        ));
        entry_block.terminator = Some(Terminator::Return { values: vec![] });

        let mut module = Module::new("tests::v12_capability_bridge");
        module.functions = vec![
            Function::kernel_entry(
                "entry",
                Signature::new(vec![], vec![]),
                vec![],
                vec![entry_block],
            ),
            helper,
        ];
        module.kernels.push(fe2o3_kernel_ir::Kernel::new(
            "kernel",
            "entry",
            fe2o3_kernel_ir::LaunchDomain::D1 {
                x: fe2o3_kernel_ir::LaunchExtent::Dynamic,
            },
        ));
        module.required_capabilities = [
            fe2o3_kernel_ir::ExecutionCapabilityRequirementV1::AddressSpace {
                address_space: AddressSpace::Global,
                access: AccessMode::ReadOnly,
            },
            fe2o3_kernel_ir::ExecutionCapabilityRequirementV1::Resource(
                fe2o3_kernel_ir::ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(256),
            ),
        ]
        .into_iter()
        .map(TargetCapability::Execution)
        .collect();
        module
    }

    fn v12_global_capability_module(role: fe2o3_kernel_ir::GlobalCapabilityRoleV1) -> Module {
        let context = fe2o3_kernel_ir::KernelContextTypeV1::new("entry", [1; 32], [2; 32], [3; 32]);
        let source =
            fe2o3_kernel_ir::KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]);
        let physical = Type::slice(Type::F32, AddressSpace::Global, role.access());
        let capability =
            fe2o3_kernel_ir::GlobalCapabilityTypeV1::new(Type::F32, context.clone(), role);
        let index_space = match role {
            fe2o3_kernel_ir::GlobalCapabilityRoleV1::ReadOnly
            | fe2o3_kernel_ir::GlobalCapabilityRoleV1::ExclusiveReadWrite => None,
            fe2o3_kernel_ir::GlobalCapabilityRoleV1::DisjointWrite(contract) => Some(contract),
        };
        let mut entry = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
        entry.operations = vec![
            KirOperation::kernel_context_issue(ValueId(2), context, source),
            KirOperation::global_capability_bind(
                ValueId(3),
                capability.clone(),
                ValueId(2),
                ValueId(0),
            ),
            KirOperation::global_capability_index(ValueId(4), ValueId(3), ValueId(1), index_space),
            KirOperation::effect_free(
                ValueDef::new(ValueId(5), Type::INDEX),
                OperationKind::SliceLength { slice: ValueId(3) },
            ),
            KirOperation::effect_free(
                ValueDef::new(ValueId(6), capability.physical_pointer_type()),
                OperationKind::SliceData { slice: ValueId(3) },
            ),
        ];
        entry.terminator = Some(Terminator::Return { values: vec![] });

        let mut module = Module::new("tests::v12_global_capability_bridge");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![physical, Type::INDEX], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![entry],
        ));
        module.kernels.push(fe2o3_kernel_ir::Kernel::new(
            "kernel",
            "entry",
            fe2o3_kernel_ir::LaunchDomain::D1 {
                x: fe2o3_kernel_ir::LaunchExtent::Dynamic,
            },
        ));
        module
    }

    fn imported_v12() -> (
        PlironSession,
        KirPlironGraphV1,
        VerifiedCanonicalKernelIrV12,
    ) {
        let input = VerifiedCanonicalKernelIrV12::from_module(v12_capability_module()).unwrap();
        let mut session = session();
        let graph = session.import_canonical_kir_v12_o0(&input).unwrap();
        (session, graph, input)
    }

    fn root_operations(session: &PlironSession, graph: &KirPlironGraphV1) -> Vec<Ptr<Operation>> {
        let root = session.operations[&graph.root.identity];
        let block = root
            .deref(&session.context)
            .get_region(0)
            .deref(&session.context)
            .iter(&session.context)
            .next()
            .unwrap();
        block
            .deref(&session.context)
            .iter(&session.context)
            .collect()
    }

    fn entry_operations(session: &PlironSession, graph: &KirPlironGraphV1) -> Vec<Ptr<Operation>> {
        let roots = root_operations(session, graph);
        let function = roots
            .into_iter()
            .find_map(|operation| Operation::get_op::<FuncOp>(operation, &session.context))
            .expect("module contains a defined function");
        let entry = function.get_entry_block(&session.context);
        entry
            .deref(&session.context)
            .iter(&session.context)
            .collect()
    }

    fn require_v12_rejection(session: &mut PlironSession, graph: &KirPlironGraphV1) {
        assert_eq!(
            session.extract_canonical_kir_v12_o0(graph).unwrap_err(),
            KirBridgeErrorV1::MalformedGraph
        );
    }

    #[test]
    fn v12_global_capability_type_and_operations_round_trip_without_projection() {
        use fe2o3_kernel_ir::{GlobalCapabilityRoleV1, GlobalDisjointIndexContractV1};

        let mappings = [
            fe2o3_kernel_ir::GlobalDisjointIndexSpaceV1::Index1d,
            fe2o3_kernel_ir::GlobalDisjointIndexSpaceV1::ShiftedIndex1d { offset: 9 },
            fe2o3_kernel_ir::GlobalDisjointIndexSpaceV1::BlockedIndex1d {
                lanes_per_block: 64,
                elements_per_lane: 2,
            },
            fe2o3_kernel_ir::GlobalDisjointIndexSpaceV1::Tiled2dIndex1d {
                lanes_per_tile: 64,
                tile_rows: 8,
                tile_columns: 8,
                elements_per_lane: 4,
            },
            fe2o3_kernel_ir::GlobalDisjointIndexSpaceV1::RowStriped2dIndex1d {
                lanes_per_row: 16,
                elements_per_lane: 4,
            },
            fe2o3_kernel_ir::GlobalDisjointIndexSpaceV1::GridExclusive,
        ];
        let roles = [
            GlobalCapabilityRoleV1::ReadOnly,
            GlobalCapabilityRoleV1::ExclusiveReadWrite,
        ]
        .into_iter()
        .chain(mappings.into_iter().enumerate().map(|(index, mapping)| {
            let mut identity = [0_u8; 32];
            identity[0] = u8::try_from(index + 1).unwrap();
            GlobalCapabilityRoleV1::DisjointWrite(GlobalDisjointIndexContractV1::new(
                identity, mapping,
            ))
        }));

        for role in roles {
            let input =
                VerifiedCanonicalKernelIrV12::from_module(v12_global_capability_module(role))
                    .unwrap();
            let mut session = session();
            let graph = session.import_canonical_kir_v12_o0(&input).unwrap();
            let operations = entry_operations(&session, &graph);
            assert!(Operation::is_op::<PlironGlobalCapabilityBindOp>(
                operations[1],
                &session.context
            ));
            assert!(Operation::is_op::<PlironGlobalCapabilityIndexOp>(
                operations[2],
                &session.context
            ));
            assert!(Operation::is_op::<PlironGlobalCapabilityLengthOp>(
                operations[3],
                &session.context
            ));
            assert!(Operation::is_op::<PlironGlobalCapabilityDataOp>(
                operations[4],
                &session.context
            ));
            {
                let capability_type_handle = operations[1].deref(&session.context).get_type(0);
                let capability_type = capability_type_handle.deref(&session.context);
                assert!(capability_type.is::<PlironGlobalCapabilityType>());
                assert!(!capability_type.is::<PlironSliceType>());
            }

            let (output, report) = session.extract_canonical_kir_v12_o0(&graph).unwrap();
            assert_eq!(output.canonical_bytes(), input.canonical_bytes());
            assert!(report.is_exact());
        }
    }

    #[test]
    fn v12_global_capability_bridge_rejects_projection_and_contract_substitution() {
        use fe2o3_kernel_ir::{
            GlobalCapabilityRoleV1, GlobalDisjointIndexContractV1, GlobalDisjointIndexSpaceV1,
        };

        let contract =
            GlobalDisjointIndexContractV1::new([8; 32], GlobalDisjointIndexSpaceV1::Index1d);
        let role = GlobalCapabilityRoleV1::DisjointWrite(contract);

        let input =
            VerifiedCanonicalKernelIrV12::from_module(v12_global_capability_module(role)).unwrap();
        let mut projected = session();
        let projected_graph = projected.import_canonical_kir_v12_o0(&input).unwrap();
        let bind = entry_operations(&projected, &projected_graph)[1];
        let physical = Type::slice(Type::F32, AddressSpace::Global, AccessMode::WriteOnly);
        let bind_result = { bind.deref(&projected.context).get_result(0) };
        bind_result.set_type(
            &projected.context,
            type_to_pliron(&projected.context, &physical).unwrap(),
        );
        require_v12_rejection(&mut projected, &projected_graph);

        let mut substituted = session();
        let substituted_graph = substituted.import_canonical_kir_v12_o0(&input).unwrap();
        let index = Operation::get_op::<PlironGlobalCapabilityIndexOp>(
            entry_operations(&substituted, &substituted_graph)[2],
            &substituted.context,
        )
        .unwrap();
        index.set_attr_global_access_contract(
            &substituted.context,
            PlironGlobalAccessContractAttr::new(GlobalCapabilityRoleV1::DisjointWrite(
                GlobalDisjointIndexContractV1::new([9; 32], GlobalDisjointIndexSpaceV1::Index1d),
            )),
        );
        require_v12_rejection(&mut substituted, &substituted_graph);
    }

    #[test]
    fn v12_bridge_rejects_requirement_deletion_duplication_reorder_and_substitution() {
        let (mut deleted, deleted_graph, _) = imported_v12();
        root_operations(&deleted, &deleted_graph)[1].unlink(&deleted.context);
        require_v12_rejection(&mut deleted, &deleted_graph);

        let (mut duplicated, duplicated_graph, _) = imported_v12();
        let roots = root_operations(&duplicated, &duplicated_graph);
        let requirement = portable_requirements(&duplicated_graph.metadata)[0];
        let duplicate = ExecutionRequirementOp::new(
            &mut duplicated.context,
            CanonicalIdentityAttr::from_bytes(duplicated_graph.input.digest),
            0,
            requirement,
        )
        .get_operation();
        duplicate.insert_before(&duplicated.context, roots[3]);
        require_v12_rejection(&mut duplicated, &duplicated_graph);

        let (mut reordered, reordered_graph, _) = imported_v12();
        let roots = root_operations(&reordered, &reordered_graph);
        roots[2].unlink(&reordered.context);
        roots[2].insert_before(&reordered.context, roots[1]);
        require_v12_rejection(&mut reordered, &reordered_graph);

        let (mut substituted, substituted_graph, _) = imported_v12();
        let first = root_operations(&substituted, &substituted_graph)[1];
        let first =
            Operation::get_op::<ExecutionRequirementOp>(first, &substituted.context).unwrap();
        first.set_attr_kernel_execution_requirement_decl_requirement(
            &substituted.context,
            dialect_kernel::ExecutionRequirementAttr::new(
                &fe2o3_kernel_ir::ExecutionCapabilityRequirementV1::AddressSpace {
                    address_space: AddressSpace::Constant,
                    access: AccessMode::ReadOnly,
                },
            ),
        );
        require_v12_rejection(&mut substituted, &substituted_graph);
    }

    #[test]
    fn v12_bridge_rejects_stale_epoch_and_context_correspondence_tampering() {
        let (mut stale, stale_graph, _) = imported_v12();
        let contract = Operation::get_op::<GraphContractOp>(
            root_operations(&stale, &stale_graph)[0],
            &stale.context,
        )
        .unwrap();
        contract.set_attr_kernel_graph_contract_epoch(
            &stale.context,
            CanonicalIdentityAttr::from_bytes([9; 32]),
        );
        require_v12_rejection(&mut stale, &stale_graph);

        let (mut source_substitution, source_graph, _) = imported_v12();
        let issue = Operation::get_op::<KernelContextIssueOp>(
            entry_operations(&source_substitution, &source_graph)[0],
            &source_substitution.context,
        )
        .unwrap();
        issue.set_attr_kernel_kernel_context_issue_frontend_unit(
            &source_substitution.context,
            CanonicalIdentityAttr::from_bytes([8; 32]),
        );
        require_v12_rejection(&mut source_substitution, &source_graph);

        let (mut coordinate_substitution, coordinate_graph, _) = imported_v12();
        let issue = Operation::get_op::<KernelContextIssueOp>(
            entry_operations(&coordinate_substitution, &coordinate_graph)[0],
            &coordinate_substitution.context,
        )
        .unwrap();
        issue.set_attr_kernel_kernel_context_issue_coordinate(
            &coordinate_substitution.context,
            SourceCoordinateAttr::new(0, 0, 1),
        );
        require_v12_rejection(&mut coordinate_substitution, &coordinate_graph);

        let (mut identity_substitution, identity_graph, _) = imported_v12();
        let issue = Operation::get_op::<KernelContextIssueOp>(
            entry_operations(&identity_substitution, &identity_graph)[0],
            &identity_substitution.context,
        )
        .unwrap();
        issue.set_attr_kernel_kernel_context_issue_operation_identity(
            &identity_substitution.context,
            CanonicalIdentityAttr::from_bytes([8; 32]),
        );
        require_v12_rejection(&mut identity_substitution, &identity_graph);

        let (mut reordered, reordered_graph, _) = imported_v12();
        let operations = entry_operations(&reordered, &reordered_graph);
        operations[0].unlink(&reordered.context);
        operations[0].insert_after(&reordered.context, operations[1]);
        require_v12_rejection(&mut reordered, &reordered_graph);
    }

    fn capacity_module(operation_count: usize) -> Module {
        let mut entry = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
        entry.operations = (0..operation_count)
            .map(|index| {
                KirOperation::effect_free(
                    ValueDef::new(
                        ValueId(u32::try_from(index).expect("test value id fits")),
                        Type::Scalar(ScalarType::U32),
                    ),
                    OperationKind::Constant(Constant::U32(
                        u32::try_from(index).expect("test constant fits"),
                    )),
                )
            })
            .collect();
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        });
        let mut exit = fe2o3_kernel_ir::BasicBlock::new(BlockId(1));
        exit.terminator = Some(Terminator::Return { values: vec![] });

        let mut module = Module::new("tests::kir_bridge_capacity_v1");
        module.functions.push(Function::internal_helper(
            "capacity",
            Signature::new(vec![], vec![]),
            vec![],
            vec![entry, exit],
        ));
        module
    }

    fn pointer_restriction_module() -> Module {
        let read_write = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
        let read_only = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
        let mut entry = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
        entry.operations.push(KirOperation::effect_free(
            ValueDef::new(ValueId(1), read_only.clone()),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: ValueId(0),
                to: read_only,
            },
        ));
        entry.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("tests::kir_bridge_pointer_restriction_v11");
        module.functions.push(Function::internal_helper(
            "restrict",
            Signature::new(vec![read_write], vec![]),
            vec![ValueId(0)],
            vec![entry],
        ));
        module
    }

    #[test]
    fn pointer_access_restriction_round_trips_through_typed_v11_bridge() {
        let input = VerifiedCanonicalKernelIrV11::from_module(pointer_restriction_module())
            .expect("verified V11 input");
        let mut session = session();
        let graph = session
            .import_canonical_kir_v11_o0(&input)
            .expect("typed V11 import");
        let (output, report) = session
            .extract_canonical_kir_v11_o0(&graph)
            .expect("exact typed V11 extraction");
        assert_eq!(output.canonical_bytes(), input.canonical_bytes());
        assert_eq!(report.input(), report.output());
    }

    #[test]
    fn correspondence_digest_has_a_fixed_complete_transcript() {
        let records = [
            KirBridgeCorrespondenceV1 {
                pliron_ordinal: 0,
                coordinate: KirBridgeCoordinateV1::Function { function: 7 },
            },
            KirBridgeCorrespondenceV1 {
                pliron_ordinal: 1,
                coordinate: KirBridgeCoordinateV1::Operation {
                    function: 7,
                    block: 2,
                    operation: 9,
                },
            },
        ];
        let digest = correspondence_digest_v1(&records);
        assert_eq!(digest.count(), 2);
        assert_eq!(
            digest.digest(),
            [
                0xa7, 0x24, 0xd2, 0x85, 0x3b, 0xba, 0x9d, 0x50, 0x90, 0xb3, 0x40, 0x0c, 0x49, 0x9f,
                0xf5, 0xd9, 0x6c, 0x46, 0x7d, 0x38, 0x26, 0x96, 0x87, 0xad, 0xbf, 0x5c, 0x26, 0xe2,
                0x36, 0xf7, 0x8b, 0x88,
            ]
        );

        let mut reordered = records;
        reordered.swap(0, 1);
        assert_ne!(
            correspondence_digest_v1(&reordered).digest(),
            digest.digest()
        );
        let mut changed_coordinate = records;
        changed_coordinate[1].coordinate = KirBridgeCoordinateV1::Terminator {
            function: 7,
            block: 2,
        };
        assert_ne!(
            correspondence_digest_v1(&changed_coordinate).digest(),
            digest.digest()
        );
    }

    #[test]
    fn every_safety_significant_operation_family_has_a_bounded_preflight_codec() {
        use fe2o3_kernel_ir::*;

        let access = MemoryAccess::new(AddressSpace::Global, 4);
        let kinds = vec![
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
                pointer: ValueId(1),
                value: ValueId(2),
                element: MemoryElementType::Scalar(ScalarType::U32),
                address_space: AddressSpace::Global,
                layout: MemoryLayout::new(4, 4),
                contract: VolatileAccessContract::rust_allocation_store(),
            }),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: Some(ValueId(0)),
                address_space: AddressSpace::Private,
                alignment: 4,
            },
            OperationKind::GuardedLoad {
                pointer: ValueId(0),
                predicate: ValueId(1),
                fallback: ValueId(2),
                access,
            },
            OperationKind::GuardedStore {
                pointer: ValueId(0),
                predicate: ValueId(1),
                value: ValueId(2),
                access,
            },
            OperationKind::Barrier(Barrier {
                execution_scope: SynchronizationScope::Workgroup,
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
            }),
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Add,
                pointer: ValueId(0),
                value: Some(ValueId(1)),
                compare: None,
                access,
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::Relaxed,
                failure_ordering: None,
            }),
            OperationKind::Fence(Fence {
                memory_scope: SynchronizationScope::Device,
                semantics: BarrierSemantics::new(MemoryOrdering::Release, [AddressSpace::Global]),
            }),
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::Static(64),
                alignment: 16,
            }),
            OperationKind::Matrix(MatrixOperation::multiply_accumulate(
                [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
                [ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
                [ValueId(8), ValueId(9), ValueId(10), ValueId(11)],
            )),
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Current {
                    format: Gfx950LdsTransposeFormatV1::Fp8E4M3,
                },
            )),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::LaneId,
                WaveWidth::Wave64,
            )),
            OperationKind::InlineAssembly(InlineAssembly {
                target: InlineAssemblyTarget::AmdGpuGfx942,
                source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
                mnemonic: "v_nop".to_owned(),
                operands: vec![],
                options: Default::default(),
                declared_effects: Default::default(),
            }),
        ];
        let coordinate = KirBridgeCoordinateV1::Operation {
            function: 1,
            block: 2,
            operation: 3,
        };
        assert_eq!(kinds.len(), 14, "the closed legacy family catalog changed");
        for kind in kinds {
            let operation = KirOperation::new(vec![], kind);
            assert_eq!(preflight_operation(&operation, coordinate), Ok(()));
        }
    }

    #[test]
    fn root_and_session_capacity_are_exact_and_reject_before_allocation() {
        // Two blocks make total tree work `12 + 2 * operation_count`.
        let exact_operation_count = (HARD_MAX_OPERATION_TREE_ITEMS - 12) / 2;
        let exact = capacity_module(exact_operation_count);
        assert_eq!(preflight(&exact).unwrap().0, HARD_MAX_OPERATION_TREE_ITEMS);
        let exact_input = VerifiedCanonicalKernelIrV9::from_module(exact).unwrap();
        let mut exact_limited = session();
        exact_limited
            .import_canonical_kir_v9_o0(&exact_input)
            .expect("exact root boundary is admitted");
        assert_eq!(
            exact_limited.operation_tree_work,
            HARD_MAX_OPERATION_TREE_ITEMS
        );
        assert!(!exact_limited.is_poisoned());

        let over = capacity_module(exact_operation_count + 1);
        let over_input = VerifiedCanonicalKernelIrV9::from_module(over).unwrap();
        let mut root_limited = session();
        let next_handle = root_limited.next_operation_handle;
        let error = match root_limited.import_canonical_kir_v9_o0(&over_input) {
            Err(error) => error,
            Ok(_) => panic!("over-limit root was admitted"),
        };
        assert_eq!(
            error,
            KirBridgeErrorV1::Session(OperationHandleError::OperationTreeLimitExceeded)
        );
        assert!(root_limited.operations.is_empty());
        assert!(root_limited.operation_roots.is_empty());
        assert!(root_limited.owned_tree_work.is_empty());
        assert!(root_limited.next_operation_handle == next_handle);
        assert!(!root_limited.is_poisoned());

        let small_module = capacity_module(0);
        let required = preflight(&small_module).unwrap().0;
        let small_input = VerifiedCanonicalKernelIrV9::from_module(small_module).unwrap();
        let mut aggregate_limited = session();
        aggregate_limited.operation_tree_work =
            HARD_MAX_SESSION_OPERATION_TREE_ITEMS - required + 1;
        let initial_work = aggregate_limited.operation_tree_work;
        let next_handle = aggregate_limited.next_operation_handle;
        let error = match aggregate_limited.import_canonical_kir_v9_o0(&small_input) {
            Err(error) => error,
            Ok(_) => panic!("over-limit session aggregate was admitted"),
        };
        assert_eq!(
            error,
            KirBridgeErrorV1::Session(OperationHandleError::SessionOperationTreeLimitExceeded)
        );
        assert_eq!(aggregate_limited.operation_tree_work, initial_work);
        assert!(aggregate_limited.operations.is_empty());
        assert!(aggregate_limited.next_operation_handle == next_handle);
        assert!(!aggregate_limited.is_poisoned());

        aggregate_limited.operation_tree_work = HARD_MAX_SESSION_OPERATION_TREE_ITEMS - required;
        aggregate_limited
            .import_canonical_kir_v9_o0(&small_input)
            .expect("exact aggregate boundary is admitted");
        assert_eq!(
            aggregate_limited.operation_tree_work,
            HARD_MAX_SESSION_OPERATION_TREE_ITEMS
        );
        assert!(!aggregate_limited.is_poisoned());
    }
}
