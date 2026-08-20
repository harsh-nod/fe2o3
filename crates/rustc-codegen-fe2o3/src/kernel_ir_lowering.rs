//! Verification-only lowering from imported MIR to `fe2o3-kernel-ir`.
//!
//! This vertical slice models the optimized MIR shape of the existing `vecadd`
//! kernel plus explicitly classified internal helpers and device FFI exports.
//! Known helper calls remain typed external calls with
//! their exact rustc identities. The `DisjointSlice::get_mut` declaration uses
//! two results (the `Option` discriminant and payload pointer), because kernel IR
//! does not yet have Rust aggregate types. MIR unwind actions are not represented
//! by kernel IR; supported helper calls are treated as non-unwinding and failed
//! bounds assertions branch to one synthetic unreachable block.
//!
//! The executable subset is unprojected aliases, `Use`, `Discriminant`,
//! `PtrMetadata`, `Add`, `Mul`, and `Lt`; direct/indexed dereferences; the classified
//! 1D thread-index and disjoint-slice helpers; and return, unreachable, goto,
//! integer switch, call, and assert terminators. Locals must be assigned once
//! and MIR blocks must appear in definition-before-use order. Every other
//! construct produces a located diagnostic rather than a partial module.

mod control_flow_ssa;
mod semantic_lowering;

use crate::AmdGpuTarget;
use crate::mir_import::{
    MirBinaryOp, MirBlock, MirCallee, MirConstant, MirFunction, MirFunctionKind, MirKernelProfile,
    MirModule, MirOperandRef, MirPlaceRef, MirProjectionElem, MirReferenceSemantics, MirRvalueKind,
    MirSemanticInstanceIdentity, MirSourceLocation, MirStatement, MirStatementKind, MirTerminator,
    MirTerminatorKind, MirTypeShape, MirUnaryOp,
};
use crate::trusted_device_items::{TrustedDeviceItem, TrustedHalfOperation};
use dialect_amdgcn::{DeviceMathDiagnosticItem, recognized_device_math_operation};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AmdGpuDiagnosticOperation, BasicBlock, BinaryOp, BlockId,
    ComparePredicate, Constant, FloatConversionKind, FloatOperation, Function, FunctionId, Kernel,
    LaunchDomain, LaunchExtent, MATRIX_PROJECTED_KERNARG_POLICY_NAMESPACE_V1,
    MATRIX_SOURCE_ABI_OBSERVATION_NAMESPACE_V2, MatrixFrontendBindingV2, MemoryAccess, Module,
    Operation, OperationKind, ScalarType, Signature, SwitchCase, TargetCapability, Terminator,
    Type, ValueDef, ValueId, WorkgroupSize, gfx942_xnack_minus_target_capability, verify_module,
};
use reserved_fe2o3_symbols::{
    DeviceFfiAddressSpaceV1, DeviceFfiPhysicalResultV1, DeviceFfiPhysicalTypeV1,
    DeviceFfiPointerAccessV1, DeviceFfiScalarTypeV1,
};
use rustc_session::Session;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

const MODULE_ID: &str = "rustc_codegen_fe2o3::mir_analysis";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TranslationDiagnosticCode {
    MalformedMir,
    UnsupportedType,
    UnsupportedStatement,
    UnsupportedRvalue,
    UnsupportedProjection,
    UnsupportedCall,
    VerificationFailed,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TranslationLocation {
    pub function: Option<String>,
    pub block: Option<usize>,
    pub statement: Option<usize>,
    pub terminator: bool,
    pub operation: Option<usize>,
    pub source: Option<Box<MirSourceLocation>>,
}

impl TranslationLocation {
    fn function(function: &MirFunction) -> Self {
        Self {
            function: Some(function.rust_path.clone()),
            block: None,
            statement: None,
            terminator: false,
            operation: None,
            source: None,
        }
    }

    fn block(function: &MirFunction, block: &MirBlock) -> Self {
        Self {
            block: Some(block.index),
            ..Self::function(function)
        }
    }

    fn statement(function: &MirFunction, block: usize, statement: &MirStatement) -> Self {
        Self {
            function: Some(function.rust_path.clone()),
            block: Some(block),
            statement: Some(statement.index),
            terminator: false,
            operation: None,
            source: statement.source.clone().map(Box::new),
        }
    }

    fn terminator(function: &MirFunction, block: usize, terminator: &MirTerminator) -> Self {
        Self {
            function: Some(function.rust_path.clone()),
            block: Some(block),
            statement: None,
            terminator: true,
            operation: None,
            source: terminator.source.clone().map(Box::new),
        }
    }
}

impl fmt::Display for TranslationLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(source) = &self.source {
            write!(
                formatter,
                "{}:{}:{}",
                source.file, source.line, source.column
            )?;
        } else {
            formatter.write_str("<unknown source>")?;
        }
        if let Some(function) = &self.function {
            write!(formatter, " in {function}")?;
        }
        if let Some(block) = self.block {
            write!(formatter, " bb{block}")?;
        }
        if let Some(statement) = self.statement {
            write!(formatter, " stmt{statement}")?;
        } else if self.terminator {
            formatter.write_str(" terminator")?;
        }
        if let Some(operation) = self.operation {
            write!(formatter, " op{operation}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TranslationDiagnostic {
    pub location: TranslationLocation,
    pub code: TranslationDiagnosticCode,
    pub message: String,
}

impl fmt::Display for TranslationDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {:?}: {}",
            self.location, self.code, self.message
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationErrors {
    diagnostics: Vec<TranslationDiagnostic>,
}

impl TranslationErrors {
    #[cfg(test)]
    pub fn diagnostics(&self) -> &[TranslationDiagnostic] {
        &self.diagnostics
    }

    #[cfg(test)]
    pub fn contains(&self, code: TranslationDiagnosticCode) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code)
    }
}

impl fmt::Display for TranslationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "MIR to kernel IR translation failed with {} diagnostic(s)",
            self.diagnostics.len()
        )?;
        for diagnostic in &self.diagnostics {
            writeln!(formatter, "  {diagnostic}")?;
        }
        Ok(())
    }
}

impl Error for TranslationErrors {}

#[cfg(test)]
pub fn translate_and_verify(mir: &MirModule) -> Result<Module, TranslationErrors> {
    translate_and_verify_with_float_target(mir, None, StrictFloatPolicy::Canonical)
}

#[cfg(test)]
pub(crate) fn translate_and_verify_for_target(
    mir: &MirModule,
    target: &AmdGpuTarget,
) -> Result<Module, TranslationErrors> {
    translate_and_verify_for_target_with_policy(mir, target, StrictFloatPolicy::Canonical)
}

pub(crate) fn translate_and_verify_for_session(
    mir: &MirModule,
    target: &AmdGpuTarget,
    session: &Session,
) -> Result<Module, TranslationErrors> {
    let policy = if session.opts.cg.llvm_args.is_empty() && session.opts.cg.passes.is_empty() {
        StrictFloatPolicy::Canonical
    } else {
        StrictFloatPolicy::CustomLlvmPipeline
    };
    translate_and_verify_for_target_with_policy(mir, target, policy)
}

fn translate_and_verify_for_target_with_policy(
    mir: &MirModule,
    target: &AmdGpuTarget,
    strict_float_policy: StrictFloatPolicy,
) -> Result<Module, TranslationErrors> {
    let float_target = AmdTargetId::parse(target.as_str())
        .ok()
        .filter(|target| target.processor() == "gfx942")
        .map(|_| Gfx942FloatTarget);
    let collective_target = exact_gfx942_xnack_minus_target(target);
    translate_and_verify_with_targets(mir, float_target, collective_target, strict_float_policy)
}

#[derive(Clone, Copy, Debug)]
struct Gfx942FloatTarget;

#[derive(Clone, Copy, Debug)]
struct Gfx942WaveLdsTargetV2;

fn exact_gfx942_xnack_minus_target(target: &AmdGpuTarget) -> Option<Gfx942WaveLdsTargetV2> {
    const EXACT: &str = "gfx942:xnack-";
    let parsed = AmdTargetId::parse(target.as_str()).ok()?;
    let expected = AmdTargetId::parse(EXACT).expect("exact gfx942 target is canonical");
    (parsed == expected && parsed.to_string() == EXACT && target.as_str() == EXACT)
        .then_some(Gfx942WaveLdsTargetV2)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StrictFloatPolicy {
    Canonical,
    CustomLlvmPipeline,
}

#[derive(Clone, Debug)]
struct InternalDefinitionContract {
    export_name: String,
    signature: Signature,
}

#[cfg(test)]
fn translate_and_verify_with_float_target(
    mir: &MirModule,
    float_target: Option<Gfx942FloatTarget>,
    strict_float_policy: StrictFloatPolicy,
) -> Result<Module, TranslationErrors> {
    translate_and_verify_with_targets(mir, float_target, None, strict_float_policy)
}

fn translate_and_verify_with_targets(
    mir: &MirModule,
    float_target: Option<Gfx942FloatTarget>,
    collective_target: Option<Gfx942WaveLdsTargetV2>,
    strict_float_policy: StrictFloatPolicy,
) -> Result<Module, TranslationErrors> {
    let mut functions = mir.functions.iter().collect::<Vec<_>>();
    functions.sort_by(|lhs, rhs| {
        (&lhs.export_name, &lhs.rust_path).cmp(&(&rhs.export_name, &rhs.rust_path))
    });

    let mut diagnostics = Vec::new();
    let mut declarations = BTreeMap::new();
    let mut definitions = Vec::new();
    let mut kernel_entries = Vec::new();
    let mut kernel_ids = BTreeSet::new();
    let mut launch_contracts = BTreeMap::new();
    let mut internal_definitions = BTreeMap::new();
    let mut internal_exports = BTreeMap::new();

    for function in functions.iter().copied().filter(|function| {
        matches!(
            function.kind,
            MirFunctionKind::InternalHelper | MirFunctionKind::DeviceFfiExport
        )
    }) {
        let signature = match declared_function_signature(function) {
            Ok(signature) => signature,
            Err(error) => {
                diagnostics.push(error);
                continue;
            }
        };
        if let Some(previous) = internal_definitions.insert(
            function.semantic_instance_v1(),
            InternalDefinitionContract {
                export_name: function.export_name.clone(),
                signature,
            },
        ) {
            diagnostics.push(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(function),
                format!(
                    "internal semantic instance `{}` resolves to both `{}` and `{}`",
                    function.rust_path, previous.export_name, function.export_name
                ),
            ));
        }
        if let Some(previous_path) =
            internal_exports.insert(function.export_name.clone(), function.rust_path.clone())
        {
            diagnostics.push(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(function),
                format!(
                    "internal export symbol `{}` is defined by both `{}` and `{}`",
                    function.export_name, previous_path, function.rust_path
                ),
            ));
        }
    }
    if !diagnostics.is_empty() {
        return Err(errors(diagnostics));
    }

    for function in functions.iter().copied() {
        if function.kind != MirFunctionKind::KernelEntry {
            continue;
        }
        match kernel_ir_launch_contract(function) {
            Ok(workgroup_size) => {
                launch_contracts.insert(function.export_name.as_str(), workgroup_size);
            }
            Err(error) => diagnostics.push(error),
        }
    }
    if !diagnostics.is_empty() {
        return Err(errors(diagnostics));
    }

    for function in functions {
        if function.kind == MirFunctionKind::KernelEntry
            && !kernel_ids.insert(function.export_name.as_str())
        {
            diagnostics.push(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(function),
                format!("duplicate kernel export name `{}`", function.export_name),
            ));
            continue;
        }

        match FunctionLowerer::new(
            function,
            &mut declarations,
            &internal_definitions,
            launch_contracts
                .get(function.export_name.as_str())
                .copied()
                .flatten(),
            float_target,
            collective_target,
            strict_float_policy,
        )
        .lower()
        {
            Ok(definition) => {
                if function.kind == MirFunctionKind::KernelEntry {
                    kernel_entries.push((
                        function.export_name.clone(),
                        definition.id.clone(),
                        launch_contracts
                            .get(function.export_name.as_str())
                            .copied()
                            .flatten(),
                    ));
                }
                definitions.push(definition);
            }
            Err(error) => diagnostics.push(error),
        }
    }

    if !diagnostics.is_empty() {
        return Err(errors(diagnostics));
    }

    let definition_ids = definitions
        .iter()
        .map(|function| function.id.as_str().to_string())
        .collect::<BTreeSet<_>>();
    definitions.extend(
        declarations
            .into_iter()
            .filter(|(identity, _)| !definition_ids.contains(identity))
            .map(|(identity, signature)| {
                let id = FunctionId::new(identity.clone());
                if let Some(diagnostic) = AmdGpuDiagnosticOperation::from_intrinsic_id(&id) {
                    let declaration = diagnostic.declaration();
                    debug_assert_eq!(declaration.signature, signature);
                    declaration
                } else if let Some(float) = FloatOperation::from_intrinsic_id(&id) {
                    let declaration = float.declaration();
                    debug_assert_eq!(declaration.signature, signature);
                    declaration
                } else {
                    Function::declaration(identity, signature)
                }
            }),
    );

    let mut module = Module::new(MODULE_ID);
    module.functions = definitions;
    module.required_capabilities = module
        .functions
        .iter()
        .flat_map(|function| function.required_capabilities.iter().cloned())
        .collect();
    module.kernels = kernel_entries
        .into_iter()
        .map(|(kernel, entry, workgroup_size)| {
            let exact_target = gfx942_xnack_minus_target_capability();
            let retained_bindings = module
                .function(&entry)
                .into_iter()
                .flat_map(|function| &function.required_capabilities)
                .filter(|capability| {
                    *capability == &exact_target
                        || matches!(
                            capability,
                            TargetCapability::Extension { namespace, .. }
                                if matches!(
                                    namespace.as_str(),
                                    MATRIX_SOURCE_ABI_OBSERVATION_NAMESPACE_V2
                                        | MATRIX_PROJECTED_KERNARG_POLICY_NAMESPACE_V1
                                )
                        )
                })
                .cloned()
                .collect();
            let mut kernel = Kernel::new(
                kernel,
                entry,
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            );
            kernel.workgroup_size = workgroup_size;
            kernel.required_capabilities = retained_bindings;
            kernel
        })
        .collect();

    if let Err(verification_errors) = verify_module(&module) {
        let diagnostics = verification_errors
            .diagnostics()
            .iter()
            .map(|verification| TranslationDiagnostic {
                location: TranslationLocation {
                    function: verification
                        .location
                        .function
                        .as_ref()
                        .map(|function| function.as_str().to_string()),
                    block: verification.location.block.map(|block| block.0 as usize),
                    statement: None,
                    terminator: false,
                    operation: verification.location.operation,
                    source: None,
                },
                code: TranslationDiagnosticCode::VerificationFailed,
                message: format!("{:?}: {}", verification.code, verification.message),
            })
            .collect();
        return Err(errors(diagnostics));
    }

    Ok(module)
}

fn kernel_ir_launch_contract(
    function: &MirFunction,
) -> Result<Option<WorkgroupSize>, TranslationDiagnostic> {
    let typed_workgroup = function
        .typed_profile
        .map(|_| WorkgroupSize::new(256, 1, 1));
    let Some(authenticated) = &function.frontend_contract else {
        return Ok(typed_workgroup);
    };
    let contract = authenticated.contract();
    if contract.unsafe_assembly().is_some() {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedStatement,
            TranslationLocation::function(function),
            format!(
                "authenticated unsafe assembly from `{}` cannot enter kernel IR until AMDGPU inline-assembly lowering preserves its exact operands, options, and effects",
                authenticated.registration_path()
            ),
        ));
    }
    let Some(launch) = contract.launch() else {
        return Ok(typed_workgroup);
    };
    if launch.min_workgroups_per_compute_unit().is_some() {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedStatement,
            TranslationLocation::function(function),
            "authenticated minimum-workgroup occupancy cannot be represented by kernel IR V2",
        ));
    }
    let Some(required) = launch.required() else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedStatement,
            TranslationLocation::function(function),
            "authenticated maximum-only launch bounds cannot be represented by kernel IR V2",
        ));
    };
    if let Some(maximum) = launch.maximum()
        && maximum != required
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedStatement,
            TranslationLocation::function(function),
            "authenticated non-exact maximum launch bounds cannot be represented by kernel IR V2",
        ));
    }
    let [x, y, z] = required.as_array();
    let authenticated = WorkgroupSize::new(x, y, z);
    if let Some(typed) = typed_workgroup
        && authenticated != typed
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            TranslationLocation::function(function),
            "authenticated launch contract disagrees with the typed profile's exact 256x1x1 workgroup",
        ));
    }
    Ok(Some(authenticated))
}

fn errors(mut diagnostics: Vec<TranslationDiagnostic>) -> TranslationErrors {
    diagnostics.sort();
    TranslationErrors { diagnostics }
}

fn diagnostic(
    code: TranslationDiagnosticCode,
    location: TranslationLocation,
    message: impl Into<String>,
) -> TranslationDiagnostic {
    TranslationDiagnostic {
        location,
        code,
        message: message.into(),
    }
}

#[derive(Clone, Copy, Debug)]
enum LocalBinding {
    Value(ValueId),
    FieldlessEnum {
        discriminant: ValueId,
    },
    DeviceMathCapability,
    Gfx942CollectiveCapability,
    Gfx942StaticLdsU32x256(ValueId),
    DeviceMatrixValueCapability,
    DeviceMatrixReferenceCapability,
    Bf16MfmaFragment([ValueId; 4]),
    F32AccumulatorFragment([ValueId; 4]),
    OptionPointer {
        discriminant: ValueId,
        payload: ValueId,
        some_entry: Option<usize>,
    },
}

struct FunctionLowerer<'function, 'declarations> {
    function: &'function MirFunction,
    declarations: &'declarations mut BTreeMap<String, Signature>,
    internal_definitions:
        &'declarations BTreeMap<MirSemanticInstanceIdentity, InternalDefinitionContract>,
    locals: BTreeMap<usize, LocalBinding>,
    value_types: BTreeMap<ValueId, Type>,
    trusted_thread_indices: BTreeSet<ValueId>,
    guarded_pointer_values: BTreeMap<ValueId, usize>,
    return_type: Option<Type>,
    next_value: u32,
    trap_block: Option<BlockId>,
    workgroup_size: Option<WorkgroupSize>,
    float_target: Option<Gfx942FloatTarget>,
    collective_target: Option<Gfx942WaveLdsTargetV2>,
    strict_float_policy: StrictFloatPolicy,
    control_flow_ssa: control_flow_ssa::ControlFlowSsaPlan,
    block_parameters: BTreeMap<usize, BTreeMap<usize, ValueId>>,
    required_capabilities: BTreeSet<TargetCapability>,
}

impl<'function, 'declarations> FunctionLowerer<'function, 'declarations> {
    fn new(
        function: &'function MirFunction,
        declarations: &'declarations mut BTreeMap<String, Signature>,
        internal_definitions: &'declarations BTreeMap<
            MirSemanticInstanceIdentity,
            InternalDefinitionContract,
        >,
        workgroup_size: Option<WorkgroupSize>,
        float_target: Option<Gfx942FloatTarget>,
        collective_target: Option<Gfx942WaveLdsTargetV2>,
        strict_float_policy: StrictFloatPolicy,
    ) -> Self {
        Self {
            function,
            declarations,
            internal_definitions,
            locals: BTreeMap::new(),
            value_types: BTreeMap::new(),
            trusted_thread_indices: BTreeSet::new(),
            guarded_pointer_values: BTreeMap::new(),
            return_type: None,
            next_value: 0,
            trap_block: None,
            workgroup_size,
            float_target,
            collective_target,
            strict_float_policy,
            control_flow_ssa: control_flow_ssa::ControlFlowSsaPlan::default(),
            block_parameters: BTreeMap::new(),
            required_capabilities: BTreeSet::new(),
        }
    }

    fn lower(mut self) -> Result<Function, TranslationDiagnostic> {
        let signature = declared_function_signature(self.function)?;
        let mut args = self
            .function
            .locals
            .iter()
            .filter(|local| local.role == crate::mir_import::MirLocalRole::Arg)
            .collect::<Vec<_>>();
        args.sort_by_key(|local| local.index);
        if args.len() != self.function.arg_count {
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(self.function),
                format!(
                    "function declares {} arguments but imports {} argument locals",
                    self.function.arg_count,
                    args.len()
                ),
            ));
        }

        let mut parameter_values = Vec::with_capacity(signature.parameters.len());
        for arg in args {
            let types = lower_function_parameter_types(&arg.ty.shape).ok_or_else(|| {
                diagnostic(
                    TranslationDiagnosticCode::UnsupportedType,
                    TranslationLocation::function(self.function),
                    format!(
                        "argument local{} has unsupported type `{}`",
                        arg.index, arg.ty.rust
                    ),
                )
            })?;
            let values = types
                .into_iter()
                .map(|ty| {
                    let value =
                        self.fresh_value(ty, &TranslationLocation::function(self.function))?;
                    Ok(value.id)
                })
                .collect::<Result<Vec<_>, TranslationDiagnostic>>()?;
            let binding = match (arg.ty.shape.clone(), values.as_slice()) {
                (shape, [value]) if !is_matrix_fragment_shape(&shape) => {
                    LocalBinding::Value(*value)
                }
                (shape, [v0, v1, v2, v3])
                    if is_trusted_adt_shape(&shape, TrustedDeviceItem::Bf16MfmaFragment) =>
                {
                    LocalBinding::Bf16MfmaFragment([*v0, *v1, *v2, *v3])
                }
                (shape, [v0, v1, v2, v3])
                    if is_trusted_adt_shape(&shape, TrustedDeviceItem::F32AccumulatorFragment) =>
                {
                    LocalBinding::F32AccumulatorFragment([*v0, *v1, *v2, *v3])
                }
                _ => {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        TranslationLocation::function(self.function),
                        format!(
                            "argument local{} has an invalid lowered value shape",
                            arg.index
                        ),
                    ));
                }
            };
            self.bind_local(
                arg.index,
                binding,
                TranslationLocation::function(self.function),
            )?;
            parameter_values.extend(values);
        }

        let return_local = self
            .function
            .locals
            .iter()
            .find(|local| local.index == 0)
            .ok_or_else(|| {
                diagnostic(
                    TranslationDiagnosticCode::MalformedMir,
                    TranslationLocation::function(self.function),
                    "function has no return local0",
                )
            })?;
        if return_local.role != crate::mir_import::MirLocalRole::Return {
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(self.function),
                "local0 is not marked as the function return local",
            ));
        }
        let result_types = signature.results.clone();
        self.return_type = result_types.first().cloned();

        let mut source_blocks = self.function.blocks.iter().collect::<Vec<_>>();
        source_blocks.sort_by_key(|block| block.index);
        if source_blocks.first().map(|block| block.index) != Some(0) {
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(self.function),
                "kernel must contain entry block bb0",
            ));
        }
        let mut block_indices = BTreeSet::new();
        for block in &source_blocks {
            if !block_indices.insert(block.index) {
                return Err(diagnostic(
                    TranslationDiagnosticCode::MalformedMir,
                    TranslationLocation::block(self.function, block),
                    format!("basic block bb{} is imported more than once", block.index),
                ));
            }
            self.block_id(
                block.index,
                TranslationLocation::block(self.function, block),
            )?;
        }

        self.control_flow_ssa = control_flow_ssa::ControlFlowSsaPlan::analyze(
            self.function,
            self.float_target.is_some(),
        )?;
        for source_block in source_blocks
            .iter()
            .copied()
            .filter(|block| block.index != 0)
        {
            let mut parameters = BTreeMap::new();
            for local in self.control_flow_ssa.live_in(source_block.index).to_vec() {
                let ty = self
                    .control_flow_ssa
                    .ty(local)
                    .expect("live-in local is promoted")
                    .clone();
                let parameter =
                    self.fresh_value(ty, &TranslationLocation::block(self.function, source_block))?;
                parameters.insert(local, parameter.id);
            }
            self.block_parameters.insert(source_block.index, parameters);
        }

        if source_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|terminator| &terminator.kind),
                Some(MirTerminatorKind::Assert { .. })
            )
        }) {
            let next = source_blocks
                .last()
                .expect("entry block checked")
                .index
                .checked_add(1)
                .ok_or_else(|| {
                    diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        TranslationLocation::function(self.function),
                        "cannot allocate assertion failure block",
                    )
                })?;
            self.trap_block =
                Some(self.block_id(next, TranslationLocation::function(self.function))?);
        }

        let mut blocks =
            Vec::with_capacity(source_blocks.len() + usize::from(self.trap_block.is_some()));
        for source_block in source_blocks {
            blocks.push(self.lower_block(source_block)?);
        }
        if let Some(trap) = self.trap_block {
            let mut block = BasicBlock::new(trap);
            block.terminator = Some(Terminator::Unreachable);
            blocks.push(block);
        }

        let mut definition = match self.function.kind {
            MirFunctionKind::KernelEntry => Function::kernel_entry(
                self.function.rust_path.clone(),
                signature,
                parameter_values,
                blocks,
            ),
            MirFunctionKind::InternalHelper => Function::internal_helper(
                self.function.export_name.clone(),
                signature,
                parameter_values,
                blocks,
            ),
            MirFunctionKind::DeviceFfiExport => Function::device_ffi_export(
                self.function.export_name.clone(),
                signature,
                parameter_values,
                blocks,
            ),
        };
        let mut required_capabilities = definition.derived_capabilities();
        required_capabilities.extend(self.required_capabilities);
        definition.required_capabilities = required_capabilities;
        Ok(definition)
    }

    fn is_exact_general_v3_alpha_zeta_context(&self) -> bool {
        if !self.is_general_v3_profile_context() {
            return false;
        }
        let mut arguments = self
            .function
            .locals
            .iter()
            .filter(|local| local.role == crate::mir_import::MirLocalRole::Arg)
            .collect::<Vec<_>>();
        arguments.sort_by_key(|local| local.index);
        match (self.function.export_name.as_str(), arguments.as_slice()) {
            ("alpha", [scale, input, output]) => {
                scale.ty.shape == MirTypeShape::F32
                    && is_readonly_f32_slice(&input.ty.shape)
                    && is_disjoint_f32_slice(&output.ty.shape)
            }
            ("zeta", [a, b, bias, output]) => {
                is_readonly_f32_slice(&a.ty.shape)
                    && is_readonly_f32_slice(&b.ty.shape)
                    && bias.ty.shape == MirTypeShape::F32
                    && is_disjoint_f32_slice(&output.ty.shape)
            }
            _ => false,
        }
    }

    fn is_general_v3_profile_context(&self) -> bool {
        self.function.kind == MirFunctionKind::KernelEntry
            && self.function.typed_profile
                == Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3)
    }
    fn is_gfx942_memory_v1_context(&self) -> bool {
        self.is_general_v3_profile_context() && self.float_target.is_some()
    }

    fn gfx942_collective_workgroup_size(&self) -> Option<u32> {
        let workgroup = self.workgroup_size?;
        (self.is_general_v3_profile_context()
            && self.float_target.is_some()
            && workgroup.y == 1
            && workgroup.z == 1
            && workgroup.x != 0
            && workgroup.x <= 256
            && workgroup.x.is_power_of_two())
        .then_some(workgroup.x)
    }

    fn is_gfx942_wave64_collective_context(&self) -> bool {
        self.collective_target
            .and(self.gfx942_collective_workgroup_size())
            .is_some_and(|size| size.is_multiple_of(64))
    }

    fn is_gfx942_collective_v1_context(&self) -> bool {
        self.collective_target.is_some() && self.gfx942_collective_workgroup_size().is_some()
    }

    fn is_exact_gfx942_wave64_matrix_context(&self) -> bool {
        self.function.kind == MirFunctionKind::KernelEntry
            && self.collective_target.is_some()
            && self.workgroup_size == Some(WorkgroupSize::new(64, 1, 1))
    }

    fn lower_block(&mut self, source: &MirBlock) -> Result<BasicBlock, TranslationDiagnostic> {
        let mut block = BasicBlock::new(self.block_id(
            source.index,
            TranslationLocation::block(self.function, source),
        )?);
        let promoted = self.control_flow_ssa.promoted_locals().collect::<Vec<_>>();
        if source.index == 0 {
            for local in promoted {
                if self
                    .function
                    .locals
                    .iter()
                    .find(|candidate| candidate.index == local)
                    .is_none_or(|local| local.role != crate::mir_import::MirLocalRole::Arg)
                {
                    self.locals.remove(&local);
                }
            }
        } else {
            for local in promoted {
                self.locals.remove(&local);
            }
            for (&local, &id) in self
                .block_parameters
                .get(&source.index)
                .expect("non-entry block parameter map")
            {
                let ty = self
                    .control_flow_ssa
                    .ty(local)
                    .expect("block parameter local is promoted")
                    .clone();
                block.parameters.push(ValueDef::new(id, ty));
                let binding = match self.control_flow_ssa.kind(local) {
                    Some(control_flow_ssa::PromotedLocalKind::Scalar) => LocalBinding::Value(id),
                    Some(control_flow_ssa::PromotedLocalKind::FieldlessEnum) => {
                        LocalBinding::FieldlessEnum { discriminant: id }
                    }
                    None => unreachable!("block parameter local is promoted"),
                };
                self.locals.insert(local, binding);
            }
        }
        for statement in &source.statements {
            self.lower_statement(source.index, statement, &mut block)?;
        }
        let terminator = source.terminator.as_ref().ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::block(self.function, source),
                "basic block has no terminator",
            )
        })?;
        block.terminator = Some(self.lower_terminator(source.index, terminator, &mut block)?);
        Ok(block)
    }

    fn lower_statement(
        &mut self,
        block_index: usize,
        statement: &MirStatement,
        block: &mut BasicBlock,
    ) -> Result<(), TranslationDiagnostic> {
        let location = TranslationLocation::statement(self.function, block_index, statement);
        match statement.kind {
            MirStatementKind::StorageLive
            | MirStatementKind::StorageDead
            | MirStatementKind::Coverage
            | MirStatementKind::Nop => return Ok(()),
            MirStatementKind::Retag => {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedStatement,
                    location,
                    "legacy payload-free retag MIR is not lowerable because its retag kind and place are absent",
                ));
            }
            MirStatementKind::Assume => {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedStatement,
                    location,
                    "rustc assume MIR is semantically imported but direct Kernel IR lowering remains disabled",
                ));
            }
            MirStatementKind::CopyNonOverlapping => {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedStatement,
                    location,
                    "rustc copy_nonoverlapping MIR is recognized but compiler import remains disabled until a real source path supplies exact pointee layout, address space, and unsafe obligations",
                ));
            }
            MirStatementKind::Assign => {}
            _ => {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedStatement,
                    location,
                    format!("unsupported MIR statement kind: {:?}", statement.kind),
                ));
            }
        }

        let destination = statement.destination.as_ref().ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                "assignment has no destination",
            )
        })?;
        let rvalue = statement.rvalue.ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                "assignment has no structured rvalue",
            )
        })?;

        let assignment = semantic_lowering::SemanticAssignment::new(
            rvalue,
            destination,
            &statement.operands,
            &location,
        );
        match semantic_lowering::lower_assignment(self, assignment, block) {
            semantic_lowering::LoweringOutcome::NotOwned => {}
            semantic_lowering::LoweringOutcome::Lowered(()) => return Ok(()),
            semantic_lowering::LoweringOutcome::Reject(diagnostic) => return Err(diagnostic),
        }

        match rvalue {
            MirRvalueKind::Ref => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedRvalue,
                location,
                "legacy payload-free reference MIR is not lowerable because its borrow kind is absent",
            )),
            MirRvalueKind::Reference(borrow_kind) => {
                let semantics = borrow_kind.reference_semantics_v3().ok_or_else(|| {
                    diagnostic(
                        TranslationDiagnosticCode::UnsupportedRvalue,
                        location.clone(),
                        format!(
                            "reference borrow kind {borrow_kind:?} is not lowerable because Kernel IR does not preserve its alias semantics"
                        ),
                    )
                })?;
                let expected_mutable = semantics == MirReferenceSemantics::Mutable;
                if !matches!(
                    self.imported_local_shape(destination.local),
                    Some(MirTypeShape::Reference { mutable, .. }) if *mutable == expected_mutable
                ) {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location,
                        "reference borrow kind does not match the destination reference mutability preserved by Kernel IR",
                    ));
                }
                let [MirOperandRef::Place(place)] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "reference assignment must have one place operand",
                    ));
                };
                if matches!(
                    self.locals.get(&place.local),
                    Some(LocalBinding::DeviceMathCapability)
                        | Some(LocalBinding::Gfx942CollectiveCapability)
                        | Some(LocalBinding::Gfx942StaticLdsU32x256(_))
                ) {
                    if !place.projection.is_empty() {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedProjection,
                            location,
                            "projected reference rvalues are not supported",
                        ));
                    }
                    let binding = self.locals[&place.local];
                    return self.bind_local(destination.local, binding, location);
                }
                if matches!(
                    self.locals.get(&place.local),
                    Some(LocalBinding::DeviceMatrixValueCapability)
                ) {
                    let exact_value = place.projection.is_empty()
                        && matches!(
                            self.imported_local_shape(place.local),
                            Some(shape) if is_trusted_adt_shape(shape, TrustedDeviceItem::DeviceMatrix)
                        );
                    let exact_reference = destination.projection.is_empty()
                        && matches!(
                            self.imported_local_shape(destination.local),
                            Some(MirTypeShape::Reference { pointee, mutable: false })
                                if is_trusted_adt_shape(pointee, TrustedDeviceItem::DeviceMatrix)
                        );
                    if !exact_value || !exact_reference {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location,
                            "DeviceMatrix autoref requires an unprojected DeviceMatrix source and exact unprojected &DeviceMatrix destination",
                        ));
                    }
                    return self.bind_local(
                        destination.local,
                        LocalBinding::DeviceMatrixReferenceCapability,
                        location,
                    );
                }
                if !place.projection.is_empty() {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedProjection,
                        location,
                        "projected reference rvalues are not supported",
                    ));
                }
                let value = self.plain_local(place.local, &location)?;
                self.bind_plain_destination(destination, value, location)
            }
            MirRvalueKind::Use => {
                let [operand] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "use assignment must have one operand",
                    ));
                };
                let value = self.lower_operand(operand, block, &location)?;
                self.assign_value(destination, value, block, location)
            }
            MirRvalueKind::Discriminant => {
                let [MirOperandRef::Place(place)] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "discriminant assignment must have one place operand",
                    ));
                };
                if !place.projection.is_empty() {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedProjection,
                        location,
                        "projected discriminants are not supported",
                    ));
                }
                let binding = self
                    .locals
                    .get(&place.local)
                    .copied()
                    .ok_or_else(|| self.undefined_local(place.local, location.clone()))?;
                let discriminant = match binding {
                    LocalBinding::OptionPointer { discriminant, .. }
                    | LocalBinding::FieldlessEnum { discriminant } => discriminant,
                    LocalBinding::Value(_)
                    | LocalBinding::DeviceMathCapability
                    | LocalBinding::Gfx942CollectiveCapability
                    | LocalBinding::Gfx942StaticLdsU32x256(_)
                    | LocalBinding::DeviceMatrixValueCapability
                    | LocalBinding::DeviceMatrixReferenceCapability
                    | LocalBinding::Bf16MfmaFragment(_)
                    | LocalBinding::F32AccumulatorFragment(_) => {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location,
                            "discriminant operand is not a translated Option pointer or authenticated fieldless enum",
                        ));
                    }
                };
                self.bind_plain_destination(destination, discriminant, location)
            }
            MirRvalueKind::FieldlessEnumVariant(discriminant) => {
                if self.float_target.is_none() {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedRvalue,
                        location,
                        "fieldless enum construction is supported only for the exact gfx942 target profile",
                    ));
                }
                if !destination.projection.is_empty()
                    || !matches!(
                        self.local_shape(destination.local, &location)?,
                        MirTypeShape::Adt { .. }
                    )
                {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location,
                        "authenticated fieldless enum construction requires an unprojected ADT local",
                    ));
                }
                let value = self.emit_result(
                    block,
                    Type::Scalar(ScalarType::I64),
                    OperationKind::Constant(Constant::I64(discriminant)),
                    &location,
                )?;
                self.bind_local(
                    destination.local,
                    LocalBinding::FieldlessEnum {
                        discriminant: value,
                    },
                    location,
                )
            }
            MirRvalueKind::Unary(MirUnaryOp::PtrMetadata) => {
                let [operand] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "PtrMetadata must have one operand",
                    ));
                };
                let slice = self.lower_operand(operand, block, &location)?;
                let result = self.emit_result(
                    block,
                    Type::INDEX,
                    OperationKind::SliceLength { slice },
                    &location,
                )?;
                self.bind_plain_destination(destination, result, location)
            }
            MirRvalueKind::Binary(MirBinaryOp::Add) => {
                let [lhs, rhs] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "add must have two operands",
                    ));
                };
                let lhs = self.lower_operand(lhs, block, &location)?;
                let rhs = self.lower_operand(rhs, block, &location)?;
                let ty = self.value_type(lhs, &location)?.clone();
                if self.is_exact_general_v3_alpha_zeta_context() {
                    let rhs_ty = self.value_type(rhs, &location)?;
                    if ty != Type::F32 || rhs_ty != &Type::F32 {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location,
                            format!(
                                "exact General V3 alpha/zeta addition requires two f32 operands; found {ty:?} and {rhs_ty:?}"
                            ),
                        ));
                    }
                    if self.float_target.is_none() {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedRvalue,
                            location,
                            "f32 addition requires the exact gfx942 floating-point profile",
                        ));
                    }
                    self.require_strict_float_policy(&location)?;
                }
                let result = self.emit_result(
                    block,
                    ty,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs,
                        rhs,
                    },
                    &location,
                )?;
                self.assign_value(destination, result, block, location)
            }
            MirRvalueKind::Binary(MirBinaryOp::Lt) => {
                let [lhs, rhs] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "less-than comparison must have two operands",
                    ));
                };
                let lhs = self.lower_operand(lhs, block, &location)?;
                let rhs = self.lower_operand(rhs, block, &location)?;
                let result = self.emit_result(
                    block,
                    Type::BOOL,
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs,
                        rhs,
                    },
                    &location,
                )?;
                self.bind_plain_destination(destination, result, location)
            }
            MirRvalueKind::Binary(MirBinaryOp::Mul) => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedRvalue,
                location,
                format!(
                    "f32 multiply requires an exact General V3 alpha/zeta kernel context and supported assignment; found export {:?}, kind {:?}, profile {:?}, argument shapes {:?}, destination {:?}, operands {:?}",
                    self.function.export_name,
                    self.function.kind,
                    self.function.typed_profile,
                    self.function
                        .locals
                        .iter()
                        .filter(|local| local.role == crate::mir_import::MirLocalRole::Arg)
                        .map(|local| (local.index, &local.ty.shape))
                        .collect::<Vec<_>>(),
                    destination,
                    statement.operands,
                ),
            )),
            unsupported => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedRvalue,
                location,
                format!("unsupported structured MIR rvalue: {unsupported:?}"),
            )),
        }
    }

    fn lower_terminator(
        &mut self,
        block_index: usize,
        terminator: &MirTerminator,
        block: &mut BasicBlock,
    ) -> Result<Terminator, TranslationDiagnostic> {
        let location = TranslationLocation::terminator(self.function, block_index, terminator);
        let semantic_terminator =
            semantic_lowering::SemanticTerminator::new(&terminator.kind, &location);
        match semantic_lowering::lower_terminator(self, semantic_terminator, block) {
            semantic_lowering::LoweringOutcome::NotOwned => {}
            semantic_lowering::LoweringOutcome::Lowered(terminator) => return Ok(terminator),
            semantic_lowering::LoweringOutcome::Reject(diagnostic) => return Err(diagnostic),
        }

        match &terminator.kind {
            MirTerminatorKind::Return => {
                let values = match self.return_type.clone() {
                    Some(expected) => {
                        let value = self.plain_local(0, &location)?;
                        let actual = self.value_type(value, &location)?;
                        if actual != &expected {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::UnsupportedType,
                                location,
                                format!("return local0 has type {actual:?}, expected {expected:?}"),
                            ));
                        }
                        vec![value]
                    }
                    None => Vec::new(),
                };
                Ok(Terminator::Return { values })
            }
            MirTerminatorKind::Unreachable => Ok(Terminator::Unreachable),
            MirTerminatorKind::SwitchInt {
                discriminant,
                targets,
                otherwise,
            } => {
                let selector = self.lower_operand(discriminant, block, &location)?;
                if self.is_exact_general_v3_alpha_zeta_context()
                    && self.value_type(selector, &location)? == &Type::BOOL
                {
                    let zero = targets.iter().find(|target| target.value == 0);
                    let one = targets.iter().find(|target| target.value == 1);
                    let exhaustive = targets.len() == 2
                        && zero.is_some()
                        && one.is_some()
                        && zero.map(|target| target.target) != one.map(|target| target.target)
                        && self.function.blocks.iter().any(|block| {
                            block.index == *otherwise
                                && matches!(
                                    block.terminator.as_ref().map(|terminator| &terminator.kind),
                                    Some(MirTerminatorKind::Unreachable)
                                )
                        });
                    if !exhaustive {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedStatement,
                            location,
                            "boolean switch must have exact 0/1 cases and an unreachable default",
                        ));
                    }
                    let some_entry = one.expect("checked above").target;
                    if !mir_block_dominates(self.function, block_index, some_entry) {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedStatement,
                            location,
                            "Option Some edge is not dominated by its bounds predicate",
                        ));
                    }
                    let mut option_locals = self.locals.iter().filter_map(|(local, binding)| {
                        matches!(
                            binding,
                            LocalBinding::OptionPointer { discriminant, .. }
                                if *discriminant == selector
                        )
                        .then_some(*local)
                    });
                    let option_local = option_locals.next();
                    if option_local.is_none() || option_locals.next().is_some() {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedStatement,
                            location,
                            "boolean Option switch is not bound to exactly one translated get_mut result",
                        ));
                    }
                    let option_local = option_local.expect("checked above");
                    let LocalBinding::OptionPointer {
                        discriminant,
                        payload,
                        ..
                    } = self.locals[&option_local]
                    else {
                        unreachable!("selected an Option pointer above")
                    };
                    self.locals.insert(
                        option_local,
                        LocalBinding::OptionPointer {
                            discriminant,
                            payload,
                            some_entry: Some(some_entry),
                        },
                    );
                    return Ok(Terminator::ConditionalBranch {
                        condition: selector,
                        then_target: self.block_id(some_entry, location.clone())?,
                        then_arguments: self.edge_arguments(some_entry, &location)?,
                        else_target: self
                            .block_id(zero.expect("checked above").target, location.clone())?,
                        else_arguments: self
                            .edge_arguments(zero.expect("checked above").target, &location)?,
                    });
                }
                let mut cases = targets
                    .iter()
                    .map(|target| {
                        Ok(SwitchCase {
                            value: u64::try_from(target.value).map_err(|_| {
                                diagnostic(
                                    TranslationDiagnosticCode::UnsupportedType,
                                    location.clone(),
                                    format!(
                                        "switch value {} does not fit kernel IR's u64 cases",
                                        target.value
                                    ),
                                )
                            })?,
                            target: self.block_id(target.target, location.clone())?,
                            arguments: self.edge_arguments(target.target, &location)?,
                        })
                    })
                    .collect::<Result<Vec<_>, TranslationDiagnostic>>()?;
                cases.sort_by_key(|case| (case.value, case.target));
                Ok(Terminator::Switch {
                    selector,
                    cases,
                    default_target: self.block_id(*otherwise, location.clone())?,
                    default_arguments: self.edge_arguments(*otherwise, &location)?,
                })
            }
            MirTerminatorKind::Call {
                callee,
                target,
                destination,
                operands,
            } => self.lower_call(
                callee.as_ref(),
                *target,
                destination.as_ref(),
                operands,
                block,
                location,
            ),
            MirTerminatorKind::Assert {
                condition,
                expected,
                target,
            } => {
                let condition = self.lower_operand(condition, block, &location)?;
                let success = self.block_id(*target, location.clone())?;
                let failure = self.trap_block.ok_or_else(|| {
                    diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location.clone(),
                        "assertion failure block was not allocated",
                    )
                })?;
                let (then_target, else_target) = if *expected {
                    (success, failure)
                } else {
                    (failure, success)
                };
                let success_arguments = self.edge_arguments(*target, &location)?;
                Ok(Terminator::ConditionalBranch {
                    condition,
                    then_target,
                    then_arguments: if *expected {
                        success_arguments.clone()
                    } else {
                        Vec::new()
                    },
                    else_target,
                    else_arguments: if *expected {
                        Vec::new()
                    } else {
                        success_arguments
                    },
                })
            }
            unsupported => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedStatement,
                location,
                format!("unsupported MIR terminator: {unsupported:?}"),
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_call(
        &mut self,
        callee: Option<&MirCallee>,
        target: Option<usize>,
        destination: Option<&MirPlaceRef>,
        operands: &[MirOperandRef],
        block: &mut BasicBlock,
        location: TranslationLocation,
    ) -> Result<Terminator, TranslationDiagnostic> {
        let callee = callee.ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedCall,
                location.clone(),
                "indirect calls are not supported",
            )
        })?;
        let target = target.ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedCall,
                location.clone(),
                format!(
                    "call to `{}` has no normal return target",
                    callee.identity()
                ),
            )
        })?;
        let destination = destination.ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                format!("call to `{}` has no destination", callee.identity()),
            )
        })?;
        if !destination.projection.is_empty() {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location,
                "projected call destinations are not supported",
            ));
        }
        if let Some(marker) = callee.rejected_provider_marker() {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedCall,
                location,
                format!(
                    "trusted-provider rejection: diagnostic item `{marker}` is defined by `{}` instead of the external `fe2o3_device` provider",
                    callee.identity()
                ),
            ));
        }

        if let Some(TrustedDeviceItem::DeviceMath(math)) = callee.trusted_item() {
            return self.lower_device_math_call(
                callee,
                math,
                target,
                destination,
                operands,
                block,
                location,
            );
        }
        if let Some(TrustedDeviceItem::HalfOperation(operation)) = callee.trusted_item() {
            return self.lower_half_operation_call(
                callee,
                operation,
                target,
                destination,
                operands,
                block,
                location,
            );
        }
        if let Some(call) = semantic_lowering::SessionRecognizedSemanticCall::new(
            callee,
            target,
            destination,
            operands,
            &location,
        ) {
            match semantic_lowering::lower_call(self, call, block) {
                semantic_lowering::LoweringOutcome::NotOwned => {}
                semantic_lowering::LoweringOutcome::Lowered(terminator) => {
                    return Ok(terminator);
                }
                semantic_lowering::LoweringOutcome::Reject(diagnostic) => {
                    return Err(diagnostic);
                }
            }
        }

        let arguments = operands
            .iter()
            .map(|operand| self.lower_operand(operand, block, &location))
            .collect::<Result<Vec<_>, _>>()?;
        let argument_types = arguments
            .iter()
            .map(|value| self.value_type(*value, &location).cloned())
            .collect::<Result<Vec<_>, _>>()?;

        let trusted_item = callee.trusted_item();
        let external_import = callee.external_import_evidence();
        let internal_identity = match callee.semantic_instance_identity() {
            Some(Ok(identity)) => Some(identity),
            Some(Err(detail)) => {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedCall,
                    location,
                    format!(
                        "callee `{}` has no structured semantic instance identity: {detail}",
                        callee.identity()
                    ),
                ));
            }
            None => None,
        };
        let internal_definition = internal_identity
            .and_then(|identity| self.internal_definitions.get(identity))
            .cloned();
        let mut call_identity = callee.identity().to_string();
        let result_types = if let Some(import) = external_import {
            if !import.effects().is_none() {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedCall,
                    location,
                    format!(
                        "device FFI import `{}` has effects that kernel IR cannot preserve",
                        callee.identity()
                    ),
                ));
            }
            let signature = lower_device_ffi_signature(import.physical_abi());
            self.require_call_types(
                callee,
                &argument_types,
                &signature.parameters,
                location.clone(),
            )?;
            signature.results
        } else if let Some(definition) = &internal_definition {
            self.require_call_types(
                callee,
                &argument_types,
                &definition.signature.parameters,
                location.clone(),
            )?;
            call_identity.clone_from(&definition.export_name);
            definition.signature.results.clone()
        } else {
            match trusted_item {
                Some(TrustedDeviceItem::ThreadIndex1d) => {
                    self.require_call_types(callee, &argument_types, &[], location.clone())?;
                    vec![Type::INDEX]
                }
                Some(TrustedDeviceItem::ThreadIndexGet) => {
                    self.require_call_types(
                        callee,
                        &argument_types,
                        &[Type::INDEX],
                        location.clone(),
                    )?;
                    vec![Type::INDEX]
                }
                Some(
                    TrustedDeviceItem::ThreadIndexOffset | TrustedDeviceItem::ThreadIndexStride,
                ) => {
                    self.require_call_types(
                        callee,
                        &argument_types,
                        &[Type::INDEX, Type::INDEX],
                        location.clone(),
                    )?;
                    vec![Type::INDEX]
                }
                Some(TrustedDeviceItem::ThreadIndexOffsetSigned) => {
                    self.require_call_types(
                        callee,
                        &argument_types,
                        &[Type::INDEX, Type::Scalar(ScalarType::I64)],
                        location.clone(),
                    )?;
                    vec![Type::INDEX]
                }
                Some(TrustedDeviceItem::ThreadIndexStrideOffset) => {
                    self.require_call_types(
                        callee,
                        &argument_types,
                        &[Type::INDEX, Type::INDEX, Type::Scalar(ScalarType::I64)],
                        location.clone(),
                    )?;
                    vec![Type::INDEX]
                }
                Some(
                    TrustedDeviceItem::DisjointSliceGetMut
                    | TrustedDeviceItem::DisjointSliceGetMutAt,
                ) => {
                    if arguments.len() != 2 {
                        return Err(self.call_arity(callee, 2, arguments.len(), location.clone()));
                    }
                    let Type::Slice(slice) = &argument_types[0] else {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location.clone(),
                            format!(
                                "callee `{}` receiver is not a translated slice",
                                callee.identity()
                            ),
                        ));
                    };
                    if slice.access != AccessMode::ReadWrite {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location.clone(),
                            format!("callee `{}` receiver must be writable", callee.identity()),
                        ));
                    }
                    if argument_types[1] != Type::INDEX {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location.clone(),
                            format!(
                                "callee `{}` index must lower to index type",
                                callee.identity()
                            ),
                        ));
                    }
                    vec![
                        Type::INDEX,
                        Type::pointer((*slice.element).clone(), slice.address_space, slice.access),
                    ]
                }
                Some(
                    TrustedDeviceItem::DisjointSlice
                    | TrustedDeviceItem::DeviceGlobalMutPtr
                    | TrustedDeviceItem::WorkgroupLdsScope
                    | TrustedDeviceItem::ThreadIndex
                    | TrustedDeviceItem::Gfx942CollectivesContext
                    | TrustedDeviceItem::Gfx942StaticLdsU32x256Type
                    | TrustedDeviceItem::DeviceMatrix
                    | TrustedDeviceItem::Bf16MfmaFragment
                    | TrustedDeviceItem::F32AccumulatorFragment,
                ) => {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedCall,
                        location,
                        format!(
                            "trusted device item `{}` is a type, not a callable helper",
                            callee.identity()
                        ),
                    ));
                }
                Some(
                    TrustedDeviceItem::Bf16MfmaFragmentFromBits
                    | TrustedDeviceItem::F32AccumulatorFragmentFromValues
                    | TrustedDeviceItem::F32AccumulatorFragmentIntoValues
                    | TrustedDeviceItem::WaveLaneFromRaw
                    | TrustedDeviceItem::Gfx942LdsBf16TilePairM16x16
                    | TrustedDeviceItem::LdsTile16x16AssumeInit
                    | TrustedDeviceItem::LdsTile16x16WriteMfmaBf16
                    | TrustedDeviceItem::LdsTile16x16ReadMfmaBf16
                    | TrustedDeviceItem::WorkgroupSyncthreads
                    | TrustedDeviceItem::DynamicLdsExactFromCompiler
                    | TrustedDeviceItem::DisjointSliceLen,
                ) => {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedCall,
                        location,
                        format!(
                            "trusted device item `{}` is reserved for the source-authenticated collected tiled GEMM V1 profile",
                            callee.identity()
                        ),
                    ));
                }
                Some(TrustedDeviceItem::GeneralGemm(_, _)) => {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedCall,
                        location,
                        format!(
                            "trusted device item `{}` requires authenticated general GEMM MIR import",
                            callee.identity()
                        ),
                    ));
                }
                Some(TrustedDeviceItem::DeviceValue(_)) => {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedCall,
                        location,
                        format!(
                            "trusted half value item `{}` is a type, not a callable helper",
                            callee.identity()
                        ),
                    ));
                }
                Some(TrustedDeviceItem::DeviceMath(_)) => {
                    unreachable!("device math calls are handled before ordinary argument lowering")
                }
                Some(TrustedDeviceItem::HalfOperation(_)) => {
                    unreachable!("half operations are handled before ordinary argument lowering")
                }
                Some(
                    TrustedDeviceItem::MemoryOffsetFrom
                    | TrustedDeviceItem::MemoryVolatileLoad
                    | TrustedDeviceItem::MemoryVolatileStore
                    | TrustedDeviceItem::MemoryCopyNonOverlapping,
                ) => {
                    unreachable!("memory operations are handled by semantic lowering")
                }
                Some(
                    TrustedDeviceItem::Gfx942CollectivesFromCompiler
                    | TrustedDeviceItem::Gfx942StaticLdsU32x256
                    | TrustedDeviceItem::Gfx942Wave64ReduceActiveU32
                    | TrustedDeviceItem::Gfx942Workgroup256ReduceActiveU32
                    | TrustedDeviceItem::Gfx942Wave64ReduceSum
                    | TrustedDeviceItem::Gfx942Wave64InclusiveScanSum
                    | TrustedDeviceItem::Gfx942Wave64ExclusiveScanSum
                    | TrustedDeviceItem::Gfx942WorkgroupReduceSum
                    | TrustedDeviceItem::Gfx942WorkgroupInclusiveScanSum
                    | TrustedDeviceItem::Gfx942WorkgroupExclusiveScanSum
                    | TrustedDeviceItem::Gfx942BarrierArrive
                    | TrustedDeviceItem::Gfx942BarrierWait
                    | TrustedDeviceItem::DeviceMatrixFromCompiler
                    | TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
                ) => {
                    unreachable!("collective operations are handled by semantic lowering")
                }
                Some(
                    TrustedDeviceItem::AmdGpuInline(_) | TrustedDeviceItem::AmdGpuDiagnostic(_),
                ) => unreachable!("AMDGPU diagnostics are handled by semantic lowering"),
                None => {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedCall,
                        location,
                        format!(
                            "callee `{}` has no classified trusted device identity",
                            callee.identity()
                        ),
                    ));
                }
            }
        };

        let signature = Signature::new(argument_types, result_types.clone());
        self.register_declaration_identity(&call_identity, signature, &location)?;
        let results = result_types
            .into_iter()
            .map(|ty| self.fresh_value(ty, &location))
            .collect::<Result<Vec<_>, _>>()?;
        block.operations.push(Operation::new(
            results.clone(),
            OperationKind::Call {
                callee: FunctionId::new(call_identity),
                arguments,
            },
        ));

        match results.as_slice() {
            [] if external_import.is_some() || internal_definition.is_some() => {}
            [result] => {
                if internal_definition.is_some() {
                    self.require_destination_type(destination, &result.ty, &location)?;
                }
                self.bind_local(
                    destination.local,
                    LocalBinding::Value(result.id),
                    location.clone(),
                )?;
            }
            [discriminant, payload]
                if matches!(
                    trusted_item,
                    Some(
                        TrustedDeviceItem::DisjointSliceGetMut
                            | TrustedDeviceItem::DisjointSliceGetMutAt
                    )
                ) =>
            {
                self.bind_local(
                    destination.local,
                    LocalBinding::OptionPointer {
                        discriminant: discriminant.id,
                        payload: payload.id,
                        some_entry: None,
                    },
                    location.clone(),
                )?
            }
            _ => {
                return Err(diagnostic(
                    TranslationDiagnosticCode::MalformedMir,
                    location,
                    "known call produced an unexpected result shape",
                ));
            }
        }

        Ok(Terminator::Branch {
            target: self.block_id(target, location.clone())?,
            arguments: self.edge_arguments(target, &location)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_device_math_call(
        &mut self,
        callee: &MirCallee,
        item: DeviceMathDiagnosticItem,
        target: usize,
        destination: &MirPlaceRef,
        operands: &[MirOperandRef],
        block: &mut BasicBlock,
        location: TranslationLocation,
    ) -> Result<Terminator, TranslationDiagnostic> {
        if self.float_target.is_none() {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedCall,
                location,
                format!(
                    "trusted device math item `{}` requires the exact gfx942 floating-point profile",
                    callee.identity()
                ),
            ));
        }
        self.require_strict_float_policy(&location)?;

        if item == DeviceMathDiagnosticItem::Context {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedCall,
                location,
                "DeviceMath is a capability type, not a callable operation",
            ));
        }

        if item == DeviceMathDiagnosticItem::ContextFromCompiler {
            if !operands.is_empty() {
                return Err(self.call_arity(callee, 0, operands.len(), location));
            }
            self.require_destination_shape(destination, &MirTypeShape::DeviceMath, &location)?;
            self.bind_local(
                destination.local,
                LocalBinding::DeviceMathCapability,
                location.clone(),
            )?;
            return Ok(Terminator::Branch {
                target: self.block_id(target, location.clone())?,
                arguments: self.edge_arguments(target, &location)?,
            });
        }

        let Some((receiver, numerical)) = operands.split_first() else {
            return Err(self.call_arity(callee, 1, 0, location));
        };
        self.require_device_math_receiver(receiver, &location)?;
        let arguments = numerical
            .iter()
            .map(|operand| self.lower_operand(operand, block, &location))
            .collect::<Result<Vec<_>, _>>()?;
        let float = recognized_device_math_operation(item, &arguments).map_err(|error| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedCall,
                location.clone(),
                format!(
                    "trusted device math call `{}` has an invalid semantic shape: {error:?}",
                    callee.identity()
                ),
            )
        })?;
        self.require_float_argument_types(&float, &location)?;
        self.require_destination_type(destination, &float.result_type(), &location)?;

        let declaration = float.declaration();
        self.register_declaration_identity(
            declaration.id.as_str(),
            declaration.signature.clone(),
            &location,
        )?;
        let result = self.fresh_value(float.result_type(), &location)?;
        block.operations.push(float.operation(result.id));
        self.bind_local(
            destination.local,
            LocalBinding::Value(result.id),
            location.clone(),
        )?;
        Ok(Terminator::Branch {
            target: self.block_id(target, location.clone())?,
            arguments: self.edge_arguments(target, &location)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_half_operation_call(
        &mut self,
        callee: &MirCallee,
        operation: TrustedHalfOperation,
        target: usize,
        destination: &MirPlaceRef,
        operands: &[MirOperandRef],
        block: &mut BasicBlock,
        location: TranslationLocation,
    ) -> Result<Terminator, TranslationDiagnostic> {
        if self.float_target.is_none() {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedCall,
                location,
                format!(
                    "trusted half operation `{}` requires the exact gfx942 floating-point profile",
                    callee.identity()
                ),
            ));
        }
        self.require_strict_float_policy(&location)?;

        let arguments = operands
            .iter()
            .map(|operand| self.lower_operand(operand, block, &location))
            .collect::<Result<Vec<_>, _>>()?;
        let float = match operation {
            TrustedHalfOperation::FromF32(format) => {
                let [value] = arguments.as_slice() else {
                    return Err(self.call_arity(callee, 1, arguments.len(), location));
                };
                FloatOperation::Convert {
                    kind: match format {
                        fe2o3_kernel_ir::NarrowFloatFormat::F16 => {
                            FloatConversionKind::F32ToF16RoundTiesEven
                        }
                        fe2o3_kernel_ir::NarrowFloatFormat::Bf16 => {
                            FloatConversionKind::F32ToBf16RoundTiesEven
                        }
                    },
                    value: *value,
                }
            }
            TrustedHalfOperation::ToF32(format) => {
                let [value] = arguments.as_slice() else {
                    return Err(self.call_arity(callee, 1, arguments.len(), location));
                };
                FloatOperation::Convert {
                    kind: match format {
                        fe2o3_kernel_ir::NarrowFloatFormat::F16 => FloatConversionKind::F16ToF32,
                        fe2o3_kernel_ir::NarrowFloatFormat::Bf16 => FloatConversionKind::Bf16ToF32,
                    },
                    value: *value,
                }
            }
            TrustedHalfOperation::WidenedBinary { format, op } => {
                let [lhs, rhs] = arguments.as_slice() else {
                    return Err(self.call_arity(callee, 2, arguments.len(), location));
                };
                FloatOperation::WidenedBinary {
                    format,
                    op,
                    lhs: *lhs,
                    rhs: *rhs,
                }
            }
            TrustedHalfOperation::Bf16x2FusedMultiplyAdd => {
                let [value, multiplier, addend] = arguments.as_slice() else {
                    return Err(self.call_arity(callee, 3, arguments.len(), location));
                };
                FloatOperation::Bf16x2FusedMultiplyAdd {
                    value: *value,
                    multiplier: *multiplier,
                    addend: *addend,
                }
            }
        };
        self.require_float_argument_types(&float, &location)?;
        self.require_destination_type(destination, &float.result_type(), &location)?;

        let declaration = float.declaration();
        self.register_declaration_identity(
            declaration.id.as_str(),
            declaration.signature.clone(),
            &location,
        )?;
        let result = self.fresh_value(float.result_type(), &location)?;
        block.operations.push(float.operation(result.id));
        self.bind_local(
            destination.local,
            LocalBinding::Value(result.id),
            location.clone(),
        )?;
        Ok(Terminator::Branch {
            target: self.block_id(target, location.clone())?,
            arguments: self.edge_arguments(target, &location)?,
        })
    }

    fn require_device_math_receiver(
        &self,
        operand: &MirOperandRef,
        location: &TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        let MirOperandRef::Place(place) = operand else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                "DeviceMath receiver must be an authenticated local capability",
            ));
        };
        if !place.projection.is_empty()
            || !matches!(
                self.locals.get(&place.local),
                Some(LocalBinding::DeviceMathCapability)
            )
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                "DeviceMath receiver did not originate from the authenticated compiler constructor",
            ));
        }
        Ok(())
    }

    fn require_strict_float_policy(
        &self,
        location: &TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        if self.strict_float_policy != StrictFloatPolicy::Canonical {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedCall,
                location.clone(),
                "strict gfx942 half/math lowering rejects custom -Cllvm-args and -Cpasses pipelines",
            ));
        }
        Ok(())
    }

    fn require_matrix_frontend_abi(
        &mut self,
        location: &TranslationLocation,
    ) -> Result<MatrixFrontendBindingV2, TranslationDiagnostic> {
        validate_matrix_frontend_function_abi(self.function)?;
        let evidence = self.function.matrix_frontend_abi.as_ref().ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                "matrix fragment flattening requires a rustc-bound source ABI observation",
            )
        })?;
        evidence.validate().map_err(|reason| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                reason,
            )
        })?;
        let binding = evidence.kernel_ir_binding().map_err(|reason| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                reason,
            )
        })?;
        self.required_capabilities
            .insert(gfx942_xnack_minus_target_capability());
        self.required_capabilities.extend(binding.capabilities());
        Ok(binding)
    }

    fn require_float_argument_types(
        &self,
        float: &FloatOperation,
        location: &TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        let actual = float
            .operands()
            .into_iter()
            .map(|value| self.value_type(value, location).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        let expected = float.parameter_types();
        if actual != expected {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                format!("device math arguments must lower to {expected:?}; found {actual:?}"),
            ));
        }
        Ok(())
    }

    fn require_destination_shape(
        &self,
        destination: &MirPlaceRef,
        expected: &MirTypeShape,
        location: &TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        let actual = self.local_shape(destination.local, location)?;
        if actual != expected {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                format!(
                    "call destination local{} must have imported type {expected:?}; found {actual:?}",
                    destination.local
                ),
            ));
        }
        Ok(())
    }

    fn require_destination_type(
        &self,
        destination: &MirPlaceRef,
        expected: &Type,
        location: &TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        let actual = lower_parameter_type(self.local_shape(destination.local, location)?)
            .ok_or_else(|| {
                diagnostic(
                    TranslationDiagnosticCode::UnsupportedType,
                    location.clone(),
                    format!(
                        "call destination local{} has no supported kernel IR type",
                        destination.local
                    ),
                )
            })?;
        if &actual != expected {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                format!(
                    "call destination local{} must lower to {expected:?}; found {actual:?}",
                    destination.local
                ),
            ));
        }
        Ok(())
    }

    fn register_declaration_identity(
        &mut self,
        identity: &str,
        signature: Signature,
        location: &TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        if let Some(previous) = self.declarations.get(identity)
            && previous != &signature
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                format!(
                    "callee `{}` was imported with inconsistent signatures",
                    identity
                ),
            ));
        }
        self.declarations
            .entry(identity.to_string())
            .or_insert(signature);
        Ok(())
    }

    fn require_call_types(
        &self,
        callee: &MirCallee,
        actual: &[Type],
        expected: &[Type],
        location: TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        if actual.len() != expected.len() {
            return Err(self.call_arity(callee, expected.len(), actual.len(), location));
        }
        if let Some((index, (actual, expected))) = actual
            .iter()
            .zip(expected)
            .enumerate()
            .find(|(_, (actual, expected))| actual != expected)
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location,
                format!(
                    "callee `{}` operand {index} must lower to {expected:?}, found {actual:?}",
                    callee.identity()
                ),
            ));
        }
        Ok(())
    }

    fn call_arity(
        &self,
        callee: &MirCallee,
        expected: usize,
        actual: usize,
        location: TranslationLocation,
    ) -> TranslationDiagnostic {
        diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            location,
            format!(
                "callee `{}` expects {expected} operand(s), found {actual}",
                callee.identity()
            ),
        )
    }

    fn lower_operand(
        &mut self,
        operand: &MirOperandRef,
        block: &mut BasicBlock,
        location: &TranslationLocation,
    ) -> Result<ValueId, TranslationDiagnostic> {
        match operand {
            MirOperandRef::Place(place) => self.lower_place_read(place, block, location),
            MirOperandRef::Constant { literal, .. } => {
                let constant = lower_constant(literal).ok_or_else(|| {
                    diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location.clone(),
                        format!("unsupported or unevaluated constant: {literal:?}"),
                    )
                })?;
                self.emit_result(
                    block,
                    constant.ty(),
                    OperationKind::Constant(constant),
                    location,
                )
            }
        }
    }

    fn lower_place_read(
        &mut self,
        place: &MirPlaceRef,
        block: &mut BasicBlock,
        location: &TranslationLocation,
    ) -> Result<ValueId, TranslationDiagnostic> {
        match place.projection.as_slice() {
            [] => match self.locals.get(&place.local).copied() {
                Some(LocalBinding::Value(value)) => {
                    self.validate_guarded_pointer_use(value, location)?;
                    Ok(value)
                }
                Some(
                    LocalBinding::OptionPointer { .. }
                    | LocalBinding::FieldlessEnum { .. }
                    | LocalBinding::DeviceMathCapability
                    | LocalBinding::Gfx942CollectiveCapability
                    | LocalBinding::Gfx942StaticLdsU32x256(_)
                    | LocalBinding::DeviceMatrixValueCapability
                    | LocalBinding::DeviceMatrixReferenceCapability
                    | LocalBinding::Bf16MfmaFragment(_)
                    | LocalBinding::F32AccumulatorFragment(_),
                ) => Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedType,
                    location.clone(),
                    format!(
                        "local{} is a Rust aggregate, not one kernel IR value",
                        place.local
                    ),
                )),
                None => Err(self.undefined_local(place.local, location.clone())),
            },
            [
                MirProjectionElem::Downcast { variant: 1 },
                MirProjectionElem::Field(0),
            ] => match self.locals.get(&place.local).copied() {
                Some(LocalBinding::OptionPointer {
                    payload,
                    some_entry,
                    ..
                }) => {
                    if self.is_exact_general_v3_alpha_zeta_context() {
                        let Some(some_entry) = some_entry else {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::UnsupportedProjection,
                                location.clone(),
                                "Option payload is used before an authenticated Some-edge guard",
                            ));
                        };
                        let Some(use_block) = location.block else {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::UnsupportedProjection,
                                location.clone(),
                                "Option payload use has no MIR block identity",
                            ));
                        };
                        if !mir_block_dominates(self.function, some_entry, use_block) {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::UnsupportedProjection,
                                location.clone(),
                                "Option payload use is not dominated by the bounds-checked Some edge",
                            ));
                        }
                        self.guarded_pointer_values.insert(payload, some_entry);
                    }
                    Ok(payload)
                }
                Some(
                    LocalBinding::Value(_)
                    | LocalBinding::FieldlessEnum { .. }
                    | LocalBinding::DeviceMathCapability
                    | LocalBinding::Gfx942CollectiveCapability
                    | LocalBinding::Gfx942StaticLdsU32x256(_)
                    | LocalBinding::DeviceMatrixValueCapability
                    | LocalBinding::DeviceMatrixReferenceCapability
                    | LocalBinding::Bf16MfmaFragment(_)
                    | LocalBinding::F32AccumulatorFragment(_),
                ) => Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedType,
                    location.clone(),
                    format!("local{} is not a translated Option pointer", place.local),
                )),
                None => Err(self.undefined_local(place.local, location.clone())),
            },
            [MirProjectionElem::Deref, MirProjectionElem::Index { local }] => {
                let pointer = self.indexed_pointer(place.local, *local, block, location)?;
                let pointee =
                    pointer_pointee(self.value_type(pointer, location)?).ok_or_else(|| {
                        diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location.clone(),
                            "indexed place did not produce a pointer",
                        )
                    })?;
                let alignment = scalar_alignment(&pointee).ok_or_else(|| {
                    diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location.clone(),
                        format!("cannot load unsupported pointee type {pointee:?}"),
                    )
                })?;
                self.emit_result(
                    block,
                    pointee,
                    OperationKind::Load {
                        pointer,
                        access: MemoryAccess::new(AddressSpace::Global, alignment),
                    },
                    location,
                )
            }
            [MirProjectionElem::Deref] => {
                let pointer = self.plain_local(place.local, location)?;
                let pointee =
                    pointer_pointee(self.value_type(pointer, location)?).ok_or_else(|| {
                        diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location.clone(),
                            "deref place base is not a pointer",
                        )
                    })?;
                let alignment = scalar_alignment(&pointee).ok_or_else(|| {
                    diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location.clone(),
                        format!("cannot load unsupported pointee type {pointee:?}"),
                    )
                })?;
                self.emit_result(
                    block,
                    pointee,
                    OperationKind::Load {
                        pointer,
                        access: MemoryAccess::new(AddressSpace::Global, alignment),
                    },
                    location,
                )
            }
            projection => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                format!("unsupported place projection: {projection:?}"),
            )),
        }
    }

    fn assign_value(
        &mut self,
        destination: &MirPlaceRef,
        value: ValueId,
        block: &mut BasicBlock,
        location: TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        if destination.projection.is_empty() {
            return self.bind_local(destination.local, LocalBinding::Value(value), location);
        }
        let pointer = self.place_pointer(destination, block, &location)?;
        let pointee = pointer_pointee(self.value_type(pointer, &location)?).ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                "store destination is not a pointer",
            )
        })?;
        let alignment = scalar_alignment(&pointee).ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                format!("cannot store unsupported pointee type {pointee:?}"),
            )
        })?;
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer,
                value,
                access: MemoryAccess::new(AddressSpace::Global, alignment),
            },
        ));
        Ok(())
    }

    fn bind_plain_destination(
        &mut self,
        destination: &MirPlaceRef,
        value: ValueId,
        location: TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        if !destination.projection.is_empty() {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location,
                "this rvalue requires an unprojected local destination",
            ));
        }
        self.bind_local(destination.local, LocalBinding::Value(value), location)
    }

    fn place_pointer(
        &mut self,
        place: &MirPlaceRef,
        block: &mut BasicBlock,
        location: &TranslationLocation,
    ) -> Result<ValueId, TranslationDiagnostic> {
        match place.projection.as_slice() {
            [MirProjectionElem::Deref] => self.plain_local(place.local, location),
            [MirProjectionElem::Deref, MirProjectionElem::Index { local }] => {
                self.indexed_pointer(place.local, *local, block, location)
            }
            projection => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                format!("unsupported store projection: {projection:?}"),
            )),
        }
    }

    fn indexed_pointer(
        &mut self,
        base_local: usize,
        index_local: usize,
        block: &mut BasicBlock,
        location: &TranslationLocation,
    ) -> Result<ValueId, TranslationDiagnostic> {
        let slice = self.plain_local(base_local, location)?;
        let slice_ty = self.value_type(slice, location)?.clone();
        let Type::Slice(slice_type) = &slice_ty else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                format!("local{base_local} is not a slice"),
            ));
        };
        let pointer_ty = Type::pointer(
            (*slice_type.element).clone(),
            slice_type.address_space,
            slice_type.access,
        );
        let data = self.emit_result(
            block,
            pointer_ty.clone(),
            OperationKind::SliceData { slice },
            location,
        )?;
        let offset = self.plain_local(index_local, location)?;
        self.emit_result(
            block,
            pointer_ty,
            OperationKind::GetElementPointer { base: data, offset },
            location,
        )
    }

    fn emit_result(
        &mut self,
        block: &mut BasicBlock,
        ty: Type,
        kind: OperationKind,
        location: &TranslationLocation,
    ) -> Result<ValueId, TranslationDiagnostic> {
        let definition = self.fresh_value(ty, location)?;
        let id = definition.id;
        block
            .operations
            .push(Operation::effect_free(definition, kind));
        Ok(id)
    }

    fn fresh_value(
        &mut self,
        ty: Type,
        location: &TranslationLocation,
    ) -> Result<ValueDef, TranslationDiagnostic> {
        let id = ValueId(self.next_value);
        self.next_value = self.next_value.checked_add(1).ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                "function has too many SSA values",
            )
        })?;
        self.value_types.insert(id, ty.clone());
        Ok(ValueDef::new(id, ty))
    }

    fn bind_local(
        &mut self,
        local: usize,
        binding: LocalBinding,
        location: TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        let previous = self.locals.insert(local, binding);
        if previous.is_some() && !self.control_flow_ssa.is_promoted(local) {
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location,
                format!("local{local} is assigned more than once in the supported SSA subset"),
            ));
        }
        Ok(())
    }

    fn plain_local(
        &self,
        local: usize,
        location: &TranslationLocation,
    ) -> Result<ValueId, TranslationDiagnostic> {
        match self.locals.get(&local).copied() {
            Some(LocalBinding::Value(value)) => {
                self.validate_guarded_pointer_use(value, location)?;
                Ok(value)
            }
            Some(
                LocalBinding::OptionPointer { .. }
                | LocalBinding::FieldlessEnum { .. }
                | LocalBinding::DeviceMathCapability
                | LocalBinding::Gfx942CollectiveCapability
                | LocalBinding::Gfx942StaticLdsU32x256(_)
                | LocalBinding::DeviceMatrixValueCapability
                | LocalBinding::DeviceMatrixReferenceCapability
                | LocalBinding::Bf16MfmaFragment(_)
                | LocalBinding::F32AccumulatorFragment(_),
            ) => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                format!("local{local} is a Rust aggregate, not one kernel IR value"),
            )),
            None => Err(self.undefined_local(local, location.clone())),
        }
    }

    pub(super) fn edge_arguments(
        &self,
        target: usize,
        location: &TranslationLocation,
    ) -> Result<Vec<ValueId>, TranslationDiagnostic> {
        self.control_flow_ssa
            .live_in(target)
            .iter()
            .map(|local| match self.locals.get(local).copied() {
                Some(LocalBinding::Value(value))
                | Some(LocalBinding::FieldlessEnum {
                    discriminant: value,
                }) => Ok(value),
                Some(
                    LocalBinding::OptionPointer { .. }
                    | LocalBinding::DeviceMathCapability
                    | LocalBinding::Gfx942CollectiveCapability
                    | LocalBinding::Gfx942StaticLdsU32x256(_)
                    | LocalBinding::DeviceMatrixValueCapability
                    | LocalBinding::DeviceMatrixReferenceCapability
                    | LocalBinding::Bf16MfmaFragment(_)
                    | LocalBinding::F32AccumulatorFragment(_),
                ) => Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedType,
                    location.clone(),
                    format!("local{local} is not a promotable scalar control-flow value"),
                )),
                None => Err(self.undefined_local(*local, location.clone())),
            })
            .collect()
    }

    fn validate_guarded_pointer_use(
        &self,
        value: ValueId,
        location: &TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        let Some(some_entry) = self.guarded_pointer_values.get(&value).copied() else {
            return Ok(());
        };
        let Some(use_block) = location.block else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                "guarded Option payload use has no MIR block identity",
            ));
        };
        if !mir_block_dominates(self.function, some_entry, use_block) {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                "Option payload alias escapes the bounds-checked Some region",
            ));
        }
        Ok(())
    }

    fn local_shape(
        &self,
        local: usize,
        location: &TranslationLocation,
    ) -> Result<&MirTypeShape, TranslationDiagnostic> {
        self.imported_local_shape(local).ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                format!("local{local} has no imported type"),
            )
        })
    }

    fn imported_local_shape(&self, local: usize) -> Option<&MirTypeShape> {
        self.function
            .locals
            .iter()
            .find(|candidate| candidate.index == local)
            .map(|candidate| &candidate.ty.shape)
    }

    fn value_type(
        &self,
        value: ValueId,
        location: &TranslationLocation,
    ) -> Result<&Type, TranslationDiagnostic> {
        self.value_types.get(&value).ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                format!("SSA value {value} has no imported type"),
            )
        })
    }

    fn undefined_local(
        &self,
        local: usize,
        location: TranslationLocation,
    ) -> TranslationDiagnostic {
        diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            location,
            format!("local{local} is used before it is defined"),
        )
    }

    fn block_id(
        &self,
        index: usize,
        location: TranslationLocation,
    ) -> Result<BlockId, TranslationDiagnostic> {
        u32::try_from(index).map(BlockId).map_err(|_| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location,
                format!("basic block index {index} exceeds kernel IR limits"),
            )
        })
    }
}

fn declared_function_signature(function: &MirFunction) -> Result<Signature, TranslationDiagnostic> {
    validate_matrix_frontend_function_abi(function)?;
    let mut args = function
        .locals
        .iter()
        .filter(|local| local.role == crate::mir_import::MirLocalRole::Arg)
        .collect::<Vec<_>>();
    args.sort_by_key(|local| local.index);
    if args.len() != function.arg_count {
        return Err(diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            TranslationLocation::function(function),
            format!(
                "function declares {} arguments but imports {} argument locals",
                function.arg_count,
                args.len()
            ),
        ));
    }
    let parameters = args
        .into_iter()
        .map(|arg| {
            lower_function_parameter_types(&arg.ty.shape).ok_or_else(|| {
                diagnostic(
                    TranslationDiagnosticCode::UnsupportedType,
                    TranslationLocation::function(function),
                    format!(
                        "argument local{} has unsupported type `{}`",
                        arg.index, arg.ty.rust
                    ),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();

    let return_local = function
        .locals
        .iter()
        .find(|local| local.index == 0)
        .ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(function),
                "function has no return local0",
            )
        })?;
    if return_local.role != crate::mir_import::MirLocalRole::Return {
        return Err(diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            TranslationLocation::function(function),
            "local0 is not marked as the function return local",
        ));
    }
    let results = match (&function.kind, &return_local.ty.shape) {
        (_, MirTypeShape::Unit) => Vec::new(),
        (MirFunctionKind::InternalHelper | MirFunctionKind::DeviceFfiExport, shape) => {
            lower_scalar_type(shape).map_or_else(
                || {
                    Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        TranslationLocation::function(function),
                        format!(
                            "device definition return type `{}` is not supported",
                            return_local.ty.rust
                        ),
                    ))
                },
                |ty| Ok(vec![ty]),
            )?
        }
        (MirFunctionKind::KernelEntry, _) => {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                TranslationLocation::function(function),
                format!(
                    "kernel entry return type `{}` is not supported",
                    return_local.ty.rust
                ),
            ));
        }
    };
    Ok(Signature::new(parameters, results))
}

fn validate_matrix_frontend_function_abi(
    function: &MirFunction,
) -> Result<(), TranslationDiagnostic> {
    let mut args = function
        .locals
        .iter()
        .filter(|local| local.role == crate::mir_import::MirLocalRole::Arg)
        .collect::<Vec<_>>();
    args.sort_by_key(|local| local.index);
    let has_fragment = args
        .iter()
        .any(|argument| is_matrix_fragment_shape(&argument.ty.shape));
    if !has_fragment && function.matrix_frontend_abi.is_none() {
        return Ok(());
    }
    let Some(evidence) = &function.matrix_frontend_abi else {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            TranslationLocation::function(function),
            "matrix fragment flattening requires a rustc-bound source ABI observation",
        ));
    };
    evidence.validate().map_err(|reason| {
        diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            TranslationLocation::function(function),
            reason,
        )
    })?;
    let expected = [
        TrustedDeviceItem::Bf16MfmaFragment,
        TrustedDeviceItem::Bf16MfmaFragment,
        TrustedDeviceItem::F32AccumulatorFragment,
    ];
    if function.kind != MirFunctionKind::KernelEntry
        || args.len() != expected.len()
        || !args
            .iter()
            .zip(expected)
            .all(|(argument, item)| is_trusted_adt_shape(&argument.ty.shape, item))
    {
        return Err(diagnostic(
            TranslationDiagnosticCode::UnsupportedType,
            TranslationLocation::function(function),
            "matrix frontend ABI requires a kernel entry with exact (Bf16MfmaFragment, Bf16MfmaFragment, F32AccumulatorFragment) parameters",
        ));
    }
    Ok(())
}

fn lower_parameter_type(shape: &MirTypeShape) -> Option<Type> {
    if let Some(scalar) = lower_scalar_type(shape) {
        return Some(scalar);
    }
    match shape {
        MirTypeShape::Slice { element, mutable } => Some(Type::slice(
            lower_element_type(element)?,
            AddressSpace::Global,
            if *mutable {
                AccessMode::ReadWrite
            } else {
                AccessMode::ReadOnly
            },
        )),
        MirTypeShape::DisjointSlice { element } => Some(Type::slice(
            lower_element_type(element)?,
            AddressSpace::Global,
            AccessMode::ReadWrite,
        )),
        _ => None,
    }
}

fn lower_function_parameter_types(shape: &MirTypeShape) -> Option<Vec<Type>> {
    if is_trusted_adt_shape(shape, TrustedDeviceItem::Bf16MfmaFragment) {
        return Some(vec![Type::Scalar(ScalarType::Bf16); 4]);
    }
    if is_trusted_adt_shape(shape, TrustedDeviceItem::F32AccumulatorFragment) {
        return Some(vec![Type::F32; 4]);
    }
    Some(vec![lower_parameter_type(shape)?])
}

fn is_matrix_fragment_shape(shape: &MirTypeShape) -> bool {
    is_trusted_adt_shape(shape, TrustedDeviceItem::Bf16MfmaFragment)
        || is_trusted_adt_shape(shape, TrustedDeviceItem::F32AccumulatorFragment)
}

fn is_trusted_adt_shape(shape: &MirTypeShape, item: TrustedDeviceItem) -> bool {
    matches!(
        shape,
        MirTypeShape::Adt { identity } if identity == item.canonical_path()
    )
}

fn is_readonly_f32_slice(shape: &MirTypeShape) -> bool {
    matches!(
        shape,
        MirTypeShape::Slice {
            element,
            mutable: false,
        } if element.as_ref() == &MirTypeShape::F32
    )
}

fn mir_block_dominates(function: &MirFunction, dominator: usize, dominated: usize) -> bool {
    let blocks = function
        .blocks
        .iter()
        .map(|block| block.index)
        .collect::<BTreeSet<_>>();
    if !blocks.contains(&dominator) || !blocks.contains(&dominated) {
        return false;
    }
    let Some(entry) = blocks.first().copied() else {
        return false;
    };
    let mut predecessors = blocks
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &function.blocks {
        let Some(terminator) = block.terminator.as_ref() else {
            return false;
        };
        for successor in mir_successors(&terminator.kind) {
            let Some(incoming) = predecessors.get_mut(&successor) else {
                return false;
            };
            incoming.insert(block.index);
        }
    }

    let mut dominators = blocks
        .iter()
        .copied()
        .map(|block| {
            let initial = if block == entry {
                BTreeSet::from([entry])
            } else {
                blocks.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let previous = dominators.clone();
        let mut changed = false;
        for block in blocks.iter().copied().filter(|block| *block != entry) {
            let incoming = &predecessors[&block];
            let mut next = if let Some(first) = incoming.first() {
                previous[first].clone()
            } else {
                BTreeSet::new()
            };
            for predecessor in incoming.iter().skip(1) {
                next = next.intersection(&previous[predecessor]).copied().collect();
            }
            next.insert(block);
            if next != previous[&block] {
                dominators.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    dominators[&dominated].contains(&dominator)
}

fn mir_successors(terminator: &MirTerminatorKind) -> Vec<usize> {
    match terminator {
        MirTerminatorKind::Goto { target }
        | MirTerminatorKind::Assert { target, .. }
        | MirTerminatorKind::Drop { target } => vec![*target],
        MirTerminatorKind::SwitchInt {
            targets, otherwise, ..
        } => targets
            .iter()
            .map(|target| target.target)
            .chain(std::iter::once(*otherwise))
            .collect(),
        MirTerminatorKind::Call { target, .. } => target.iter().copied().collect(),
        MirTerminatorKind::Return | MirTerminatorKind::Unreachable | MirTerminatorKind::Other => {
            Vec::new()
        }
    }
}

fn is_disjoint_f32_slice(shape: &MirTypeShape) -> bool {
    matches!(
        shape,
        MirTypeShape::DisjointSlice { element } if element.as_ref() == &MirTypeShape::F32
    )
}

fn lower_scalar_type(shape: &MirTypeShape) -> Option<Type> {
    match shape {
        MirTypeShape::Bool => Some(Type::BOOL),
        MirTypeShape::I32 => Some(Type::Scalar(ScalarType::I32)),
        MirTypeShape::U32 | MirTypeShape::Bf16x2 => Some(Type::Scalar(ScalarType::U32)),
        MirTypeShape::I64 | MirTypeShape::ISize => Some(Type::Scalar(ScalarType::I64)),
        MirTypeShape::USize => Some(Type::INDEX),
        MirTypeShape::F16 => Some(Type::Scalar(ScalarType::F16)),
        MirTypeShape::Bf16 => Some(Type::Scalar(ScalarType::Bf16)),
        MirTypeShape::F32 => Some(Type::F32),
        MirTypeShape::F64 => Some(Type::F64),
        _ => None,
    }
}

fn lower_device_ffi_signature(
    physical_abi: &reserved_fe2o3_symbols::DeviceFfiPhysicalAbiV1,
) -> Signature {
    let parameters = physical_abi
        .arguments()
        .iter()
        .copied()
        .map(lower_device_ffi_type)
        .collect();
    let results = match physical_abi.result() {
        DeviceFfiPhysicalResultV1::Unit => Vec::new(),
        DeviceFfiPhysicalResultV1::Value(result) => vec![lower_device_ffi_type(result)],
    };
    Signature::new(parameters, results)
}

fn lower_device_ffi_type(physical: DeviceFfiPhysicalTypeV1) -> Type {
    match physical {
        DeviceFfiPhysicalTypeV1::Scalar(scalar) => Type::Scalar(match scalar {
            DeviceFfiScalarTypeV1::I8 => ScalarType::I8,
            DeviceFfiScalarTypeV1::U8 => ScalarType::U8,
            DeviceFfiScalarTypeV1::I16 => ScalarType::I16,
            DeviceFfiScalarTypeV1::U16 => ScalarType::U16,
            DeviceFfiScalarTypeV1::I32 => ScalarType::I32,
            DeviceFfiScalarTypeV1::U32 => ScalarType::U32,
            DeviceFfiScalarTypeV1::I64 => ScalarType::I64,
            DeviceFfiScalarTypeV1::U64 => ScalarType::U64,
            DeviceFfiScalarTypeV1::F32 => ScalarType::F32,
            DeviceFfiScalarTypeV1::F64 => ScalarType::F64,
        }),
        DeviceFfiPhysicalTypeV1::Pointer(pointer) => Type::pointer(
            lower_device_ffi_type(DeviceFfiPhysicalTypeV1::Scalar(pointer.element())),
            match pointer.address_space() {
                DeviceFfiAddressSpaceV1::Constant => AddressSpace::Constant,
                DeviceFfiAddressSpaceV1::Global => AddressSpace::Global,
                DeviceFfiAddressSpaceV1::Private => AddressSpace::Private,
                DeviceFfiAddressSpaceV1::Workgroup => AddressSpace::Workgroup,
            },
            match pointer.access() {
                DeviceFfiPointerAccessV1::Const => AccessMode::ReadOnly,
                DeviceFfiPointerAccessV1::Mut => AccessMode::ReadWrite,
            },
        ),
    }
}

fn lower_element_type(shape: &MirTypeShape) -> Option<Type> {
    match shape {
        MirTypeShape::F32 => Some(Type::F32),
        MirTypeShape::F64 => Some(Type::F64),
        _ => None,
    }
}

fn lower_constant(constant: &MirConstant) -> Option<Constant> {
    match constant {
        MirConstant::Bool(value) => Some(Constant::Bool(*value)),
        MirConstant::I32(value) => Some(Constant::I32(*value)),
        MirConstant::U32(value) => Some(Constant::U32(*value)),
        MirConstant::I64(value) | MirConstant::ISize(value) => Some(Constant::I64(*value)),
        MirConstant::U64(value) => Some(Constant::U64(*value)),
        MirConstant::USize(value) => Some(Constant::Index(*value)),
        MirConstant::F32Bits(value) => Some(Constant::F32Bits(*value)),
        MirConstant::F64Bits(value) => Some(Constant::F64Bits(*value)),
        MirConstant::ZeroSized
        | MirConstant::StructuredValue(_)
        | MirConstant::ImportFailed(_)
        | MirConstant::Unevaluated => None,
    }
}

fn pointer_pointee(ty: &Type) -> Option<Type> {
    let Type::Pointer(pointer) = ty else {
        return None;
    };
    Some((*pointer.pointee).clone())
}

fn scalar_alignment(ty: &Type) -> Option<u32> {
    match ty {
        Type::Scalar(ScalarType::Bool | ScalarType::I8 | ScalarType::U8) => Some(1),
        Type::Scalar(ScalarType::I16 | ScalarType::U16 | ScalarType::F16 | ScalarType::Bf16) => {
            Some(2)
        }
        Type::Scalar(ScalarType::I32 | ScalarType::U32 | ScalarType::F32) => Some(4),
        Type::Scalar(ScalarType::I64 | ScalarType::U64 | ScalarType::F64 | ScalarType::Index) => {
            Some(8)
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "kernel_ir_lowering_vecadd_tests.rs"]
mod vecadd_tests;

#[cfg(test)]
#[path = "kernel_ir_lowering_general_v3_tests.rs"]
mod general_v3_tests;

#[cfg(test)]
#[path = "kernel_ir_lowering_control_flow_tests.rs"]
mod control_flow_tests;

#[cfg(test)]
#[path = "kernel_ir_lowering_memory_tests.rs"]
mod memory_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir_import::{
        MirImportedType, MirLocal, MirLocalRole, MirPlaceRef, MirProjectionElem,
    };
    use dialect_mir::MirType;
    use fe2o3_rustc_front::{
        FrontendLaunchBoundsV1, FrontendWorkgroupDimensionsV1, KernelFrontendContractV1,
    };

    #[test]
    fn empty_kernels_are_sorted_and_verify() {
        let mut alpha = scalar_fixture().functions.remove(0);
        alpha.export_name = "alpha".to_string();
        alpha.rust_path = "tests::alpha".to_string();
        alpha.blocks.truncate(1);
        alpha.blocks[0].statements.clear();
        alpha.blocks[0].terminator = Some(terminator(MirTerminatorKind::Return));
        let mut zeta = alpha.clone();
        zeta.export_name = "zeta".to_string();
        zeta.rust_path = "tests::zeta".to_string();

        let module = translate_and_verify(&MirModule {
            functions: vec![zeta, alpha],
        })
        .expect("empty kernels");

        assert_eq!(module.kernels[0].id.as_str(), "alpha");
        assert_eq!(module.kernels[1].id.as_str(), "zeta");
    }

    #[test]
    fn exact_authenticated_launch_contract_enters_kernel_ir() {
        let mut kernel =
            empty_kernel_with_contract(launch_contract(Some([128, 1, 1]), Some([128, 1, 1]), None));
        kernel.export_name = "launch_exact".to_owned();
        kernel.rust_path = "tests::launch_exact".to_owned();

        let module = translate_and_verify(&MirModule {
            functions: vec![kernel],
        })
        .expect("exact launch contract");
        assert_eq!(
            module.kernels[0].workgroup_size,
            Some(WorkgroupSize::new(128, 1, 1))
        );
    }

    #[test]
    fn unrepresentable_authenticated_launch_fields_fail_closed() {
        for (contract, expected) in [
            (
                launch_contract(None, Some([128, 1, 1]), None),
                "maximum-only launch bounds",
            ),
            (
                launch_contract(Some([64, 1, 1]), Some([128, 1, 1]), None),
                "non-exact maximum launch bounds",
            ),
            (
                launch_contract(Some([64, 1, 1]), Some([64, 1, 1]), Some(2)),
                "minimum-workgroup occupancy",
            ),
        ] {
            let error = translate_and_verify(&MirModule {
                functions: vec![empty_kernel_with_contract(contract)],
            })
            .unwrap_err();
            assert!(
                error.to_string().contains(expected),
                "missing `{expected}` in {error}"
            );
        }
    }

    #[test]
    fn explicit_device_roles_and_u32_returns_are_preserved() {
        let helper = u32_definition(
            "fe2o3_kernel_looking_helper",
            MirFunctionKind::InternalHelper,
            false,
        );
        let export = u32_definition("device_add", MirFunctionKind::DeviceFfiExport, true);

        let module = translate_and_verify(&MirModule {
            functions: vec![export, helper],
        })
        .expect("u32 device definitions");

        assert!(module.kernels.is_empty());
        let helper = module
            .functions
            .iter()
            .find(|function| function.id.as_str() == "fe2o3_kernel_looking_helper")
            .expect("helper definition");
        assert_eq!(helper.role, fe2o3_kernel_ir::FunctionRole::InternalHelper);
        assert_eq!(
            helper.signature,
            Signature::new(
                vec![Type::Scalar(ScalarType::U32)],
                vec![Type::Scalar(ScalarType::U32)]
            )
        );
        assert!(matches!(
            helper.body.as_ref().expect("helper body").blocks[0].terminator,
            Some(Terminator::Return { ref values }) if values.len() == 1
        ));

        let export = module
            .functions
            .iter()
            .find(|function| function.id.as_str() == "device_add")
            .expect("device export definition");
        assert_eq!(export.role, fe2o3_kernel_ir::FunctionRole::DeviceFfiExport);
        assert_eq!(
            export.signature,
            Signature::new(
                vec![Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
                vec![Type::Scalar(ScalarType::U32)]
            )
        );
        assert!(
            export.body.as_ref().expect("export body").blocks[0]
                .operations
                .iter()
                .any(|operation| matches!(
                    operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                ))
        );
    }

    #[test]
    fn kernel_entries_cannot_return_u32() {
        let function = u32_definition("not_a_device_return", MirFunctionKind::KernelEntry, false);

        let errors = translate_and_verify(&MirModule {
            functions: vec![function],
        })
        .expect_err("kernel result must be rejected");

        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedType));
        assert!(
            errors
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("kernel entry return type") })
        );
    }

    #[test]
    fn device_definition_rejects_non_value_return() {
        let mut function = u32_definition("bad_export", MirFunctionKind::DeviceFfiExport, false);
        function.locals[0] = local(0, MirLocalRole::Return, MirTypeShape::DeviceMath);

        let errors = translate_and_verify(&MirModule {
            functions: vec![function],
        })
        .expect_err("capability result must be rejected");

        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedType));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic.message.contains("return type")
                && diagnostic.message.contains("is not supported")
        }));
    }

    #[test]
    fn authenticated_device_math_becomes_canonical_gfx942_float_ir() {
        let fixture = device_math_fixture(
            DeviceMathDiagnosticItem::F32(fe2o3_kernel_ir::F32MathFunction::Sqrt),
            vec![f32_constant(4.0)],
            MirTypeShape::F32,
            true,
        );
        let module = translate_and_verify_for_target(&fixture, &AmdGpuTarget::new("gfx942"))
            .expect("strict gfx942 math must lower");

        assert!(
            module
                .functions
                .iter()
                .any(|function| function.id.as_str() == "__fe2o3_ir_float_v1_sqrt_f32")
        );
        let body = module
            .functions
            .iter()
            .find(|function| function.role == fe2o3_kernel_ir::FunctionRole::KernelEntry)
            .and_then(|function| function.body.as_ref())
            .expect("kernel body");
        assert!(
            body.blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    &operation.kind,
                    OperationKind::Call { callee, .. }
                        if callee.as_str() == "__fe2o3_ir_float_v1_sqrt_f32"
                ))
        );
        assert!(module.required_capabilities.is_empty());
        verify_module(&module).expect("canonical math module");
    }

    #[test]
    fn device_math_requires_gfx942_constructor_provenance_and_exact_types() {
        let strict = device_math_fixture(
            DeviceMathDiagnosticItem::F32(fe2o3_kernel_ir::F32MathFunction::Sqrt),
            vec![f32_constant(4.0)],
            MirTypeShape::F32,
            true,
        );
        let wrong_target =
            translate_and_verify_for_target(&strict, &AmdGpuTarget::new("gfx1100")).unwrap_err();
        assert!(wrong_target.to_string().contains("exact gfx942"));

        let unproven = device_math_fixture(
            DeviceMathDiagnosticItem::F32(fe2o3_kernel_ir::F32MathFunction::Sqrt),
            vec![f32_constant(4.0)],
            MirTypeShape::F32,
            false,
        );
        let unproven =
            translate_and_verify_for_target(&unproven, &AmdGpuTarget::new("gfx942")).unwrap_err();
        assert!(unproven.to_string().contains("did not originate"));

        let wrong_type = device_math_fixture(
            DeviceMathDiagnosticItem::F32(fe2o3_kernel_ir::F32MathFunction::Sqrt),
            vec![MirOperandRef::Constant {
                ty: MirImportedType {
                    kind: MirType::I32,
                    rust: "u32".to_string(),
                    shape: MirTypeShape::U32,
                    semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
                },
                literal: MirConstant::U32(4),
                value: "4_u32".to_string(),
            }],
            MirTypeShape::F32,
            true,
        );
        let wrong_type =
            translate_and_verify_for_target(&wrong_type, &AmdGpuTarget::new("gfx942")).unwrap_err();
        assert!(
            wrong_type
                .to_string()
                .contains("must lower to [Scalar(F32)]")
        );
    }

    #[test]
    fn authenticated_half_forms_become_canonical_float_calls() {
        use fe2o3_kernel_ir::{NarrowFloatFormat, TargetCapability, WidenedFloatBinaryOp};

        let cases = [
            (
                TrustedHalfOperation::FromF32(NarrowFloatFormat::F16),
                vec![MirTypeShape::F32],
                MirTypeShape::F16,
                "__fe2o3_ir_float_v1_f32_to_f16_rne",
                TargetCapability::Float16,
            ),
            (
                TrustedHalfOperation::ToF32(NarrowFloatFormat::Bf16),
                vec![MirTypeShape::Bf16],
                MirTypeShape::F32,
                "__fe2o3_ir_float_v1_bf16_to_f32",
                TargetCapability::BFloat16,
            ),
            (
                TrustedHalfOperation::WidenedBinary {
                    format: NarrowFloatFormat::F16,
                    op: WidenedFloatBinaryOp::Divide,
                },
                vec![MirTypeShape::F16, MirTypeShape::F16],
                MirTypeShape::F16,
                "__fe2o3_ir_float_v1_f16_div_widened_rne",
                TargetCapability::Float16,
            ),
            (
                TrustedHalfOperation::Bf16x2FusedMultiplyAdd,
                vec![
                    MirTypeShape::Bf16x2,
                    MirTypeShape::Bf16x2,
                    MirTypeShape::Bf16x2,
                ],
                MirTypeShape::Bf16x2,
                "__fe2o3_ir_float_v1_fma_bf16x2",
                TargetCapability::BFloat16,
            ),
        ];

        for (operation, arguments, result, intrinsic, capability) in cases {
            let fixture = half_operation_fixture(operation, &arguments, result);
            let module = translate_and_verify_for_target(&fixture, &AmdGpuTarget::new("gfx942"))
                .expect("authenticated half form");
            assert!(module.required_capabilities.contains(&capability));
            assert!(module.functions.iter().any(|function| {
                function.id.as_str() == intrinsic
                    && function.role == fe2o3_kernel_ir::FunctionRole::ExternalImport
            }));
            verify_module(&module).expect("canonical half module");
        }
    }

    #[test]
    fn half_forms_reject_wrong_target_arity_and_type() {
        use fe2o3_kernel_ir::NarrowFloatFormat;

        let operation = TrustedHalfOperation::FromF32(NarrowFloatFormat::F16);
        let fixture = half_operation_fixture(operation, &[MirTypeShape::F32], MirTypeShape::F16);
        assert!(
            translate_and_verify_for_target(&fixture, &AmdGpuTarget::new("gfx1100"))
                .unwrap_err()
                .to_string()
                .contains("exact gfx942")
        );

        let wrong_arity = half_operation_fixture(operation, &[], MirTypeShape::F16);
        assert!(
            translate_and_verify_for_target(&wrong_arity, &AmdGpuTarget::new("gfx942"))
                .unwrap_err()
                .to_string()
                .contains("expects 1 operand(s), found 0")
        );

        let wrong_type = half_operation_fixture(operation, &[MirTypeShape::U32], MirTypeShape::F16);
        assert!(
            translate_and_verify_for_target(&wrong_type, &AmdGpuTarget::new("gfx942"))
                .unwrap_err()
                .to_string()
                .contains("must lower to [Scalar(F32)]")
        );

        let custom_pipeline = translate_and_verify_for_target_with_policy(
            &fixture,
            &AmdGpuTarget::new("gfx942"),
            StrictFloatPolicy::CustomLlvmPipeline,
        )
        .unwrap_err();
        assert!(
            custom_pipeline
                .to_string()
                .contains("rejects custom -Cllvm-args and -Cpasses")
        );
    }

    #[test]
    fn u32_return_requires_an_initialized_return_local() {
        let mut function = u32_definition(
            "missing_return_value",
            MirFunctionKind::DeviceFfiExport,
            false,
        );
        function.blocks[0].statements.clear();

        let errors = translate_and_verify(&MirModule {
            functions: vec![function],
        })
        .expect_err("uninitialized return local must fail");

        assert!(errors.contains(TranslationDiagnosticCode::MalformedMir));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("local0 is used before it is defined")
        }));
    }

    #[test]
    fn constant_kernel_has_typed_operation() {
        let mut fixture = scalar_fixture();
        fixture.functions[0].blocks.truncate(1);
        fixture.functions[0].blocks[0].statements = vec![assign(
            0,
            3,
            vec![MirOperandRef::Constant {
                ty: MirImportedType {
                    kind: MirType::I32,
                    rust: "i32".to_string(),
                    shape: MirTypeShape::I32,
                    semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
                },
                literal: MirConstant::I32(7),
                value: "7_i32".to_string(),
            }],
            MirRvalueKind::Use,
        )];
        fixture.functions[0].blocks[0].terminator = Some(terminator(MirTerminatorKind::Return));

        let module = translate_and_verify(&fixture).expect("constant kernel");
        assert!(matches!(
            module.functions[0].body.as_ref().expect("body").blocks[0].operations[0].kind,
            OperationKind::Constant(Constant::I32(7))
        ));
    }

    #[test]
    fn scalar_framework_builds_and_verifies_typed_control_flow() {
        let module = translate_and_verify(&scalar_fixture()).expect("scalar fixture");
        verify_module(&module).expect("framework output should verify");

        assert_eq!(module.kernels.len(), 1);
        let body = module.functions[0].body.as_ref().expect("body");
        assert_eq!(body.blocks.len(), 3, "two MIR blocks plus assert trap");
        assert!(body.blocks[0].operations.iter().any(|operation| matches!(
            operation.kind,
            OperationKind::Binary {
                op: BinaryOp::Add,
                ..
            }
        )));
        assert!(matches!(
            body.blocks[0].terminator,
            Some(Terminator::ConditionalBranch { .. })
        ));
    }

    #[test]
    fn scalar_translation_is_deterministic() {
        let fixture = scalar_fixture();
        assert_eq!(
            translate_and_verify(&fixture).expect("first"),
            translate_and_verify(&fixture).expect("second")
        );
    }

    #[test]
    fn slice_metadata_and_indexed_memory_verify() {
        let module = translate_and_verify(&memory_fixture()).expect("memory fixture");
        let operations = &module.functions[0].body.as_ref().expect("body").blocks[0].operations;

        let expected: [fn(&OperationKind) -> bool; 3] = [
            |kind: &OperationKind| matches!(kind, OperationKind::SliceLength { .. }),
            |kind: &OperationKind| matches!(kind, OperationKind::Load { .. }),
            |kind: &OperationKind| matches!(kind, OperationKind::Store { .. }),
        ];
        for expected in expected {
            assert!(operations.iter().any(|operation| expected(&operation.kind)));
        }
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::SliceData { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn every_callable_trusted_helper_has_an_exact_typed_signature() {
        let writable_slice = Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
        let option_pointer_results = vec![
            Type::INDEX,
            Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
        ];
        let cases = vec![
            (
                TrustedDeviceItem::ThreadIndex1d,
                vec![],
                Signature::new(vec![], vec![Type::INDEX]),
            ),
            (
                TrustedDeviceItem::ThreadIndexGet,
                vec![MirTypeShape::USize],
                Signature::new(vec![Type::INDEX], vec![Type::INDEX]),
            ),
            (
                TrustedDeviceItem::ThreadIndexOffset,
                vec![MirTypeShape::USize, MirTypeShape::USize],
                Signature::new(vec![Type::INDEX, Type::INDEX], vec![Type::INDEX]),
            ),
            (
                TrustedDeviceItem::ThreadIndexOffsetSigned,
                vec![MirTypeShape::USize, MirTypeShape::ISize],
                Signature::new(
                    vec![Type::INDEX, Type::Scalar(ScalarType::I64)],
                    vec![Type::INDEX],
                ),
            ),
            (
                TrustedDeviceItem::ThreadIndexStride,
                vec![MirTypeShape::USize, MirTypeShape::USize],
                Signature::new(vec![Type::INDEX, Type::INDEX], vec![Type::INDEX]),
            ),
            (
                TrustedDeviceItem::ThreadIndexStrideOffset,
                vec![
                    MirTypeShape::USize,
                    MirTypeShape::USize,
                    MirTypeShape::ISize,
                ],
                Signature::new(
                    vec![Type::INDEX, Type::INDEX, Type::Scalar(ScalarType::I64)],
                    vec![Type::INDEX],
                ),
            ),
            (
                TrustedDeviceItem::DisjointSliceGetMut,
                vec![
                    MirTypeShape::DisjointSlice {
                        element: Box::new(MirTypeShape::F32),
                    },
                    MirTypeShape::USize,
                ],
                Signature::new(
                    vec![writable_slice.clone(), Type::INDEX],
                    option_pointer_results.clone(),
                ),
            ),
            (
                TrustedDeviceItem::DisjointSliceGetMutAt,
                vec![
                    MirTypeShape::DisjointSlice {
                        element: Box::new(MirTypeShape::F32),
                    },
                    MirTypeShape::USize,
                ],
                Signature::new(vec![writable_slice, Type::INDEX], option_pointer_results),
            ),
        ];

        for (item, argument_shapes, expected) in cases {
            let fixture = helper_call_fixture(MirCallee::trusted_for_test(item), &argument_shapes);
            let module = translate_and_verify(&fixture).expect("trusted helper should lower");
            let declaration = module
                .functions
                .iter()
                .find(|function| function.id.as_str() == item.canonical_path())
                .expect("trusted helper declaration");

            assert!(declaration.body.is_none(), "{item:?}");
            assert_eq!(declaration.signature, expected, "{item:?}");
        }
    }

    #[test]
    fn thread_index_get_accepts_the_shared_receiver_mir_form() {
        let index_shape = MirTypeShape::Adt {
            identity: TrustedDeviceItem::ThreadIndex.canonical_path().to_string(),
        };
        let module = MirModule {
            functions: vec![MirFunction {
                semantic_instance: None,
                export_name: "borrowed_index".to_string(),
                rust_path: "tests::borrowed_index".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                matrix_frontend_abi: None,
                arg_count: 0,
                local_count: 4,
                locals: vec![
                    local(0, MirLocalRole::Return, MirTypeShape::Unit),
                    local(1, MirLocalRole::Temp, index_shape.clone()),
                    local(
                        2,
                        MirLocalRole::Temp,
                        MirTypeShape::Reference {
                            pointee: Box::new(index_shape),
                            mutable: false,
                        },
                    ),
                    local(3, MirLocalRole::Temp, MirTypeShape::USize),
                ],
                blocks: vec![
                    MirBlock {
                        index: 0,
                        statements: Vec::new(),
                        terminator: Some(terminator(MirTerminatorKind::Call {
                            callee: Some(MirCallee::trusted_for_test(
                                TrustedDeviceItem::ThreadIndex1d,
                            )),
                            target: Some(1),
                            destination: Some(place(1)),
                            operands: Vec::new(),
                        })),
                    },
                    MirBlock {
                        index: 1,
                        statements: vec![assign(
                            0,
                            2,
                            vec![operand(1)],
                            MirRvalueKind::Reference(crate::mir_import::MirBorrowKind::Shared),
                        )],
                        terminator: Some(terminator(MirTerminatorKind::Call {
                            callee: Some(MirCallee::trusted_for_test(
                                TrustedDeviceItem::ThreadIndexGet,
                            )),
                            target: Some(2),
                            destination: Some(place(3)),
                            operands: vec![operand(2)],
                        })),
                    },
                    MirBlock {
                        index: 2,
                        statements: Vec::new(),
                        terminator: Some(terminator(MirTerminatorKind::Return)),
                    },
                ],
            }],
        };

        let module = translate_and_verify(&module).expect("borrowed receiver should lower");
        let get = module
            .functions
            .iter()
            .find(|function| {
                function.id.as_str() == TrustedDeviceItem::ThreadIndexGet.canonical_path()
            })
            .expect("thread-index get declaration");
        assert_eq!(
            get.signature,
            Signature::new(vec![Type::INDEX], vec![Type::INDEX])
        );
    }

    #[test]
    fn thread_index_get_accepts_the_default_mutable_receiver_mir_form() {
        let fixture = borrowed_index_fixture(
            MirRvalueKind::Reference(crate::mir_import::MirBorrowKind::MutableDefault),
            true,
        );
        translate_and_verify(&fixture).expect("default mutable receiver should lower");
    }

    #[test]
    fn thread_index_get_accepts_the_two_phase_mutable_receiver_mir_form() {
        let fixture = borrowed_index_fixture(
            MirRvalueKind::Reference(crate::mir_import::MirBorrowKind::MutableTwoPhase),
            true,
        );
        translate_and_verify(&fixture).expect("two-phase mutable receiver should lower");
    }

    #[test]
    fn lowering_rejects_legacy_reference_and_retag_forms() {
        let legacy_reference = borrowed_index_fixture(MirRvalueKind::Ref, false);
        let error = translate_and_verify(&legacy_reference).unwrap_err();
        assert!(error.contains(TranslationDiagnosticCode::UnsupportedRvalue));
        assert!(error.to_string().contains("payload-free reference"));

        let mut legacy_retag = borrowed_index_fixture(
            MirRvalueKind::Reference(crate::mir_import::MirBorrowKind::Shared),
            false,
        );
        legacy_retag.functions[0].blocks[1].statements.insert(
            0,
            MirStatement {
                index: 0,
                kind: MirStatementKind::Retag,
                destination: None,
                operands: Vec::new(),
                rvalue: None,
                semantic_rvalue_type: None,
                operation: None,
                source: Some(source()),
            },
        );
        let error = translate_and_verify(&legacy_retag).unwrap_err();
        assert!(error.contains(TranslationDiagnosticCode::UnsupportedStatement));
        assert!(error.to_string().contains("payload-free retag"));
    }

    #[test]
    fn lowering_rejects_reference_alias_kinds_not_preserved_by_kernel_ir() {
        for kind in [
            crate::mir_import::MirBorrowKind::FakeDeep,
            crate::mir_import::MirBorrowKind::FakeShallow,
            crate::mir_import::MirBorrowKind::MutableClosureCapture,
        ] {
            let fixture = borrowed_index_fixture(MirRvalueKind::Reference(kind), true);
            let error = translate_and_verify(&fixture).unwrap_err();
            assert!(
                error.contains(TranslationDiagnosticCode::UnsupportedRvalue),
                "{kind:?}: {error}"
            );
            assert!(
                error
                    .to_string()
                    .contains("does not preserve its alias semantics")
            );
        }
    }

    fn borrowed_index_fixture(rvalue: MirRvalueKind, mutable: bool) -> MirModule {
        let index_shape = MirTypeShape::Adt {
            identity: TrustedDeviceItem::ThreadIndex.canonical_path().to_string(),
        };
        MirModule {
            functions: vec![MirFunction {
                semantic_instance: None,
                export_name: "borrowed_index_fixture".to_string(),
                rust_path: "tests::borrowed_index_fixture".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                matrix_frontend_abi: None,
                arg_count: 0,
                local_count: 4,
                locals: vec![
                    local(0, MirLocalRole::Return, MirTypeShape::Unit),
                    local(1, MirLocalRole::Temp, index_shape.clone()),
                    local(
                        2,
                        MirLocalRole::Temp,
                        MirTypeShape::Reference {
                            pointee: Box::new(index_shape),
                            mutable,
                        },
                    ),
                    local(3, MirLocalRole::Temp, MirTypeShape::USize),
                ],
                blocks: vec![
                    MirBlock {
                        index: 0,
                        statements: Vec::new(),
                        terminator: Some(terminator(MirTerminatorKind::Call {
                            callee: Some(MirCallee::trusted_for_test(
                                TrustedDeviceItem::ThreadIndex1d,
                            )),
                            target: Some(1),
                            destination: Some(place(1)),
                            operands: Vec::new(),
                        })),
                    },
                    MirBlock {
                        index: 1,
                        statements: vec![assign(0, 2, vec![operand(1)], rvalue)],
                        terminator: Some(terminator(MirTerminatorKind::Call {
                            callee: Some(MirCallee::trusted_for_test(
                                TrustedDeviceItem::ThreadIndexGet,
                            )),
                            target: Some(2),
                            destination: Some(place(3)),
                            operands: vec![operand(2)],
                        })),
                    },
                    MirBlock {
                        index: 2,
                        statements: Vec::new(),
                        terminator: Some(terminator(MirTerminatorKind::Return)),
                    },
                ],
            }],
        }
    }

    #[test]
    fn marker_free_canonical_spelling_is_not_session_recognized() {
        let identity = TrustedDeviceItem::ThreadIndex1d.canonical_path();
        let fixture = helper_call_fixture(MirCallee::untrusted_for_test(identity), &[]);

        let errors = translate_and_verify(&fixture).expect_err("untrusted spelling must fail");

        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedCall));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("has no classified trusted device identity")
        }));
    }

    #[test]
    fn session_recognized_call_rejects_unsupported_context_without_fallback() {
        let mut fixture = helper_call_fixture(
            MirCallee::trusted_for_test(TrustedDeviceItem::ThreadIndex1d),
            &[],
        );
        fixture.functions[0].typed_profile =
            Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3);

        let errors = translate_and_verify(&fixture)
            .expect_err("recognized General V3 call outside its context must fail closed");

        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedCall));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic.message.contains(
                "session-recognized semantic call `fe2o3_device::thread::index_1d` requires an exact General V3 alpha/zeta kernel context",
            )
        }));
        assert!(errors.diagnostics().iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("has no classified trusted device identity")
        }));
    }

    #[test]
    fn collected_internal_helper_call_uses_its_export_symbol() {
        let mut fixture = helper_call_fixture(
            MirCallee::untrusted_for_test("tests::shared_helper"),
            &[MirTypeShape::U32],
        );
        fixture.functions[0].locals[2] = local(2, MirLocalRole::Temp, MirTypeShape::U32);
        fixture.functions.push(u32_definition(
            "shared_helper_export",
            MirFunctionKind::InternalHelper,
            false,
        ));
        fixture.functions[1].rust_path = "tests::shared_helper".to_string();

        let module = translate_and_verify(&fixture).expect("collected helper call should lower");
        let kernel = module
            .functions
            .iter()
            .find(|function| function.id.as_str() == "tests::helper_call")
            .expect("kernel definition");
        let calls = kernel
            .body
            .as_ref()
            .expect("kernel body")
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::Call { callee, .. } => Some(callee.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls, ["shared_helper_export"]);
        assert_eq!(
            module
                .functions
                .iter()
                .filter(|function| function.id.as_str() == "shared_helper_export")
                .count(),
            1
        );
        assert!(
            module
                .functions
                .iter()
                .all(|function| function.id.as_str() != "tests::shared_helper")
        );
    }

    #[test]
    fn collected_internal_helpers_resolve_distinct_monomorphizations() {
        let path = "tests::generic_helper";
        let first_identity = MirSemanticInstanceIdentity::monomorphization_for_test(path, 1);
        let second_identity = MirSemanticInstanceIdentity::monomorphization_for_test(path, 2);
        let mut fixture = helper_call_fixture(
            MirCallee::untrusted_semantic_for_test(path, first_identity.clone()),
            &[MirTypeShape::U32],
        );
        fixture.functions[0].locals[2] = local(2, MirLocalRole::Temp, MirTypeShape::U32);
        let mut first =
            u32_definition("generic_helper_u32", MirFunctionKind::InternalHelper, false);
        first.rust_path = path.to_owned();
        first.semantic_instance = Some(first_identity);
        let mut second =
            u32_definition("generic_helper_i32", MirFunctionKind::InternalHelper, false);
        second.rust_path = path.to_owned();
        second.semantic_instance = Some(second_identity);
        fixture.functions.extend([second, first]);

        let module =
            translate_and_verify(&fixture).expect("monomorphized helper call should lower");
        let kernel = module
            .functions
            .iter()
            .find(|function| function.id.as_str() == "tests::helper_call")
            .expect("kernel definition");
        let calls = kernel
            .body
            .as_ref()
            .expect("kernel body")
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::Call { callee, .. } => Some(callee.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls, ["generic_helper_u32"]);
    }

    #[test]
    fn two_kernels_share_one_collected_internal_helper() {
        let mut fixture = helper_call_fixture(
            MirCallee::untrusted_for_test("tests::shared_helper"),
            &[MirTypeShape::U32],
        );
        fixture.functions[0].locals[2] = local(2, MirLocalRole::Temp, MirTypeShape::U32);
        let mut second_kernel = fixture.functions[0].clone();
        second_kernel.export_name = "zeta_kernel".to_string();
        second_kernel.rust_path = "tests::zeta_kernel".to_string();
        let mut helper = u32_definition(
            "shared_helper_export",
            MirFunctionKind::InternalHelper,
            false,
        );
        helper.rust_path = "tests::shared_helper".to_string();
        fixture.functions = vec![second_kernel, helper, fixture.functions.remove(0)];

        let module = translate_and_verify(&fixture).expect("shared helper should lower once");
        assert_eq!(module.kernels.len(), 2);
        assert_eq!(
            module
                .functions
                .iter()
                .filter(|function| function.id.as_str() == "shared_helper_export")
                .count(),
            1
        );
        for kernel in module
            .functions
            .iter()
            .filter(|function| matches!(function.role, fe2o3_kernel_ir::FunctionRole::KernelEntry))
        {
            assert!(
                kernel
                    .body
                    .as_ref()
                    .expect("kernel body")
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| matches!(
                        &operation.kind,
                        OperationKind::Call { callee, .. }
                            if callee.as_str() == "shared_helper_export"
                    ))
            );
        }
    }

    #[test]
    fn collected_internal_helper_signature_mismatch_is_rejected() {
        let mut fixture = helper_call_fixture(
            MirCallee::untrusted_for_test("tests::shared_helper"),
            &[MirTypeShape::F32],
        );
        fixture.functions[0].locals[2] = local(2, MirLocalRole::Temp, MirTypeShape::U32);
        let mut helper = u32_definition(
            "shared_helper_export",
            MirFunctionKind::InternalHelper,
            false,
        );
        helper.rust_path = "tests::shared_helper".to_string();
        fixture.functions.push(helper);

        let errors = translate_and_verify(&fixture).expect_err("ABI mismatch must fail closed");
        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedType));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic.message.contains("operand 0 must lower to")
                && diagnostic.message.contains("found Scalar(F32)")
        }));
    }

    #[test]
    fn duplicate_internal_semantic_instances_are_rejected() {
        let mut alpha = u32_definition("helper_alpha", MirFunctionKind::InternalHelper, false);
        alpha.rust_path = "tests::ambiguous_helper".to_string();
        let mut beta = u32_definition("helper_beta", MirFunctionKind::InternalHelper, false);
        beta.rust_path = "tests::ambiguous_helper".to_string();

        let errors = translate_and_verify(&MirModule {
            functions: vec![alpha, beta],
        })
        .expect_err("duplicate semantic instance must fail closed");
        assert!(errors.contains(TranslationDiagnosticCode::MalformedMir));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("internal semantic instance `tests::ambiguous_helper`")
        }));
    }

    #[test]
    fn authenticated_device_ffi_import_lowers_to_an_external_declaration() {
        let callee = MirCallee::external_import_for_test(
            "external_device_add_v1",
            "C(u32[size=4,align=4])->u32[size=4,align=4]",
            "none",
        );
        let fixture = helper_call_fixture(callee, &[MirTypeShape::U32]);

        let module = translate_and_verify(&fixture).expect("authenticated import should lower");
        let declaration = module
            .functions
            .iter()
            .find(|function| function.id.as_str() == "external_device_add_v1")
            .expect("external import declaration");
        assert_eq!(
            declaration.role,
            fe2o3_kernel_ir::FunctionRole::ExternalImport
        );
        assert_eq!(declaration.body, None);
        assert_eq!(
            declaration.signature,
            Signature::new(
                vec![Type::Scalar(ScalarType::U32)],
                vec![Type::Scalar(ScalarType::U32)]
            )
        );
    }

    #[test]
    fn authenticated_device_ffi_import_rejects_rust_operand_abi_mismatch() {
        let callee = MirCallee::external_import_for_test(
            "external_device_add_v1",
            "C(i32[size=4,align=4])->u32[size=4,align=4]",
            "none",
        );
        let fixture = helper_call_fixture(callee, &[MirTypeShape::U32]);

        let errors = translate_and_verify(&fixture).expect_err("ABI mismatch must fail closed");
        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedType));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic.message.contains("operand 0 must lower to")
                && diagnostic.message.contains("found Scalar(U32)")
        }));
    }

    #[test]
    fn ordinary_host_extern_is_not_promoted_by_matching_symbol_spelling() {
        let fixture = helper_call_fixture(
            MirCallee::untrusted_for_test("external_device_add_v1"),
            &[MirTypeShape::U32],
        );

        let errors = translate_and_verify(&fixture).expect_err("host extern must remain rejected");
        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedCall));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("has no classified trusted device identity")
        }));
    }

    #[test]
    fn trusted_helper_signature_mismatch_is_rejected() {
        let fixture = helper_call_fixture(
            MirCallee::trusted_for_test(TrustedDeviceItem::ThreadIndexOffset),
            &[MirTypeShape::F32, MirTypeShape::USize],
        );

        let errors = translate_and_verify(&fixture).expect_err("wrong helper signature must fail");

        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedType));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic.message.contains("operand 0 must lower to")
                && diagnostic.message.contains("found Scalar(F32)")
        }));
    }

    #[test]
    fn trusted_type_items_are_not_callable_helpers() {
        for item in [
            TrustedDeviceItem::DisjointSlice,
            TrustedDeviceItem::ThreadIndex,
        ] {
            let fixture = helper_call_fixture(MirCallee::trusted_for_test(item), &[]);
            let errors = translate_and_verify(&fixture).expect_err("type call must fail");

            assert!(errors.contains(TranslationDiagnosticCode::UnsupportedCall));
            assert!(errors.diagnostics().iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("is a type, not a callable helper")
            }));
        }
    }

    #[test]
    fn malformed_block_fails_explicitly() {
        let mut fixture = scalar_fixture();
        fixture.functions[0].blocks[1].terminator = None;

        let errors = translate_and_verify(&fixture).expect_err("missing terminator");
        assert!(errors.contains(TranslationDiagnosticCode::MalformedMir));
        assert_eq!(errors.diagnostics()[0].location.block, Some(1));
    }

    #[test]
    fn typed_copy_statement_is_inert_and_rejected() {
        let mut fixture = scalar_fixture();
        fixture.functions[0].blocks[0].statements = vec![MirStatement {
            index: 0,
            kind: MirStatementKind::CopyNonOverlapping,
            destination: None,
            operands: vec![operand(1), operand(2), operand(1)],
            rvalue: None,
            semantic_rvalue_type: None,
            operation: None,
            source: None,
        }];

        let errors = translate_and_verify(&fixture)
            .expect_err("typed copy MIR must not create semantic authority");
        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedStatement));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("compiler import remains disabled")
        }));
    }

    #[test]
    fn raw_pointer_parameter_is_not_reinterpreted_as_global() {
        let mut fixture = scalar_fixture();
        let function = &mut fixture.functions[0];
        function.arg_count = 1;
        function.local_count = 2;
        function.locals = vec![
            local(0, MirLocalRole::Return, MirTypeShape::Unit),
            local(
                1,
                MirLocalRole::Arg,
                MirTypeShape::RawPointer {
                    pointee: Box::new(MirTypeShape::F32),
                    mutable: true,
                },
            ),
        ];
        function.blocks.truncate(1);
        function.blocks[0].statements.clear();
        function.blocks[0].terminator = Some(terminator(MirTerminatorKind::Return));

        let errors = translate_and_verify(&fixture)
            .expect_err("unknown raw-pointer address space must fail closed");
        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedType));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic.message.contains("argument local1")
                && diagnostic.message.contains("unsupported type")
        }));
    }

    #[test]
    fn projected_scalar_operand_reports_source_location() {
        let mut fixture = scalar_fixture();
        fixture.functions[0].blocks[0].statements[0].operands[0] =
            MirOperandRef::Place(MirPlaceRef {
                local: 1,
                projection: vec![MirProjectionElem::Deref],
                semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
            });

        let errors = translate_and_verify(&fixture).expect_err("projection must fail");
        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedType));
        assert_eq!(
            errors.diagnostics()[0]
                .location
                .source
                .as_ref()
                .map(|source| source.file.as_str()),
            Some("tests/scalar.rs")
        );
    }

    fn half_operation_fixture(
        operation: TrustedHalfOperation,
        argument_shapes: &[MirTypeShape],
        result_shape: MirTypeShape,
    ) -> MirModule {
        let destination = argument_shapes.len() + 1;
        let mut locals = vec![local(0, MirLocalRole::Return, MirTypeShape::Unit)];
        locals.extend(
            argument_shapes
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, shape)| local(index + 1, MirLocalRole::Arg, shape)),
        );
        locals.push(local(destination, MirLocalRole::Temp, result_shape));

        MirModule {
            functions: vec![MirFunction {
                semantic_instance: None,
                export_name: "half_operation".to_string(),
                rust_path: "tests::half_operation".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                matrix_frontend_abi: None,
                arg_count: argument_shapes.len(),
                local_count: locals.len(),
                locals,
                blocks: vec![
                    MirBlock {
                        index: 0,
                        statements: Vec::new(),
                        terminator: Some(terminator(MirTerminatorKind::Call {
                            callee: Some(MirCallee::trusted_for_test(
                                TrustedDeviceItem::HalfOperation(operation),
                            )),
                            target: Some(1),
                            destination: Some(place(destination)),
                            operands: (1..=argument_shapes.len()).map(operand).collect(),
                        })),
                    },
                    MirBlock {
                        index: 1,
                        statements: Vec::new(),
                        terminator: Some(terminator(MirTerminatorKind::Return)),
                    },
                ],
            }],
        }
    }

    fn device_math_fixture(
        item: DeviceMathDiagnosticItem,
        numerical_operands: Vec<MirOperandRef>,
        result_shape: MirTypeShape,
        construct_capability: bool,
    ) -> MirModule {
        let mut blocks = Vec::new();
        if construct_capability {
            blocks.push(MirBlock {
                index: 0,
                statements: Vec::new(),
                terminator: Some(terminator(MirTerminatorKind::Call {
                    callee: Some(MirCallee::trusted_for_test(TrustedDeviceItem::DeviceMath(
                        DeviceMathDiagnosticItem::ContextFromCompiler,
                    ))),
                    target: Some(1),
                    destination: Some(place(1)),
                    operands: Vec::new(),
                })),
            });
        } else {
            blocks.push(MirBlock {
                index: 0,
                statements: Vec::new(),
                terminator: Some(terminator(MirTerminatorKind::Goto { target: 1 })),
            });
        }
        blocks.push(MirBlock {
            index: 1,
            statements: if construct_capability {
                vec![assign(
                    0,
                    2,
                    vec![operand(1)],
                    MirRvalueKind::Reference(crate::mir_import::MirBorrowKind::Shared),
                )]
            } else {
                Vec::new()
            },
            terminator: Some(terminator(MirTerminatorKind::Call {
                callee: Some(MirCallee::trusted_for_test(TrustedDeviceItem::DeviceMath(
                    item,
                ))),
                target: Some(2),
                destination: Some(place(3)),
                operands: std::iter::once(operand(2))
                    .chain(numerical_operands)
                    .collect(),
            })),
        });
        blocks.push(MirBlock {
            index: 2,
            statements: Vec::new(),
            terminator: Some(terminator(MirTerminatorKind::Return)),
        });

        MirModule {
            functions: vec![MirFunction {
                semantic_instance: None,
                export_name: "strict_math".to_string(),
                rust_path: "tests::strict_math".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                matrix_frontend_abi: None,
                arg_count: 0,
                local_count: 4,
                locals: vec![
                    local(0, MirLocalRole::Return, MirTypeShape::Unit),
                    local(1, MirLocalRole::Temp, MirTypeShape::DeviceMath),
                    local(
                        2,
                        MirLocalRole::Temp,
                        MirTypeShape::Reference {
                            pointee: Box::new(MirTypeShape::DeviceMath),
                            mutable: false,
                        },
                    ),
                    local(3, MirLocalRole::Temp, result_shape),
                ],
                blocks,
            }],
        }
    }

    fn f32_constant(value: f32) -> MirOperandRef {
        MirOperandRef::Constant {
            ty: MirImportedType {
                kind: MirType::F32,
                rust: "f32".to_string(),
                shape: MirTypeShape::F32,
                semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
            },
            literal: MirConstant::F32Bits(value.to_bits()),
            value: format!("{value:?}_f32"),
        }
    }

    fn scalar_fixture() -> MirModule {
        MirModule {
            functions: vec![MirFunction {
                semantic_instance: None,
                export_name: "scalar".to_string(),
                rust_path: "tests::scalar".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                matrix_frontend_abi: None,
                arg_count: 2,
                local_count: 5,
                locals: vec![
                    local(0, MirLocalRole::Return, MirTypeShape::Unit),
                    local(1, MirLocalRole::Arg, MirTypeShape::F32),
                    local(2, MirLocalRole::Arg, MirTypeShape::F32),
                    local(3, MirLocalRole::Temp, MirTypeShape::F32),
                    local(4, MirLocalRole::Temp, MirTypeShape::Bool),
                ],
                blocks: vec![
                    MirBlock {
                        index: 0,
                        statements: vec![
                            assign(
                                0,
                                3,
                                vec![operand(1), operand(2)],
                                MirRvalueKind::Binary(MirBinaryOp::Add),
                            ),
                            assign(
                                1,
                                4,
                                vec![operand(1), operand(2)],
                                MirRvalueKind::Binary(MirBinaryOp::Lt),
                            ),
                        ],
                        terminator: Some(terminator(MirTerminatorKind::Assert {
                            condition: operand(4),
                            expected: true,
                            target: 1,
                        })),
                    },
                    MirBlock {
                        index: 1,
                        statements: Vec::new(),
                        terminator: Some(terminator(MirTerminatorKind::Return)),
                    },
                ],
            }],
        }
    }

    fn memory_fixture() -> MirModule {
        let indexed = |local| MirPlaceRef {
            local,
            projection: vec![
                MirProjectionElem::Deref,
                MirProjectionElem::Index { local: 3 },
            ],
            semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
        };
        let mut load = assign(
            1,
            5,
            vec![MirOperandRef::Place(indexed(1))],
            MirRvalueKind::Use,
        );
        load.source = Some(source());
        let store = MirStatement {
            index: 2,
            kind: MirStatementKind::Assign,
            destination: Some(indexed(2)),
            operands: vec![operand(5)],
            rvalue: Some(MirRvalueKind::Use),
            semantic_rvalue_type: None,
            operation: Some("store".to_string()),
            source: Some(source()),
        };

        MirModule {
            functions: vec![MirFunction {
                semantic_instance: None,
                export_name: "memory".to_string(),
                rust_path: "tests::memory".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                matrix_frontend_abi: None,
                arg_count: 3,
                local_count: 6,
                locals: vec![
                    local(0, MirLocalRole::Return, MirTypeShape::Unit),
                    MirLocal {
                        index: 1,
                        role: MirLocalRole::Arg,
                        ty: MirImportedType {
                            kind: MirType::Slice,
                            rust: "&[f32]".to_string(),
                            shape: MirTypeShape::Slice {
                                element: Box::new(MirTypeShape::F32),
                                mutable: false,
                            },
                            semantic_identity:
                                crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
                        },
                    },
                    MirLocal {
                        index: 2,
                        role: MirLocalRole::Arg,
                        ty: MirImportedType {
                            kind: MirType::DisjointSlice,
                            rust: "DisjointSlice<f32>".to_string(),
                            shape: MirTypeShape::DisjointSlice {
                                element: Box::new(MirTypeShape::F32),
                            },
                            semantic_identity:
                                crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
                        },
                    },
                    local(3, MirLocalRole::Arg, MirTypeShape::USize),
                    local(4, MirLocalRole::Temp, MirTypeShape::USize),
                    local(5, MirLocalRole::Temp, MirTypeShape::F32),
                ],
                blocks: vec![MirBlock {
                    index: 0,
                    statements: vec![
                        assign(
                            0,
                            4,
                            vec![operand(1)],
                            MirRvalueKind::Unary(MirUnaryOp::PtrMetadata),
                        ),
                        load,
                        store,
                    ],
                    terminator: Some(terminator(MirTerminatorKind::Return)),
                }],
            }],
        }
    }

    fn helper_call_fixture(callee: MirCallee, argument_shapes: &[MirTypeShape]) -> MirModule {
        let destination = argument_shapes.len() + 1;
        let mut locals = vec![local(0, MirLocalRole::Return, MirTypeShape::Unit)];
        locals.extend(
            argument_shapes
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, shape)| local(index + 1, MirLocalRole::Arg, shape)),
        );
        locals.push(local(
            destination,
            MirLocalRole::Temp,
            MirTypeShape::Unknown,
        ));

        MirModule {
            functions: vec![MirFunction {
                semantic_instance: None,
                export_name: "helper_call".to_string(),
                rust_path: "tests::helper_call".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                matrix_frontend_abi: None,
                arg_count: argument_shapes.len(),
                local_count: locals.len(),
                locals,
                blocks: vec![
                    MirBlock {
                        index: 0,
                        statements: Vec::new(),
                        terminator: Some(terminator(MirTerminatorKind::Call {
                            callee: Some(callee),
                            target: Some(1),
                            destination: Some(place(destination)),
                            operands: (1..=argument_shapes.len()).map(operand).collect(),
                        })),
                    },
                    MirBlock {
                        index: 1,
                        statements: Vec::new(),
                        terminator: Some(terminator(MirTerminatorKind::Return)),
                    },
                ],
            }],
        }
    }

    fn u32_definition(name: &str, kind: MirFunctionKind, add: bool) -> MirFunction {
        let arg_count = if add { 2 } else { 1 };
        let operands = if add {
            vec![operand(1), operand(2)]
        } else {
            vec![operand(1)]
        };
        let rvalue = if add {
            MirRvalueKind::Binary(MirBinaryOp::Add)
        } else {
            MirRvalueKind::Use
        };
        let mut locals = vec![local(0, MirLocalRole::Return, MirTypeShape::U32)];
        locals.extend(
            (1..=arg_count).map(|index| local(index, MirLocalRole::Arg, MirTypeShape::U32)),
        );
        MirFunction {
            semantic_instance: None,
            export_name: name.to_string(),
            rust_path: format!("tests::{name}"),
            kind,
            typed_profile: None,
            frontend_contract: None,
            matrix_frontend_abi: None,
            arg_count,
            local_count: locals.len(),
            locals,
            blocks: vec![MirBlock {
                index: 0,
                statements: vec![assign(0, 0, operands, rvalue)],
                terminator: Some(terminator(MirTerminatorKind::Return)),
            }],
        }
    }

    fn empty_kernel_with_contract(contract: KernelFrontendContractV1) -> MirFunction {
        MirFunction {
            semantic_instance: None,
            export_name: "kernel".to_owned(),
            rust_path: "tests::kernel".to_owned(),
            kind: MirFunctionKind::KernelEntry,
            typed_profile: None,
            frontend_contract: Some(
                crate::collector::AuthenticatedKernelFrontendContractV1::for_test(contract),
            ),
            matrix_frontend_abi: None,
            arg_count: 0,
            local_count: 1,
            locals: vec![local(0, MirLocalRole::Return, MirTypeShape::Unit)],
            blocks: vec![MirBlock {
                index: 0,
                statements: Vec::new(),
                terminator: Some(terminator(MirTerminatorKind::Return)),
            }],
        }
    }

    fn launch_contract(
        required: Option<[u32; 3]>,
        maximum: Option<[u32; 3]>,
        occupancy: Option<u16>,
    ) -> KernelFrontendContractV1 {
        let required = required.map(|value| FrontendWorkgroupDimensionsV1::new(value).unwrap());
        let maximum = maximum.map(|value| FrontendWorkgroupDimensionsV1::new(value).unwrap());
        KernelFrontendContractV1::new(
            Some(FrontendLaunchBoundsV1::new(required, maximum, occupancy).unwrap()),
            None,
        )
        .unwrap()
    }

    fn local(index: usize, role: MirLocalRole, shape: MirTypeShape) -> MirLocal {
        let (kind, rust) = match shape {
            MirTypeShape::Unit => (MirType::Unit, "()"),
            MirTypeShape::Bool => (MirType::I1, "bool"),
            MirTypeShape::U32 => (MirType::I32, "u32"),
            MirTypeShape::F32 => (MirType::F32, "f32"),
            _ => (MirType::Unknown, "<unknown>"),
        };
        MirLocal {
            index,
            role,
            ty: MirImportedType {
                kind,
                rust: rust.to_string(),
                shape,
                semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
            },
        }
    }

    fn assign(
        index: usize,
        destination: usize,
        operands: Vec<MirOperandRef>,
        rvalue: MirRvalueKind,
    ) -> MirStatement {
        MirStatement {
            index,
            kind: MirStatementKind::Assign,
            destination: Some(place(destination)),
            operands,
            rvalue: Some(rvalue),
            semantic_rvalue_type: None,
            operation: Some("structured".to_string()),
            source: Some(source()),
        }
    }

    fn terminator(kind: MirTerminatorKind) -> MirTerminator {
        MirTerminator {
            kind,
            source: Some(source()),
        }
    }

    fn operand(local: usize) -> MirOperandRef {
        MirOperandRef::Place(place(local))
    }

    fn place(local: usize) -> MirPlaceRef {
        MirPlaceRef {
            local,
            projection: Vec::new(),
            semantic_identity: crate::mir_import::MirSemanticTypeEvidence::OmittedV2Fixture,
        }
    }

    fn source() -> MirSourceLocation {
        MirSourceLocation {
            file: "tests/scalar.rs".to_string(),
            line: 1,
            column: 1,
        }
    }
}
