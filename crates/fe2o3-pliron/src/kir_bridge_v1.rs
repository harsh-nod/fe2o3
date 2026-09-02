//! Typed, O0-only bridge between canonical Kernel IR V9 and Pliron.
//!
//! This bridge is deliberately not a textual import path.  It constructs a
//! live operation graph and extracts Kernel IR from that graph.  The report is
//! evidence of exact structural replay only; it makes no optimization or
//! semantic-preservation claim.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt,
    num::NonZero,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_gpu::{
    AddressSpaceAttr,
    optimization_v1::{
        AccessModeAttr, BFloat16Attr, BFloat16Type, BinaryKindAttr, BinaryOp as PlironBinaryOp,
        BranchOp, CallOp, CastKindAttr, CastOp, CompareOp as PlironCompareOp, ComparePredicateAttr,
        CondBranchOp, ConstantOp as PlironConstantOp, GetElementPointerOp, IndexAttr, IndexType,
        LoadOp, PointerType as PlironPointerType, ReturnOp, SelectOp as PlironSelectOp,
        SliceDataOp, SliceLengthOp, SliceType as PlironSliceType, StoreOp, UnaryKindAttr,
        UnaryOp as PlironUnaryOp,
    },
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, BlockId, CastKind, Constant, FunctionId, Module,
    Operation as KirOperation, OperationKind, ScalarType, Terminator, Type, UnaryOp, ValueId,
    VerifiedCanonicalKernelIrV9,
};
use pliron::{
    attribute::{AttrObj, attr_cast},
    basic_block::BasicBlock,
    builtin::{
        attr_interfaces::TypedAttrInterface,
        attributes::{FPDoubleAttr, FPHalfAttr, FPSingleAttr, IntegerAttr},
        op_interfaces::{BranchOpInterface, SingleBlockRegionInterface},
        ops::{ConstantOp as BuiltinConstantOp, FuncOp, ModuleOp},
        type_interfaces::FunctionTypeInterface,
        types::{FP16Type, FP32Type, FP64Type, FunctionType, IntegerType, Signedness, UnitType},
    },
    context::{Context, Ptr},
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    r#type::{TypeHandle, Typed},
    utils::{
        apfloat::{Double, Float, Half, Single},
        apint::APInt,
    },
    value::Value,
};

use crate::{HARD_MAX_OPERATION_TREE_ITEMS, OperationHandle, OperationHandleError, PlironSession};

// `ModuleOp::new` creates one operation containing one region and one block.
const BUILTIN_MODULE_ROOT_TREE_WORK_V1: usize = 3;

/// Domain separator for identities of exact canonical Kernel IR at this bridge.
pub const KIR_PLIRON_BRIDGE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/CANONICAL-KIR-V9/V1\0";

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

    pub fn is_exact(&self) -> bool {
        self.input == self.output
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
    correspondence: Vec<KirBridgeCorrespondenceV1>,
    origins: KirBridgeOriginsV1,
}

#[derive(Clone, Default)]
struct KirBridgeOriginsV1 {
    functions: HashMap<Ptr<Operation>, usize>,
    blocks: HashMap<Ptr<BasicBlock>, (usize, BlockId)>,
    values: HashMap<Value, ValueId>,
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
    UnsupportedGenericAddressSpace,
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
                formatter.write_str("canonical Kernel IR V9 input failed revalidation")
            }
            Self::Session(error) => {
                write!(formatter, "Pliron session rejected bridge graph: {error}")
            }
            Self::SizeOverflow => formatter.write_str("bridge graph size exceeds its fixed bounds"),
            Self::UnsupportedType => {
                formatter.write_str("Kernel IR type is unsupported by the bridge")
            }
            Self::UnsupportedGenericAddressSpace => {
                formatter.write_str("generic Kernel IR address space is unsupported by the bridge")
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
        import_module(self, input, module)
    }

    /// Extracts canonical Kernel IR from a typed live graph and requires an
    /// exact O0 round trip.
    pub fn extract_canonical_kir_v9_o0(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV9, KirBridgeRoundTripReportV1), KirBridgeErrorV1> {
        let output = extract_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV9::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes())?;
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
        let (output, correspondence) = extract_optimized_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV9::from_module(output)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes())?;
        let receipt = KirBridgeOptimizedReceiptV1 {
            input: graph.input,
            output: output_digest,
            correspondence,
        };
        Ok((output, receipt))
    }
}

fn digest(bytes: &[u8]) -> Result<KirBridgeDigestV1, KirBridgeErrorV1> {
    use sha2::{Digest, Sha256};

    let canonical_bytes = u64::try_from(bytes.len()).map_err(|_| KirBridgeErrorV1::SizeOverflow)?;
    let mut hasher = Sha256::new();
    hasher.update(
        u32::try_from(KIR_PLIRON_BRIDGE_IDENTITY_DOMAIN_V1.len())
            .map_err(|_| KirBridgeErrorV1::SizeOverflow)?
            .to_le_bytes(),
    );
    hasher.update(KIR_PLIRON_BRIDGE_IDENTITY_DOMAIN_V1);
    hasher.update(canonical_bytes.to_le_bytes());
    hasher.update(bytes);
    Ok(KirBridgeDigestV1 {
        digest: hasher.finalize().into(),
        canonical_bytes,
    })
}

fn import_module(
    session: &mut PlironSession,
    input: &VerifiedCanonicalKernelIrV9,
    module: Module,
) -> Result<KirPlironGraphV1, KirBridgeErrorV1> {
    let (tree_work, correspondence) = preflight(&module)?;
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
    let built = catch_unwind(AssertUnwindSafe(|| {
        build_module_graph(&mut session.context, root_pointer, &module)
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
        input: digest(input.canonical_bytes())?,
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
        extract_module_graph(&session.context, root, &graph.metadata)
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
        extract_optimized_module_graph(&session.context, root, &graph.metadata, &graph.origins)
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
    }
}

fn preflight_address_space(address_space: AddressSpace) -> Result<(), KirBridgeErrorV1> {
    if address_space == AddressSpace::Generic {
        Err(KirBridgeErrorV1::UnsupportedGenericAddressSpace)
    } else {
        Ok(())
    }
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
        | OperationKind::Store { .. } => Ok(()),
        _ => Err(KirBridgeErrorV1::UnsupportedOperation { coordinate }),
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
        _ => Err(KirBridgeErrorV1::UnsupportedTerminator { coordinate }),
    }
}

fn build_module_graph(
    context: &mut Context,
    root: Ptr<Operation>,
    module: &Module,
) -> Result<KirBridgeOriginsV1, KirBridgeErrorV1> {
    if !Operation::is_op::<ModuleOp>(root, context) {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let root = ModuleOp::from_operation(root);
    let mut origins = KirBridgeOriginsV1::default();
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
        }
        for (block_index, block) in body.blocks.iter().enumerate() {
            let live = block_for(&blocks, function_index, block.id)?;
            let offset = usize::from(block_index == 0) * body.parameters.len();
            for (parameter_index, parameter) in block.parameters.iter().enumerate() {
                let live_value = live.deref(context).get_argument(offset + parameter_index);
                values.insert(parameter.id, live_value);
                origins.values.insert(live_value, parameter.id);
            }
        }

        for (block_index, operation_index) in
            operation_build_schedule(body, function_index, &values)?
        {
            let block = &body.blocks[block_index];
            let operation = &block.operations[operation_index];
            let live_block = block_for(&blocks, function_index, block.id)?;
            let live = build_operation(context, function_index, operation, &values)?;
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
                {
                    return Err(KirBridgeErrorV1::MalformedGraph);
                }
            }
        }
        for block in &body.blocks {
            let live_block = block_for(&blocks, function_index, block.id)?;
            let terminator = build_terminator(
                context,
                function_index,
                block.terminator.as_ref(),
                &values,
                &blocks,
            )?;
            terminator.insert_at_back(live_block, context);
        }
    }
    Ok(origins)
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
) -> Result<Ptr<Operation>, KirBridgeErrorV1> {
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
            SliceLengthOp::new(context, value_for(values, function, *slice)?).get_operation()
        }
        OperationKind::SliceData { slice } => {
            SliceDataOp::new(context, value_for(values, function, *slice)?)
                .ok_or(KirBridgeErrorV1::MalformedGraph)?
                .get_operation()
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
        _ => return Err(KirBridgeErrorV1::MalformedGraph),
    };
    Ok(live)
}

fn build_terminator(
    context: &mut Context,
    function: usize,
    terminator: Option<&Terminator>,
    values: &BTreeMap<ValueId, Value>,
    blocks: &BTreeMap<BlockId, Ptr<BasicBlock>>,
) -> Result<Ptr<Operation>, KirBridgeErrorV1> {
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
        Some(Terminator::Return { values: returned }) => {
            Ok(ReturnOp::new(context, values_for(values, function, returned)?).get_operation())
        }
        _ => Err(KirBridgeErrorV1::MalformedGraph),
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
    })
}

fn address_space_to_pliron(
    address_space: AddressSpace,
) -> Result<AddressSpaceAttr, KirBridgeErrorV1> {
    match address_space {
        AddressSpace::Private => Ok(AddressSpaceAttr::Private),
        AddressSpace::Workgroup => Ok(AddressSpaceAttr::Workgroup),
        AddressSpace::Global => Ok(AddressSpaceAttr::Global),
        AddressSpace::Constant => Ok(AddressSpaceAttr::Constant),
        AddressSpace::Generic => Err(KirBridgeErrorV1::UnsupportedGenericAddressSpace),
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

fn extract_optimized_module_graph(
    context: &Context,
    root: Ptr<Operation>,
    metadata: &Module,
    origins: &KirBridgeOriginsV1,
) -> Result<(Module, Vec<KirBridgeCorrespondenceV1>), KirBridgeErrorV1> {
    if !Operation::is_op::<ModuleOp>(root, context) || root.deref(context).num_regions() != 1 {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let root_region = root.deref(context).get_region(0);
    let root_blocks: Vec<_> = root_region.deref(context).iter(context).collect();
    let [root_block] = root_blocks.as_slice() else {
        return Err(KirBridgeErrorV1::MalformedGraph);
    };
    let live_functions: Vec<_> = root_block.deref(context).iter(context).collect();
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
        function.signature.parameters = function_type
            .arg_types()
            .into_iter()
            .map(|ty| type_from_pliron(context, ty))
            .collect::<Result<Vec<_>, _>>()?;
        function.signature.results = function_type
            .res_types()
            .into_iter()
            .map(|ty| type_from_pliron(context, ty))
            .collect::<Result<Vec<_>, _>>()?;
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
                        type_from_pliron(context, argument.get_type(context))?,
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
                let result_types: Vec<_> = live_operation
                    .deref(context)
                    .result_types()
                    .map(|ty| type_from_pliron(context, ty))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut results = Vec::with_capacity(result_types.len());
                for (index, ty) in result_types.into_iter().enumerate() {
                    let live_result = live_operation.deref(context).get_result(index);
                    let id = id_for(&reverse_values, live_result)?;
                    results.push(fe2o3_kernel_ir::ValueDef::new(id, ty));
                }
                let kind =
                    extract_any_operation(context, *live_operation, &reverse_values, coordinate)?;
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

fn extract_any_operation(
    context: &Context,
    live: Ptr<Operation>,
    reverse: &HashMap<Value, ValueId>,
    coordinate: KirBridgeCoordinateV1,
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
    if Operation::is_op::<SliceDataOp>(live, context) {
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
    let live_functions: Vec<_> = root_block.deref(context).iter(context).collect();
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
        output_function.signature.parameters = function_type
            .arg_types()
            .into_iter()
            .map(|ty| type_from_pliron(context, ty))
            .collect::<Result<Vec<_>, _>>()?;
        output_function.signature.results = function_type
            .res_types()
            .into_iter()
            .map(|ty| type_from_pliron(context, ty))
            .collect::<Result<Vec<_>, _>>()?;

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
        for ((source_block, output_block), live_block) in source_body
            .blocks
            .iter()
            .zip(&mut output_body.blocks)
            .zip(&live_blocks)
        {
            let live_operations: Vec<_> = live_block.deref(context).iter(context).collect();
            for ((source_operation, output_operation), live_operation) in source_block
                .operations
                .iter()
                .zip(&mut output_block.operations)
                .zip(&live_operations)
            {
                output_operation.kind = extract_operation(
                    context,
                    *live_operation,
                    &source_operation.kind,
                    &reverse_values,
                )?;
                for (index, result) in output_operation.results.iter_mut().enumerate() {
                    result.ty =
                        type_from_pliron(context, live_operation.deref(context).get_type(index))?;
                }
            }
            output_block.terminator = Some(extract_terminator(
                context,
                *live_operations
                    .last()
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?,
                &reverse_values,
                &reverse_blocks,
            )?);
            for (index, parameter) in output_block.parameters.iter_mut().enumerate() {
                let offset = usize::from(output_block.id == source_body.blocks[0].id)
                    * source_body.parameters.len();
                parameter.ty = type_from_pliron(
                    context,
                    live_block
                        .deref(context)
                        .get_argument(offset + index)
                        .get_type(context),
                )?;
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
    expected: &OperationKind,
    reverse: &HashMap<Value, ValueId>,
) -> Result<OperationKind, KirBridgeErrorV1> {
    let raw = live.deref(context);
    match expected {
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
            if !Operation::is_op::<SliceLengthOp>(live, context) {
                return Err(KirBridgeErrorV1::MalformedGraph);
            }
            Ok(OperationKind::SliceLength {
                slice: id_for(reverse, raw.get_operand(0))?,
            })
        }
        OperationKind::SliceData { .. } => {
            if !Operation::is_op::<SliceDataOp>(live, context) {
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
        _ => Err(KirBridgeErrorV1::MalformedGraph),
    }
}

fn extract_terminator(
    context: &Context,
    live: Ptr<Operation>,
    reverse_values: &HashMap<Value, ValueId>,
    reverse_blocks: &HashMap<Ptr<BasicBlock>, BlockId>,
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
    Err(KirBridgeErrorV1::UnsupportedType)
}

const fn address_space_from_pliron(address_space: AddressSpaceAttr) -> AddressSpace {
    match address_space {
        AddressSpaceAttr::Private => AddressSpace::Private,
        AddressSpaceAttr::Workgroup => AddressSpace::Workgroup,
        AddressSpaceAttr::Global => AddressSpace::Global,
        AddressSpaceAttr::Constant => AddressSpace::Constant,
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
            [dialect_gpu::dialect_registration().expect("valid gpu registration")],
        )
        .expect("fresh Pliron session")
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

    #[test]
    fn root_and_session_capacity_are_exact_and_reject_before_allocation() {
        // Two blocks make total tree work `12 + 2 * operation_count`.
        let exact_operation_count = (HARD_MAX_OPERATION_TREE_ITEMS - 12) / 2;
        let exact = capacity_module(exact_operation_count);
        assert_eq!(preflight(&exact).unwrap().0, HARD_MAX_OPERATION_TREE_ITEMS);

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
