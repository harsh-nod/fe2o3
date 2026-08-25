use std::collections::HashMap;

use fe2o3_amdgcn_model::AddressSpace;
use fe2o3_llvm_handoff::{
    AddressSpaceV1, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallingConventionV2, EvidenceV2,
    ExecutableModuleV2, FloatBinaryOperationV2, FunctionAttributeV1, FunctionAttributeV2,
    FunctionIdV2, FunctionKindV2, FunctionParameterV2, FunctionV2, Gfx942HandoffV1,
    Gfx942HandoffV2, Gfx942TargetPolicyV1, InstructionKindV2, InstructionV2, KernelValueTypeV1,
    ModuleFlagV1, NamedMetadataV1, ObligationKindV1, ParameterAttributeV1, ReturnTypeV2,
    ScalarTypeV1, TerminatorV2, TypedValueV2, ValueIdV2, ValueTypeV2,
};
use fe2o3_pliron::{ContextIdentity, require_context_identity};
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        op_interfaces::{SingleBlockRegionInterface, SymbolOpInterface},
        type_interfaces::FunctionTypeInterface,
        types::FP32Type,
    },
    common_traits::Named,
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    r#type::{TypeHandle, Typed},
    value::Value,
};
use pliron_llvm::{
    attributes::FastmathFlagsAttr,
    op_interfaces::{AlignableOpInterface, FastMathFlags, LlvmSymbolName},
    ops::{FAddOp, FuncOp, LoadOp, ReturnOp, StoreOp},
    types::{PointerType, VoidType},
};

use crate::{
    lower::encode_receipt,
    model::{CanonicalLoweringReceiptV1, HandoffExtractionDiagnosticV2, admitted_obligations_v1},
};

const F32_ALIGNMENT: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveValueType {
    F32,
    OpaquePointer { address_space: AddressSpaceV1 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LiveTypedValue {
    id: ValueIdV2,
    value_type: LiveValueType,
}

struct LiveParameterFacts {
    value: LiveTypedValue,
    name: String,
}

enum LiveInstructionKind {
    Load {
        pointer: LiveTypedValue,
        value_type: LiveValueType,
        alignment: u16,
    },
    FloatAdd {
        left: LiveTypedValue,
        right: LiveTypedValue,
        strict_fp: bool,
    },
    Store {
        pointer: LiveTypedValue,
        value: LiveTypedValue,
        value_type: LiveValueType,
        alignment: u16,
    },
}

struct LiveInstructionFacts {
    result: Option<LiveTypedValue>,
    kind: LiveInstructionKind,
    successors: Vec<BlockIdV2>,
}

enum LiveTerminatorFacts {
    Return {
        value: Option<LiveTypedValue>,
        successors: Vec<BlockIdV2>,
    },
}

struct LiveBlockFacts {
    id: BlockIdV2,
    instructions: Vec<LiveInstructionFacts>,
    terminator: LiveTerminatorFacts,
}

struct LiveFunctionFacts {
    id: FunctionIdV2,
    symbol: String,
    return_type: ReturnTypeV2,
    parameters: Vec<LiveParameterFacts>,
    entry: BlockIdV2,
    blocks: Vec<LiveBlockFacts>,
}

/// Policy retained outside the live Pliron graph because pliron-llvm has no
/// representation for it. Construction receives this only after exact receipt,
/// target, ABI, metadata, origin, and obligation validation.
struct ValidatedV1Sidecar<'a> {
    base: &'a Gfx942HandoffV1,
    function_kind: FunctionKindV2,
    calling_convention: CallingConventionV2,
    parameter_types: Vec<KernelValueTypeV1>,
    parameter_attributes: Vec<Vec<ParameterAttributeV1>>,
    function_attributes: Vec<FunctionAttributeV2>,
    module_flags: Vec<ModuleFlagV1>,
    named_metadata: Vec<NamedMetadataV1>,
    evidence: EvidenceV2,
}

struct LiveGraphTranslator<'a> {
    context: &'a Context,
    values: HashMap<Value, LiveTypedValue>,
    blocks: HashMap<Ptr<BasicBlock>, BlockIdV2>,
    next_value_id: u32,
}

pub(crate) fn extract_handoff_v2(
    context: &Context,
    retained_identity: ContextIdentity,
    module_owner: ContextIdentity,
    module: &pliron::builtin::ops::ModuleOp,
    expected_module_name: &str,
    base: &Gfx942HandoffV1,
    receipt: &CanonicalLoweringReceiptV1,
) -> Result<Gfx942HandoffV2, HandoffExtractionDiagnosticV2> {
    validate_owner(context, retained_identity, module_owner, module)?;
    let sidecar = validate_retained_v1_sidecar(expected_module_name, base, receipt)?;
    let facts = inspect_live_graph(context, module, expected_module_name)?;
    validate_graph_sidecar_bridge(&facts, &sidecar)?;
    build_handoff_v2(sidecar, facts)
}

fn validate_owner(
    context: &Context,
    retained_identity: ContextIdentity,
    module_owner: ContextIdentity,
    module: &pliron::builtin::ops::ModuleOp,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    let current = require_context_identity(context)
        .map_err(|_| HandoffExtractionDiagnosticV2::ContextIdentityInvalid)?;
    if current != retained_identity {
        return Err(HandoffExtractionDiagnosticV2::ContextIdentityInvalid);
    }
    if module_owner != current {
        return Err(HandoffExtractionDiagnosticV2::ForeignOwner);
    }
    let module_pointer = module.get_operation();
    let module_ref = module_pointer
        .try_deref(context)
        .map_err(|_| HandoffExtractionDiagnosticV2::StaleModule)?;
    drop(module_ref);
    verify_operation(module_pointer, context)
        .map_err(|_| HandoffExtractionDiagnosticV2::DialectVerificationFailed)
}

fn validate_retained_v1_sidecar<'a>(
    module_name: &str,
    base: &'a Gfx942HandoffV1,
    receipt: &CanonicalLoweringReceiptV1,
) -> Result<ValidatedV1Sidecar<'a>, HandoffExtractionDiagnosticV2> {
    let expected_receipt = encode_receipt(module_name, base)
        .map_err(|_| HandoffExtractionDiagnosticV2::EvidenceMismatch)?;
    if expected_receipt != *receipt {
        return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
    }

    if base.target() != &Gfx942TargetPolicyV1::canonical()
        || base.kernels().len() != 1
        || base.module().flags() != [ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2]
        || !base.module().named_metadata().is_empty()
        || !base.module().device_libraries().is_empty()
    {
        return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
    }
    let kernel = &base.kernels()[0];
    if kernel.parameters().len() != 3
        || kernel.parameters()[0].value_type()
            != (KernelValueTypeV1::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: AddressSpaceV1::Global,
            })
        || kernel.parameters()[1].value_type()
            != (KernelValueTypeV1::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: AddressSpaceV1::Global,
            })
        || kernel.parameters()[2].value_type() != KernelValueTypeV1::Scalar(ScalarTypeV1::F32)
        || kernel
            .parameters()
            .iter()
            .any(|parameter| !parameter.attributes().is_empty())
        || !has_exact_function_attributes(kernel.function_attributes())
    {
        return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
    }
    let evidence = validate_evidence(base)?;
    Ok(ValidatedV1Sidecar {
        base,
        function_kind: FunctionKindV2::Kernel,
        calling_convention: CallingConventionV2::AmdGpuKernel,
        parameter_types: kernel
            .parameters()
            .iter()
            .map(|parameter| parameter.value_type())
            .collect(),
        parameter_attributes: kernel
            .parameters()
            .iter()
            .map(|parameter| parameter.attributes().to_vec())
            .collect(),
        function_attributes: kernel
            .function_attributes()
            .iter()
            .copied()
            .map(FunctionAttributeV2::from)
            .collect(),
        module_flags: base.module().flags().to_vec(),
        named_metadata: base.module().named_metadata().to_vec(),
        evidence,
    })
}

fn has_exact_function_attributes(attributes: &[FunctionAttributeV1]) -> bool {
    attributes.len() >= 9
        && attributes.iter().all(|attribute| {
            matches!(
                attribute,
                FunctionAttributeV1::NoUnwind
                    | FunctionAttributeV1::FlatWorkgroupSize(_)
                    | FunctionAttributeV1::DenormalFpMathF32Ieee
                    | FunctionAttributeV1::UnsafeFpMathDisabled
                    | FunctionAttributeV1::NoInfsFpMathDisabled
                    | FunctionAttributeV1::NoNansFpMathDisabled
                    | FunctionAttributeV1::NoSignedZerosFpMathDisabled
                    | FunctionAttributeV1::ApproxFuncFpMathDisabled
                    | FunctionAttributeV1::FpContractOff
                    | FunctionAttributeV1::NoCompletionAction
                    | FunctionAttributeV1::NoDefaultQueue
                    | FunctionAttributeV1::NoHeapPointer
                    | FunctionAttributeV1::NoHostcallPointer
                    | FunctionAttributeV1::NoMultigridSyncArgument
                    | FunctionAttributeV1::NoQueuePointer
            )
        })
        && attributes
            .iter()
            .filter(|attribute| matches!(attribute, FunctionAttributeV1::NoUnwind))
            .count()
            == 1
        && attributes
            .iter()
            .filter(|attribute| {
                matches!(
                    attribute,
                    FunctionAttributeV1::FlatWorkgroupSize(range)
                        if range.minimum() == 1 && range.maximum() == 64
                )
            })
            .count()
            == 1
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::DenormalFpMathF32Ieee))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::UnsafeFpMathDisabled))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::NoInfsFpMathDisabled))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::NoNansFpMathDisabled))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::NoSignedZerosFpMathDisabled))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::ApproxFuncFpMathDisabled))
        && attributes
            .iter()
            .any(|attribute| matches!(attribute, FunctionAttributeV1::FpContractOff))
}

fn validate_evidence(base: &Gfx942HandoffV1) -> Result<EvidenceV2, HandoffExtractionDiagnosticV2> {
    let [origin] = base.origins() else {
        return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
    };
    if origin.kind() != fe2o3_llvm_handoff::OriginKindV1::AmdgcnIr
        || origin.span().is_some()
        || base.kernels()[0].origin() != origin.identity()
        || base.obligations().len() != admitted_obligations_v1().len()
    {
        return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
    }

    for required in admitted_obligations_v1() {
        let matching = base
            .obligations()
            .iter()
            .copied()
            .filter(|obligation| obligation.kind() == *required)
            .collect::<Vec<_>>();
        let [obligation] = matching.as_slice() else {
            return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
        };
        let expected_subject = match required {
            ObligationKindV1::PreserveKernelAbi | ObligationKindV1::MaintainOriginCoverage => {
                base.stage_identities().semantic()
            }
            ObligationKindV1::PreserveAddressSpaces
            | ObligationKindV1::PreserveTargetFeatures
            | ObligationKindV1::PreserveCallingConvention
            | ObligationKindV1::PreserveFunctionAttributes
            | ObligationKindV1::PreserveModuleMetadata => base.stage_identities().target_plan(),
            ObligationKindV1::AuthenticateDeviceLibraries => {
                return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
            }
        };
        if obligation.origin() != origin.identity() || obligation.subject() != expected_subject {
            return Err(HandoffExtractionDiagnosticV2::EvidenceMismatch);
        }
    }

    EvidenceV2::new(
        origin.identity(),
        base.obligations()
            .iter()
            .map(|obligation| obligation.identity())
            .collect(),
    )
    .map_err(|_| HandoffExtractionDiagnosticV2::EvidenceMismatch)
}

fn inspect_live_graph(
    context: &Context,
    module: &pliron::builtin::ops::ModuleOp,
    expected_module_name: &str,
) -> Result<LiveFunctionFacts, HandoffExtractionDiagnosticV2> {
    validate_module_shape(context, module, expected_module_name)?;
    let module_body = module.get_body(context, 0);
    let module_operations = module_body.deref(context).iter(context).collect::<Vec<_>>();
    if module_operations.len() != 1 {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }
    let (function_index, function_pointer) =
        module_operations
            .iter()
            .copied()
            .enumerate()
            .next()
            .ok_or(HandoffExtractionDiagnosticV2::OperationShapeMismatch)?;
    let function = Operation::get_op::<FuncOp>(function_pointer, context)
        .ok_or(HandoffExtractionDiagnosticV2::OperationShapeMismatch)?;
    let (return_type, blocks) = validate_function_shape(context, &function, function_pointer)?;

    let symbol = function.get_symbol_name(context);
    if function.llvm_symbol_name(context).is_some() {
        return Err(HandoffExtractionDiagnosticV2::SymbolMismatch);
    }

    let entry = function
        .get_entry_block(context)
        .ok_or(HandoffExtractionDiagnosticV2::ControlFlowMismatch)?;
    let mut translator = LiveGraphTranslator::new(context, &blocks)?;
    let arguments = entry.deref(context).arguments().collect::<Vec<_>>();
    let mut parameters = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let value = translator.define_value(argument)?;
        let name = argument
            .given_name(context)
            .ok_or(HandoffExtractionDiagnosticV2::SymbolMismatch)?;
        parameters.push(LiveParameterFacts {
            value,
            name: name.as_ref().to_owned(),
        });
    }

    let entry_id = translator.block_id(entry)?;
    let mut live_blocks = Vec::with_capacity(blocks.len());
    for block in blocks {
        live_blocks.push(translator.translate_block(block)?);
    }
    let facts = LiveFunctionFacts {
        id: FunctionIdV2::new(
            u32::try_from(function_index)
                .map_err(|_| HandoffExtractionDiagnosticV2::OperationShapeMismatch)?,
        ),
        symbol: symbol.as_ref().to_owned(),
        return_type,
        parameters,
        entry: entry_id,
        blocks: live_blocks,
    };
    validate_closed_live_graph(&facts)?;
    Ok(facts)
}

fn validate_module_shape(
    context: &Context,
    module: &pliron::builtin::ops::ModuleOp,
    expected_module_name: &str,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    let module_pointer = module.get_operation();
    let operation = module_pointer.deref(context);
    if operation.get_num_results() != 0
        || operation.get_num_operands() != 0
        || operation.get_num_successors() != 0
        || operation.num_regions() != 1
        || operation.attributes.0.len() != 1
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }
    drop(operation);
    if module.get_symbol_name(context).as_ref() != expected_module_name {
        return Err(HandoffExtractionDiagnosticV2::SymbolMismatch);
    }
    let body = module.get_body(context, 0);
    let body_ref = body.deref(context);
    if body_ref.get_num_arguments() != 0
        || !body_ref.attributes.0.is_empty()
        || body_ref.given_name(context).is_some()
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }
    Ok(())
}

fn validate_function_shape(
    context: &Context,
    function: &FuncOp,
    function_pointer: Ptr<Operation>,
) -> Result<(ReturnTypeV2, Vec<Ptr<BasicBlock>>), HandoffExtractionDiagnosticV2> {
    let operation = function_pointer.deref(context);
    if operation.get_num_results() != 0
        || operation.get_num_operands() != 0
        || operation.get_num_successors() != 0
        || operation.num_regions() != 1
        || operation.attributes.0.len() != 2
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }
    let region = operation.get_region(0);
    drop(operation);
    let blocks = region.deref(context).iter(context).collect::<Vec<_>>();
    let [entry] = blocks.as_slice() else {
        return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
    };
    if entry
        .deref(context)
        .given_name(context)
        .as_ref()
        .map(AsRef::as_ref)
        != Some("entry")
        || entry.deref(context).attributes.0.len() != 1
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }

    let function_type = function.get_type(context);
    let function_type_ref = function_type.deref(context);
    if function_type_ref.is_var_arg()
        || function_type_ref
            .result_type()
            .deref(context)
            .downcast_ref::<VoidType>()
            .is_none()
    {
        return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
    }
    if function_type_ref.arg_types().len() != 3 {
        return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
    }
    drop(function_type_ref);
    Ok((ReturnTypeV2::Void, blocks))
}

fn validate_leaf_shape(
    context: &Context,
    operation: Ptr<Operation>,
    results: usize,
    operands: usize,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    let operation = operation.deref(context);
    if operation.get_num_results() != results
        || operation.get_num_operands() != operands
        || operation.num_regions() != 0
    {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }
    Ok(())
}

fn translate_live_type(
    context: &Context,
    value_type: TypeHandle,
) -> Result<LiveValueType, HandoffExtractionDiagnosticV2> {
    let value_type = value_type.deref(context);
    if value_type.downcast_ref::<FP32Type>().is_some() {
        return Ok(LiveValueType::F32);
    }
    if let Some(pointer) = value_type.downcast_ref::<PointerType>() {
        if pointer.address_space() != AddressSpace::Global.llvm_id() {
            return Err(HandoffExtractionDiagnosticV2::AddressSpaceMismatch);
        }
        return Ok(LiveValueType::OpaquePointer {
            address_space: AddressSpaceV1::Global,
        });
    }
    Err(HandoffExtractionDiagnosticV2::TypeMismatch)
}

impl<'a> LiveGraphTranslator<'a> {
    fn new(
        context: &'a Context,
        blocks: &[Ptr<BasicBlock>],
    ) -> Result<Self, HandoffExtractionDiagnosticV2> {
        let mut block_ids = HashMap::with_capacity(blocks.len());
        for (index, block) in blocks.iter().copied().enumerate() {
            let id = BlockIdV2::new(
                u32::try_from(index)
                    .map_err(|_| HandoffExtractionDiagnosticV2::ControlFlowMismatch)?,
            );
            if block_ids.insert(block, id).is_some() {
                return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
            }
        }
        Ok(Self {
            context,
            values: HashMap::new(),
            blocks: block_ids,
            next_value_id: 0,
        })
    }

    fn block_id(&self, block: Ptr<BasicBlock>) -> Result<BlockIdV2, HandoffExtractionDiagnosticV2> {
        self.blocks
            .get(&block)
            .copied()
            .ok_or(HandoffExtractionDiagnosticV2::ControlFlowMismatch)
    }

    fn define_value(
        &mut self,
        value: Value,
    ) -> Result<LiveTypedValue, HandoffExtractionDiagnosticV2> {
        if self.values.contains_key(&value) {
            return Err(HandoffExtractionDiagnosticV2::DefUseMismatch);
        }
        let mapped = LiveTypedValue {
            id: ValueIdV2::new(self.next_value_id),
            value_type: translate_live_type(self.context, value.get_type(self.context))?,
        };
        self.next_value_id = self
            .next_value_id
            .checked_add(1)
            .ok_or(HandoffExtractionDiagnosticV2::OperationShapeMismatch)?;
        self.values.insert(value, mapped);
        Ok(mapped)
    }

    fn mapped_value(&self, value: Value) -> Result<LiveTypedValue, HandoffExtractionDiagnosticV2> {
        let mapped = self
            .values
            .get(&value)
            .copied()
            .ok_or(HandoffExtractionDiagnosticV2::DefUseMismatch)?;
        if mapped.value_type != translate_live_type(self.context, value.get_type(self.context))? {
            return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
        }
        Ok(mapped)
    }

    fn successors(
        &self,
        operation: Ptr<Operation>,
    ) -> Result<Vec<BlockIdV2>, HandoffExtractionDiagnosticV2> {
        operation
            .deref(self.context)
            .successors()
            .map(|successor| self.block_id(successor))
            .collect()
    }

    fn translate_block(
        &mut self,
        block: Ptr<BasicBlock>,
    ) -> Result<LiveBlockFacts, HandoffExtractionDiagnosticV2> {
        let id = self.block_id(block)?;
        let body = block
            .deref(self.context)
            .iter(self.context)
            .collect::<Vec<_>>();
        let mut instructions = Vec::with_capacity(body.len());
        let mut terminator = None;
        let mut terminator_pointer = None;

        for operation in body {
            if terminator.is_some() {
                return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
            }
            if let Some(load) = Operation::get_op::<LoadOp>(operation, self.context) {
                instructions.push(self.translate_load(operation, &load)?);
            } else if let Some(add) = Operation::get_op::<FAddOp>(operation, self.context) {
                instructions.push(self.translate_float_add(operation, &add)?);
            } else if let Some(store) = Operation::get_op::<StoreOp>(operation, self.context) {
                instructions.push(self.translate_store(operation, &store)?);
            } else if let Some(return_op) = Operation::get_op::<ReturnOp>(operation, self.context) {
                terminator = Some(self.translate_return(operation, &return_op)?);
                terminator_pointer = Some(operation);
            } else {
                return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
            }
        }

        let terminator = terminator.ok_or(HandoffExtractionDiagnosticV2::ControlFlowMismatch)?;
        if block.deref(self.context).get_terminator(self.context) != terminator_pointer {
            return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
        }
        Ok(LiveBlockFacts {
            id,
            instructions,
            terminator,
        })
    }

    fn translate_load(
        &mut self,
        operation: Ptr<Operation>,
        load: &LoadOp,
    ) -> Result<LiveInstructionFacts, HandoffExtractionDiagnosticV2> {
        validate_leaf_shape(self.context, operation, 1, 1)?;
        let operation_ref = operation.deref(self.context);
        if operation_ref.attributes.0.len() != 1 {
            return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
        }
        let pointer = operation_ref.get_operand(0);
        let result = operation_ref.get_result(0);
        drop(operation_ref);
        let pointer = self.mapped_value(pointer)?;
        let result = self.define_value(result)?;
        Ok(LiveInstructionFacts {
            result: Some(result),
            kind: LiveInstructionKind::Load {
                pointer,
                value_type: result.value_type,
                alignment: translate_alignment(load.alignment(self.context))?,
            },
            successors: self.successors(operation)?,
        })
    }

    fn translate_float_add(
        &mut self,
        operation: Ptr<Operation>,
        add: &FAddOp,
    ) -> Result<LiveInstructionFacts, HandoffExtractionDiagnosticV2> {
        validate_leaf_shape(self.context, operation, 1, 2)?;
        let operation_ref = operation.deref(self.context);
        if operation_ref.attributes.0.len() != 1 {
            return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
        }
        let left = operation_ref.get_operand(0);
        let right = operation_ref.get_operand(1);
        let result = operation_ref.get_result(0);
        drop(operation_ref);
        let left = self.mapped_value(left)?;
        let right = self.mapped_value(right)?;
        let result = self.define_value(result)?;
        Ok(LiveInstructionFacts {
            result: Some(result),
            kind: LiveInstructionKind::FloatAdd {
                left,
                right,
                strict_fp: add.fast_math_flags(self.context) == FastmathFlagsAttr::default(),
            },
            successors: self.successors(operation)?,
        })
    }

    fn translate_store(
        &self,
        operation: Ptr<Operation>,
        store: &StoreOp,
    ) -> Result<LiveInstructionFacts, HandoffExtractionDiagnosticV2> {
        validate_leaf_shape(self.context, operation, 0, 2)?;
        let operation_ref = operation.deref(self.context);
        if operation_ref.attributes.0.len() != 1 {
            return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
        }
        let value = operation_ref.get_operand(0);
        let pointer = operation_ref.get_operand(1);
        drop(operation_ref);
        let value = self.mapped_value(value)?;
        let pointer = self.mapped_value(pointer)?;
        Ok(LiveInstructionFacts {
            result: None,
            kind: LiveInstructionKind::Store {
                pointer,
                value,
                value_type: value.value_type,
                alignment: translate_alignment(store.alignment(self.context))?,
            },
            successors: self.successors(operation)?,
        })
    }

    fn translate_return(
        &self,
        operation: Ptr<Operation>,
        return_op: &ReturnOp,
    ) -> Result<LiveTerminatorFacts, HandoffExtractionDiagnosticV2> {
        let operation_ref = operation.deref(self.context);
        if operation_ref.get_num_results() != 0
            || operation_ref.get_num_operands() > 1
            || operation_ref.num_regions() != 0
            || !operation_ref.attributes.0.is_empty()
        {
            return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
        }
        drop(operation_ref);
        let value = return_op
            .retval(self.context)
            .map(|value| self.mapped_value(value))
            .transpose()?;
        Ok(LiveTerminatorFacts::Return {
            value,
            successors: self.successors(operation)?,
        })
    }
}

fn translate_alignment(alignment: Option<u32>) -> Result<u16, HandoffExtractionDiagnosticV2> {
    let alignment = alignment.ok_or(HandoffExtractionDiagnosticV2::AlignmentMismatch)?;
    u16::try_from(alignment).map_err(|_| HandoffExtractionDiagnosticV2::AlignmentMismatch)
}

fn validate_closed_live_graph(
    facts: &LiveFunctionFacts,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    let [input, output, addend] = facts.parameters.as_slice() else {
        return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
    };
    let pointer_type = LiveValueType::OpaquePointer {
        address_space: AddressSpaceV1::Global,
    };
    if input.value.value_type != pointer_type
        || output.value.value_type != pointer_type
        || addend.value.value_type != LiveValueType::F32
    {
        return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
    }
    let [block] = facts.blocks.as_slice() else {
        return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
    };
    if facts.entry != block.id {
        return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
    }
    let [load, add, store] = block.instructions.as_slice() else {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    };
    if load
        .successors
        .iter()
        .chain(&add.successors)
        .chain(&store.successors)
        .next()
        .is_some()
    {
        return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
    }

    let Some(loaded) = load.result else {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    };
    match load.kind {
        LiveInstructionKind::Load {
            pointer,
            value_type,
            alignment,
        } => {
            if pointer != input.value {
                return Err(HandoffExtractionDiagnosticV2::DefUseMismatch);
            }
            if pointer.value_type != pointer_type
                || loaded.value_type != LiveValueType::F32
                || value_type != LiveValueType::F32
            {
                return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
            }
            if alignment != F32_ALIGNMENT as u16 {
                return Err(HandoffExtractionDiagnosticV2::AlignmentMismatch);
            }
        }
        _ => return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch),
    }

    let Some(computed) = add.result else {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    };
    match add.kind {
        LiveInstructionKind::FloatAdd {
            left,
            right,
            strict_fp,
        } => {
            if left != loaded || right != addend.value {
                return Err(HandoffExtractionDiagnosticV2::DefUseMismatch);
            }
            if left.value_type != LiveValueType::F32
                || right.value_type != LiveValueType::F32
                || computed.value_type != LiveValueType::F32
            {
                return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
            }
            if !strict_fp {
                return Err(HandoffExtractionDiagnosticV2::StrictFpMismatch);
            }
        }
        _ => return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch),
    }

    if store.result.is_some() {
        return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch);
    }
    match store.kind {
        LiveInstructionKind::Store {
            pointer,
            value,
            value_type,
            alignment,
        } => {
            if pointer != output.value || value != computed {
                return Err(HandoffExtractionDiagnosticV2::DefUseMismatch);
            }
            if pointer.value_type != pointer_type
                || value.value_type != LiveValueType::F32
                || value_type != LiveValueType::F32
            {
                return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
            }
            if alignment != F32_ALIGNMENT as u16 {
                return Err(HandoffExtractionDiagnosticV2::AlignmentMismatch);
            }
        }
        _ => return Err(HandoffExtractionDiagnosticV2::OperationShapeMismatch),
    }

    match &block.terminator {
        LiveTerminatorFacts::Return { value, successors } => {
            if value.is_some() || !successors.is_empty() {
                return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
            }
        }
    }
    Ok(())
}

fn validate_graph_sidecar_bridge(
    facts: &LiveFunctionFacts,
    sidecar: &ValidatedV1Sidecar<'_>,
) -> Result<(), HandoffExtractionDiagnosticV2> {
    let kernel = &sidecar.base.kernels()[0];
    if facts.symbol != kernel.symbol() || facts.parameters.len() != kernel.parameters().len() {
        return Err(HandoffExtractionDiagnosticV2::SymbolMismatch);
    }
    for (live, retained) in facts.parameters.iter().zip(kernel.parameters()) {
        if live.name != retained.name() {
            return Err(HandoffExtractionDiagnosticV2::SymbolMismatch);
        }
        if !live_type_matches_retained(live.value.value_type, retained.value_type()) {
            return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
        }
    }
    Ok(())
}

fn live_type_matches_retained(live: LiveValueType, retained: KernelValueTypeV1) -> bool {
    matches!(
        (live, retained),
        (
            LiveValueType::F32,
            KernelValueTypeV1::Scalar(ScalarTypeV1::F32)
        ) | (
            LiveValueType::OpaquePointer {
                address_space: AddressSpaceV1::Global
            },
            KernelValueTypeV1::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: AddressSpaceV1::Global
            }
        )
    )
}

fn combine_parameter_type(
    live: LiveValueType,
    retained: KernelValueTypeV1,
) -> Result<ValueTypeV2, HandoffExtractionDiagnosticV2> {
    if !live_type_matches_retained(live, retained) {
        return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
    }
    Ok(retained.into())
}

fn build_block_v2(
    block: LiveBlockFacts,
    evidence: &EvidenceV2,
) -> Result<BasicBlockV2, HandoffExtractionDiagnosticV2> {
    let instructions = block
        .instructions
        .into_iter()
        .map(|instruction| build_instruction_v2(instruction, evidence))
        .collect::<Result<Vec<_>, _>>()?;
    let terminator = match block.terminator {
        LiveTerminatorFacts::Return { value, successors } => {
            if !successors.is_empty() {
                return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
            }
            TerminatorV2::Return(value.map(|value| value.id))
        }
    };
    Ok(BasicBlockV2::new(block.id, instructions, terminator))
}

fn build_instruction_v2(
    instruction: LiveInstructionFacts,
    evidence: &EvidenceV2,
) -> Result<InstructionV2, HandoffExtractionDiagnosticV2> {
    if !instruction.successors.is_empty() {
        return Err(HandoffExtractionDiagnosticV2::ControlFlowMismatch);
    }
    let result = instruction.result.map(translate_result).transpose()?;
    let kind = match instruction.kind {
        LiveInstructionKind::Load {
            pointer,
            value_type,
            alignment,
        } => InstructionKindV2::Load {
            pointer: pointer.id,
            value_type: translate_scalar_type(value_type)?,
            alignment,
        },
        LiveInstructionKind::FloatAdd {
            left,
            right,
            strict_fp,
        } => {
            if !strict_fp {
                return Err(HandoffExtractionDiagnosticV2::StrictFpMismatch);
            }
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Add),
                left: left.id,
                right: right.id,
            }
        }
        LiveInstructionKind::Store {
            pointer,
            value,
            value_type,
            alignment,
        } => InstructionKindV2::Store {
            pointer: pointer.id,
            value: value.id,
            value_type: translate_scalar_type(value_type)?,
            alignment,
        },
    };
    InstructionV2::new(result, kind, evidence.clone())
        .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)
}

fn translate_result(value: LiveTypedValue) -> Result<TypedValueV2, HandoffExtractionDiagnosticV2> {
    let value_type = match value.value_type {
        LiveValueType::F32 => ValueTypeV2::Scalar(ScalarTypeV1::F32),
        LiveValueType::OpaquePointer { .. } => {
            return Err(HandoffExtractionDiagnosticV2::TypeMismatch);
        }
    };
    Ok(TypedValueV2::new(value.id, value_type))
}

fn translate_scalar_type(
    value_type: LiveValueType,
) -> Result<ScalarTypeV1, HandoffExtractionDiagnosticV2> {
    match value_type {
        LiveValueType::F32 => Ok(ScalarTypeV1::F32),
        LiveValueType::OpaquePointer { .. } => Err(HandoffExtractionDiagnosticV2::TypeMismatch),
    }
}

fn build_handoff_v2(
    sidecar: ValidatedV1Sidecar<'_>,
    facts: LiveFunctionFacts,
) -> Result<Gfx942HandoffV2, HandoffExtractionDiagnosticV2> {
    let LiveFunctionFacts {
        id,
        symbol,
        return_type,
        parameters: live_parameters,
        entry,
        blocks: live_blocks,
    } = facts;
    let parameters = live_parameters
        .into_iter()
        .zip(
            sidecar
                .parameter_types
                .iter()
                .copied()
                .zip(&sidecar.parameter_attributes),
        )
        .map(|(parameter, (retained_type, retained_attributes))| {
            let value_type = combine_parameter_type(parameter.value.value_type, retained_type)?;
            FunctionParameterV2::new(
                TypedValueV2::new(parameter.value.id, value_type),
                &parameter.name,
                retained_attributes.clone(),
            )
            .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)
        })
        .collect::<Result<Vec<_>, HandoffExtractionDiagnosticV2>>()?;
    let blocks = live_blocks
        .into_iter()
        .map(|block| build_block_v2(block, &sidecar.evidence))
        .collect::<Result<Vec<_>, _>>()?;
    let function = FunctionV2::new(
        id,
        &symbol,
        sidecar.function_kind,
        sidecar.calling_convention,
        return_type,
        parameters,
        sidecar.function_attributes,
        entry,
        blocks,
        sidecar.evidence.clone(),
    )
    .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)?;
    let module = ExecutableModuleV2::new(
        sidecar.module_flags,
        sidecar.named_metadata,
        vec![],
        vec![],
        vec![function],
    )
    .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)?;
    Gfx942HandoffV2::new(sidecar.base.clone(), module)
        .map_err(|_| HandoffExtractionDiagnosticV2::HandoffConstructionFailed)
}
