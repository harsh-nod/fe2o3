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
    MirBinaryOp, MirBlock, MirCallee, MirCheckedTiled2dCallEvidenceV1, MirConstant, MirFunction,
    MirFunctionKind, MirKernelProfile, MirModule, MirOperandRef, MirPlaceRef, MirProjectionElem,
    MirReferenceSemantics, MirRvalueKind, MirSemanticInstanceIdentity, MirSourceLocation,
    MirStatement, MirStatementKind, MirTerminator, MirTerminatorKind, MirTypeShape, MirUnaryOp,
};
use crate::trusted_device_items::{TrustedDeviceItem, TrustedHalfOperation};
use dialect_amdgcn::{DeviceMathDiagnosticItem, recognized_device_math_operation};
use dialect_mir::MirCastKind;
use fe2o3_amd_target::AmdTargetId;
use fe2o3_kernel_analysis::{KernelCheckStatusV1, run_general_kernel_checks_from_verified_v1};
#[cfg(test)]
use fe2o3_kernel_ir::verify_module;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AmdGpuDiagnosticOperation, BasicBlock, BinaryOp, BlockId, CastKind,
    ComparePredicate, Constant, ExplicitLaunchExtent1d, FloatConversionKind, FloatOperation,
    FormalIndexWidth, Function, FunctionId, Kernel, LaunchDomain, LaunchExtent,
    MATRIX_PROJECTED_KERNARG_POLICY_NAMESPACE_V1, MATRIX_SOURCE_ABI_OBSERVATION_NAMESPACE_V2,
    MatrixFrontendBindingV2, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    SwitchCase, TargetCapability, Terminator, Type, UnaryOp, ValueDef, ValueId, WorkgroupSize,
    gfx942_xnack_minus_target_capability, verify_module_ref,
};
use reserved_fe2o3_symbols::{
    DeviceFfiAddressSpaceV1, DeviceFfiPhysicalResultV1, DeviceFfiPhysicalTypeV1,
    DeviceFfiPointerAccessV1, DeviceFfiScalarTypeV1, KernelBindingIdV1,
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
    KernelCheckRejected,
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
    elides_generated_result: bool,
}

#[derive(Clone, Copy, Debug)]
struct InternalKernelContext<'mir> {
    root: &'mir MirFunction,
    source_abi: Option<&'mir MirFunction>,
    elides_generated_result: bool,
    sealed_generated_kernel_binding: Option<KernelBindingIdV1>,
}

const GENERATED_KERNEL_ENTRY_PREFIX_V1: &str = "__fe2o3_host_kernel_v1_";
const GENERATED_KERNEL_BODY_PREFIX_V1: &str = "__fe2o3_kernel_body_v1_";

fn internal_kernel_contexts_v1<'mir>(
    mir: &'mir MirModule,
) -> Result<
    BTreeMap<MirSemanticInstanceIdentity, InternalKernelContext<'mir>>,
    Vec<TranslationDiagnostic>,
> {
    let mut diagnostics = Vec::new();
    let mut internal_functions = BTreeMap::new();
    for function in mir.functions.iter().filter(|function| {
        matches!(
            function.kind,
            MirFunctionKind::InternalHelper | MirFunctionKind::DeviceFfiExport
        )
    }) {
        let identity = function.semantic_instance_v1();
        if let Some(previous) = internal_functions.insert(identity, function) {
            diagnostics.push(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(function),
                format!(
                    "internal semantic instance `{}` resolves to both `{}` and `{}`",
                    function.rust_path, previous.export_name, function.export_name
                ),
            ));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut roots = mir
        .functions
        .iter()
        .filter(|function| function.kind == MirFunctionKind::KernelEntry)
        .collect::<Vec<_>>();
    roots.sort_by(|lhs, rhs| {
        (&lhs.export_name, &lhs.rust_path).cmp(&(&rhs.export_name, &rhs.rust_path))
    });

    let mut reachable_roots = BTreeMap::<MirSemanticInstanceIdentity, Vec<&MirFunction>>::new();
    for root in &roots {
        let mut pending = internal_callees(root, &internal_functions);
        let mut visited = BTreeSet::new();
        while let Some(identity) = pending.pop() {
            if !visited.insert(identity.clone()) {
                continue;
            }
            let Some(function) = internal_functions.get(&identity).copied() else {
                continue;
            };
            if function.kind != MirFunctionKind::InternalHelper {
                continue;
            }
            reachable_roots.entry(identity).or_default().push(*root);
            pending.extend(internal_callees(function, &internal_functions));
        }
    }

    let mut contexts = BTreeMap::new();
    for (identity, owners) in reachable_roots {
        if let [root] = owners.as_slice() {
            contexts.insert(
                identity,
                InternalKernelContext {
                    root,
                    source_abi: None,
                    elides_generated_result: false,
                    sealed_generated_kernel_binding: None,
                },
            );
        }
    }

    let mut sealed_bodies = BTreeSet::new();
    for root in &roots {
        for block in &root.blocks {
            let Some(MirTerminator {
                kind:
                    MirTerminatorKind::Call {
                        callee: Some(callee),
                        ..
                    },
                ..
            }) = block.terminator.as_ref()
            else {
                continue;
            };
            let Some(evidence) = callee.authenticated_kernel_body_bridge_v1() else {
                continue;
            };
            let Some(Ok(callee_identity)) = callee.semantic_instance_identity() else {
                diagnostics.push(generated_bridge_diagnostic(
                    root,
                    "compiler-sealed body edge has no resolved semantic callee identity",
                ));
                continue;
            };
            let root_identity = root.semantic_instance_v1();
            if evidence.root() != &root_identity || evidence.body() != callee_identity {
                diagnostics.push(generated_bridge_diagnostic(
                    root,
                    "compiler-sealed root/body semantic identities do not match the call edge",
                ));
                continue;
            }
            let Some(helper) = internal_functions.get(callee_identity).copied() else {
                diagnostics.push(generated_bridge_diagnostic(
                    root,
                    "compiler-sealed generated body is absent from the imported closure",
                ));
                continue;
            };
            if helper.kind != MirFunctionKind::InternalHelper
                || root.typed_profile != Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3)
                || evidence.discarded_return_local() != 0
                || !generated_item_matches_binding_v1(
                    root,
                    GENERATED_KERNEL_ENTRY_PREFIX_V1,
                    evidence.kernel_binding(),
                )
                || !generated_item_matches_binding_v1(
                    helper,
                    GENERATED_KERNEL_BODY_PREFIX_V1,
                    evidence.kernel_binding(),
                )
                || generated_item_module_v1(root) != generated_item_module_v1(helper)
            {
                diagnostics.push(generated_bridge_diagnostic(
                    helper,
                    "compiler-sealed binding, profile, role, module, or return-local invariant changed",
                ));
                continue;
            }
            if !sealed_bodies.insert(callee_identity.clone()) {
                diagnostics.push(generated_bridge_diagnostic(
                    helper,
                    "generated body carries more than one compiler-sealed bridge edge",
                ));
                continue;
            }
            if let Err(error) = validate_generated_result_bridge_v1(mir, root, helper, evidence) {
                diagnostics.push(error);
                continue;
            }
            contexts.insert(
                helper.semantic_instance_v1(),
                InternalKernelContext {
                    root,
                    source_abi: Some(root),
                    elides_generated_result: true,
                    sealed_generated_kernel_binding: Some(evidence.kernel_binding()),
                },
            );
        }
    }

    for helper in mir
        .functions
        .iter()
        .filter(|function| function.kind == MirFunctionKind::InternalHelper)
    {
        if generated_item_suffix(helper, GENERATED_KERNEL_BODY_PREFIX_V1).is_some()
            && !sealed_bodies.contains(&helper.semantic_instance_v1())
        {
            let helper_identity = helper.semantic_instance_v1();
            let rejection = mir
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .filter_map(|block| block.terminator.as_ref())
                .filter_map(|terminator| match &terminator.kind {
                    MirTerminatorKind::Call {
                        callee: Some(callee),
                        ..
                    } if matches!(
                        callee.semantic_instance_identity(),
                        Some(Ok(identity)) if identity == &helper_identity
                    ) =>
                    {
                        callee.kernel_body_bridge_rejection_v1()
                    }
                    _ => None,
                })
                .next();
            diagnostics.push(generated_bridge_diagnostic(
                helper,
                rejection.map_or_else(
                    || {
                        "generated-looking kernel body has no valid compiler-sealed owner edge"
                            .to_owned()
                    },
                    |detail| format!("compiler refused to seal the generated body edge: {detail}"),
                ),
            ));
        }
    }

    if diagnostics.is_empty() {
        Ok(contexts)
    } else {
        Err(diagnostics)
    }
}

fn internal_callees(
    function: &MirFunction,
    internal_functions: &BTreeMap<MirSemanticInstanceIdentity, &MirFunction>,
) -> Vec<MirSemanticInstanceIdentity> {
    function
        .blocks
        .iter()
        .filter_map(|block| block.terminator.as_ref())
        .filter_map(|terminator| match &terminator.kind {
            MirTerminatorKind::Call {
                callee: Some(callee),
                ..
            } => callee.semantic_instance_identity()?.ok(),
            _ => None,
        })
        .filter(|identity| internal_functions.contains_key(*identity))
        .cloned()
        .collect()
}

fn generated_item_suffix<'a>(function: &'a MirFunction, prefix: &str) -> Option<&'a str> {
    let basename = function.rust_path.rsplit("::").next()?;
    let suffix = basename.strip_prefix(prefix)?;
    (suffix.len() == 64
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then_some(suffix)
}

fn generated_item_matches_binding_v1(
    function: &MirFunction,
    prefix: &str,
    binding: reserved_fe2o3_symbols::KernelBindingIdV1,
) -> bool {
    let expected = binding.to_hex();
    generated_item_suffix(function, prefix) == Some(expected.as_str())
}

fn generated_item_module_v1(function: &MirFunction) -> Option<&str> {
    function
        .rust_path
        .rsplit_once("::")
        .map(|(module, _)| module)
}

fn generated_bridge_diagnostic(
    function: &MirFunction,
    message: impl Into<String>,
) -> TranslationDiagnostic {
    diagnostic(
        TranslationDiagnosticCode::MalformedMir,
        TranslationLocation::function(function),
        format!(
            "authenticated generated kernel result bridge rejected: {}",
            message.into()
        ),
    )
}

fn validate_generated_result_bridge_v1(
    mir: &MirModule,
    root: &MirFunction,
    helper: &MirFunction,
    evidence: &crate::mir_import::MirAuthenticatedKernelBodyBridgeV1,
) -> Result<(), TranslationDiagnostic> {
    let helper_identity = helper.semantic_instance_v1();
    let call_count = mir
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .filter_map(|block| block.terminator.as_ref())
        .filter(|terminator| {
            matches!(
                &terminator.kind,
                MirTerminatorKind::Call {
                    callee: Some(callee),
                    ..
                } if matches!(
                    callee.semantic_instance_identity(),
                    Some(Ok(identity)) if identity == &helper_identity
                )
            )
        })
        .count();
    if call_count != 1 {
        return Err(generated_bridge_diagnostic(
            helper,
            format!("binding-matched body must have exactly one call site; found {call_count}"),
        ));
    }

    let root_return = imported_local(root, 0).ok_or_else(|| {
        generated_bridge_diagnostic(root, "generated kernel entry has no return local0")
    })?;
    if root_return.role != crate::mir_import::MirLocalRole::Return
        || root_return.ty.shape != MirTypeShape::Unit
    {
        return Err(generated_bridge_diagnostic(
            root,
            "generated kernel entry does not have an exact unit return",
        ));
    }
    let helper_return = imported_local(helper, 0).ok_or_else(|| {
        generated_bridge_diagnostic(helper, "generated kernel body has no return local0")
    })?;
    if helper_return.role != crate::mir_import::MirLocalRole::Return
        || !is_standard_result_shape(&helper_return.ty.shape)
    {
        return Err(generated_bridge_diagnostic(
            helper,
            "generated kernel body return is not the exact core::result::Result ADT",
        ));
    }

    let root_args = imported_arguments(root);
    let helper_args = imported_arguments(helper);
    if root.arg_count != helper.arg_count
        || root_args.len() != root.arg_count
        || helper_args.len() != helper.arg_count
        || !root_args
            .iter()
            .zip(&helper_args)
            .all(|(root_arg, helper_arg)| root_arg.ty == helper_arg.ty)
    {
        return Err(generated_bridge_diagnostic(
            root,
            "generated entry and body argument lists are not semantically identical",
        ));
    }

    let mut blocks = root.blocks.iter().collect::<Vec<_>>();
    blocks.sort_by_key(|block| block.index);
    let [entry, exit] = blocks.as_slice() else {
        return Err(generated_bridge_diagnostic(
            root,
            "generated kernel entry must contain exactly two MIR blocks",
        ));
    };
    if entry.index != 0 || exit.index != 1 {
        return Err(generated_bridge_diagnostic(
            root,
            "generated kernel entry blocks must be exactly bb0 and bb1",
        ));
    }

    let Some(MirTerminator {
        kind:
            MirTerminatorKind::Call {
                callee: Some(callee),
                target: Some(target),
                destination: Some(destination),
                operands,
            },
        ..
    }) = entry.terminator.as_ref()
    else {
        return Err(generated_bridge_diagnostic(
            root,
            "bb0 is not the exact direct generated-body call",
        ));
    };
    if *target != 1
        || !matches!(
            callee.semantic_instance_identity(),
            Some(Ok(identity)) if identity == &helper_identity
        )
        || callee.authenticated_kernel_body_bridge_v1() != Some(evidence)
        || !destination.projection.is_empty()
    {
        return Err(generated_bridge_diagnostic(
            root,
            "bb0 call target, callee identity, or result destination is not canonical",
        ));
    }
    let result_local = imported_local(root, destination.local).ok_or_else(|| {
        generated_bridge_diagnostic(root, "bb0 call destination has no imported local")
    })?;
    if result_local.role != crate::mir_import::MirLocalRole::Temp
        || result_local.ty != helper_return.ty
    {
        return Err(generated_bridge_diagnostic(
            root,
            "bb0 call destination is not an exact temporary of the body Result type",
        ));
    }
    if operands.len() != root_args.len()
        || !operands.iter().zip(&root_args).all(|(operand, argument)| {
            matches!(
                operand,
                MirOperandRef::Place(place)
                    if place.local == argument.index && place.projection.is_empty()
            )
        })
    {
        return Err(generated_bridge_diagnostic(
            root,
            "bb0 does not forward each kernel argument exactly once and in order",
        ));
    }
    if !matches!(entry.statements.as_slice(), [statement]
        if is_exact_storage_statement(statement, MirStatementKind::StorageLive, destination.local))
    {
        return Err(generated_bridge_diagnostic(
            root,
            "bb0 must contain only StorageLive for the discarded Result temporary",
        ));
    }
    if !matches!(exit.statements.as_slice(), [statement]
        if is_exact_storage_statement(statement, MirStatementKind::StorageDead, destination.local))
        || !matches!(
            exit.terminator.as_ref().map(|terminator| &terminator.kind),
            Some(MirTerminatorKind::Return)
        )
    {
        return Err(generated_bridge_diagnostic(
            root,
            "bb1 must only retire the discarded Result temporary and return",
        ));
    }

    validate_elided_generated_result_v1(helper, helper_return)
}

fn imported_local(function: &MirFunction, index: usize) -> Option<&crate::mir_import::MirLocal> {
    function.locals.iter().find(|local| local.index == index)
}

fn is_standard_result_shape(shape: &MirTypeShape) -> bool {
    matches!(
        shape,
        MirTypeShape::Adt { identity }
            if matches!(identity.as_str(), "core::result::Result" | "std::result::Result")
    )
}

fn is_standard_option_shape(shape: &MirTypeShape) -> bool {
    matches!(
        shape,
        MirTypeShape::Adt { identity }
            if matches!(identity.as_str(), "core::option::Option" | "std::option::Option")
    )
}

fn is_standard_control_flow_shape(shape: &MirTypeShape) -> bool {
    matches!(
        shape,
        MirTypeShape::Adt { identity }
            if matches!(
                identity.as_str(),
                "core::ops::control_flow::ControlFlow" | "std::ops::ControlFlow"
            )
    )
}

fn imported_arguments(function: &MirFunction) -> Vec<&crate::mir_import::MirLocal> {
    let mut arguments = function
        .locals
        .iter()
        .filter(|local| local.role == crate::mir_import::MirLocalRole::Arg)
        .collect::<Vec<_>>();
    arguments.sort_by_key(|local| local.index);
    arguments
}

fn is_exact_storage_statement(
    statement: &MirStatement,
    kind: MirStatementKind,
    local: usize,
) -> bool {
    statement.kind == kind
        && matches!(
            statement.destination.as_ref(),
            Some(destination) if destination.local == local && destination.projection.is_empty()
        )
        && statement.operands.is_empty()
        && statement.rvalue.is_none()
        && statement.semantic_rvalue_type.is_none()
        && statement.operation.is_none()
}

fn validate_elided_generated_result_v1(
    helper: &MirFunction,
    return_local: &crate::mir_import::MirLocal,
) -> Result<(), TranslationDiagnostic> {
    let mut writes = 0usize;
    for block in &helper.blocks {
        for statement in &block.statements {
            if statement
                .operands
                .iter()
                .any(|operand| operand_mentions_local(operand, 0))
            {
                return Err(generated_bridge_diagnostic(
                    helper,
                    format!(
                        "bb{} stmt{} reads return local0",
                        block.index, statement.index
                    ),
                ));
            }
            let Some(destination) = statement.destination.as_ref() else {
                continue;
            };
            if projection_mentions_index_local(destination, 0) {
                return Err(generated_bridge_diagnostic(
                    helper,
                    format!(
                        "bb{} stmt{} indexes through return local0",
                        block.index, statement.index
                    ),
                ));
            }
            if destination.local != 0 {
                continue;
            }
            if destination.projection.is_empty()
                && statement.kind == MirStatementKind::Assign
                && is_exact_discarded_result_rvalue(helper, statement, return_local)
            {
                writes = writes.saturating_add(1);
                continue;
            }
            return Err(generated_bridge_diagnostic(
                helper,
                format!(
                    "bb{} stmt{} writes return local0 with a non-canonical Result construction",
                    block.index, statement.index
                ),
            ));
        }

        let Some(terminator) = block.terminator.as_ref() else {
            return Err(generated_bridge_diagnostic(
                helper,
                format!("bb{} has no terminator", block.index),
            ));
        };
        if terminator_operands(&terminator.kind)
            .into_iter()
            .any(|operand| operand_mentions_local(operand, 0))
            || matches!(
                &terminator.kind,
                MirTerminatorKind::Call {
                    destination: Some(destination),
                    ..
                } if destination.local == 0 || projection_mentions_index_local(destination, 0)
            )
        {
            return Err(generated_bridge_diagnostic(
                helper,
                format!("bb{} observes or overwrites return local0", block.index),
            ));
        }
    }
    if writes == 0 {
        return Err(generated_bridge_diagnostic(
            helper,
            "generated kernel body never constructs its discarded Result",
        ));
    }
    Ok(())
}

fn is_exact_discarded_result_rvalue(
    helper: &MirFunction,
    statement: &MirStatement,
    return_local: &crate::mir_import::MirLocal,
) -> bool {
    match statement.rvalue {
        Some(MirRvalueKind::AdtAggregate { .. }) => {
            statement.semantic_rvalue_type.as_ref() == Some(&return_local.ty.semantic_identity)
        }
        Some(MirRvalueKind::Use) => match statement.operands.as_slice() {
            [MirOperandRef::Constant { ty, .. }] => ty == &return_local.ty,
            [MirOperandRef::Place(place)] if place.projection.is_empty() => {
                imported_local(helper, place.local).is_some_and(|local| local.ty == return_local.ty)
            }
            _ => false,
        },
        _ => false,
    }
}

fn operand_mentions_local(operand: &MirOperandRef, local: usize) -> bool {
    matches!(
        operand,
        MirOperandRef::Place(place)
            if place.local == local || projection_mentions_index_local(place, local)
    )
}

fn projection_mentions_index_local(place: &MirPlaceRef, local: usize) -> bool {
    place.projection.iter().any(|projection| {
        matches!(projection, MirProjectionElem::Index { local: index } if *index == local)
    })
}

fn terminator_operands(terminator: &MirTerminatorKind) -> Vec<&MirOperandRef> {
    match terminator {
        MirTerminatorKind::SwitchInt { discriminant, .. } => vec![discriminant],
        MirTerminatorKind::Call { operands, .. } => operands.iter().collect(),
        MirTerminatorKind::Assert { condition, .. } => vec![condition],
        MirTerminatorKind::Return
        | MirTerminatorKind::Unreachable
        | MirTerminatorKind::Goto { .. }
        | MirTerminatorKind::Drop { .. }
        | MirTerminatorKind::Other => Vec::new(),
    }
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

    let internal_contexts = match internal_kernel_contexts_v1(mir) {
        Ok(contexts) => contexts,
        Err(context_diagnostics) => return Err(errors(context_diagnostics)),
    };

    for function in functions.iter().copied().filter(|function| {
        matches!(
            function.kind,
            MirFunctionKind::InternalHelper | MirFunctionKind::DeviceFfiExport
        )
    }) {
        let context = internal_contexts.get(&function.semantic_instance_v1());
        let source_abi = context
            .and_then(|context| context.source_abi)
            .unwrap_or(function);
        let elides_generated_result =
            context.is_some_and(|context| context.elides_generated_result);
        let signature =
            match declared_function_signature(function, source_abi, elides_generated_result) {
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
                elides_generated_result,
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

        let internal_context = internal_contexts.get(&function.semantic_instance_v1());
        let kernel_context = if function.kind == MirFunctionKind::KernelEntry {
            Some(function)
        } else {
            internal_context.map(|context| context.root)
        };
        let source_abi = internal_context
            .and_then(|context| context.source_abi)
            .unwrap_or(function);
        let elides_generated_result =
            internal_context.is_some_and(|context| context.elides_generated_result);
        let sealed_generated_kernel_binding =
            internal_context.and_then(|context| context.sealed_generated_kernel_binding);
        let workgroup_size = kernel_context
            .and_then(|root| launch_contracts.get(root.export_name.as_str()))
            .copied()
            .flatten();

        match FunctionLowerer::new(
            function,
            kernel_context,
            source_abi,
            elides_generated_result,
            sealed_generated_kernel_binding,
            &mut declarations,
            &internal_definitions,
            workgroup_size,
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

    verify_and_check_module(module)
}

fn verify_and_check_module(module: Module) -> Result<Module, TranslationErrors> {
    let verified = match verify_module_ref(&module) {
        Ok(verified) => verified,
        Err(verification_errors) => {
            let diagnostics = verification_errors
                .diagnostics()
                .iter()
                .map(|verification| {
                    let operation = verification
                        .location
                        .function
                        .as_ref()
                        .and_then(|function| module.function(function))
                        .and_then(|function| function.body.as_ref())
                        .and_then(|body| {
                            let block = verification.location.block?;
                            body.blocks.iter().find(|candidate| candidate.id == block)
                        })
                        .and_then(|block| block.operations.get(verification.location.operation?));
                    let operation = operation.map_or_else(String::new, |operation| {
                        let function = verification
                            .location
                            .function
                            .as_ref()
                            .and_then(|function| module.function(function));
                        let definitions = operation
                            .operands()
                            .into_iter()
                            .map(|operand| {
                                let site = function
                                    .and_then(|function| function.body.as_ref())
                                    .and_then(|body| {
                                        if body.parameters.contains(&operand) {
                                            return Some("function parameter".to_owned());
                                        }
                                        for block in &body.blocks {
                                            if block
                                                .parameters
                                                .iter()
                                                .any(|parameter| parameter.id == operand)
                                            {
                                                return Some(format!("bb{} parameter", block.id.0));
                                            }
                                            if let Some(index) =
                                                block.operations.iter().position(|operation| {
                                                    operation
                                                        .results
                                                        .iter()
                                                        .any(|result| result.id == operand)
                                                })
                                            {
                                                return Some(format!("bb{} op{index}", block.id.0));
                                            }
                                        }
                                        None
                                    })
                                    .unwrap_or_else(|| "undefined".to_owned());
                                format!("{operand}={site}")
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!(
                            "; operation {:?}; operand definitions [{definitions}]",
                            operation.kind
                        )
                    });
                    TranslationDiagnostic {
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
                        message: format!(
                            "{:?}: {}{operation}",
                            verification.code, verification.message
                        ),
                    }
                })
                .collect();
            return Err(errors(diagnostics));
        }
    };

    let mut check_diagnostics = Vec::new();
    for kernel in &module.kernels {
        let launch_extent = match kernel.domain {
            LaunchDomain::D1 {
                x: LaunchExtent::Static(x),
            } => ExplicitLaunchExtent1d::Exact(u64::from(x)),
            _ => ExplicitLaunchExtent1d::Unknown,
        };
        let report = run_general_kernel_checks_from_verified_v1(
            verified,
            &kernel.id,
            launch_extent,
            FormalIndexWidth::Bits64,
        )
        .expect("verified module contains the selected kernel");
        if report.status() != KernelCheckStatusV1::Rejected {
            continue;
        }
        check_diagnostics.extend(report.rejected_findings().map(|finding| {
            let location = finding.operation_location();
            TranslationDiagnostic {
                location: TranslationLocation {
                    function: Some(kernel.entry.as_str().to_owned()),
                    block: location.map(|location| location.block.0 as usize),
                    statement: None,
                    terminator: false,
                    operation: location.map(|location| location.operation_index),
                    source: None,
                },
                code: TranslationDiagnosticCode::KernelCheckRejected,
                message: finding.to_string(),
            }
        }));
    }
    if !check_diagnostics.is_empty() {
        return Err(errors(check_diagnostics));
    }

    Ok(module)
}

fn kernel_ir_launch_contract(
    function: &MirFunction,
) -> Result<Option<WorkgroupSize>, TranslationDiagnostic> {
    const DEFAULT_TYPED_WORKGROUP: WorkgroupSize = WorkgroupSize::new(256, 1, 1);
    const GENERAL_WAVE64_WORKGROUP: WorkgroupSize = WorkgroupSize::new(64, 1, 1);
    let typed_default = function.typed_profile.map(|_| DEFAULT_TYPED_WORKGROUP);
    let Some(authenticated) = &function.frontend_contract else {
        return Ok(typed_default);
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
        return Ok(typed_default);
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
    match function.typed_profile {
        Some(MirKernelProfile::VecAddRustcLayoutV2) if authenticated != DEFAULT_TYPED_WORKGROUP => {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                TranslationLocation::function(function),
                "authenticated launch contract disagrees with the VecAdd V2 profile's exact 256x1x1 workgroup",
            ));
        }
        Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3)
            if authenticated != DEFAULT_TYPED_WORKGROUP
                && authenticated != GENERAL_WAVE64_WORKGROUP =>
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                TranslationLocation::function(function),
                "General V3 kernel IR lowering requires an exact 64x1x1 or 256x1x1 authenticated workgroup",
            ));
        }
        _ => {}
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
    FixedArray4([ValueId; 4]),
    OptionPointer {
        discriminant: ValueId,
        payload: ValueId,
        some_entry: Option<usize>,
    },
    OptionTiled2dWitness {
        discriminant: ValueId,
        raw: ValueId,
        evidence: MirCheckedTiled2dCallEvidenceV1,
        some_entry: Option<usize>,
    },
    Tiled2dWitness {
        raw: ValueId,
        evidence: MirCheckedTiled2dCallEvidenceV1,
        some_entry: usize,
    },
}

impl LocalBinding {
    fn option_discriminant(self) -> Option<ValueId> {
        match self {
            Self::OptionPointer { discriminant, .. }
            | Self::OptionTiled2dWitness { discriminant, .. } => Some(discriminant),
            _ => None,
        }
    }

    fn with_option_some_entry(self, some_entry: usize) -> Option<Self> {
        match self {
            Self::OptionPointer {
                discriminant,
                payload,
                ..
            } => Some(Self::OptionPointer {
                discriminant,
                payload,
                some_entry: Some(some_entry),
            }),
            Self::OptionTiled2dWitness {
                discriminant,
                raw,
                evidence,
                ..
            } => Some(Self::OptionTiled2dWitness {
                discriminant,
                raw,
                evidence,
                some_entry: Some(some_entry),
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TryCarrierKind {
    Option,
    Result,
    ControlFlow,
}

impl TryCarrierKind {
    const fn success_variant(self) -> usize {
        match self {
            Self::Option => 1,
            Self::Result | Self::ControlFlow => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TryCarrier {
    origin_option: usize,
    kind: TryCarrierKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedPointerOrigin {
    pointer_local: usize,
    base_slice_local: usize,
    index_local: usize,
    length_local: usize,
    bounds_local: usize,
    bounds_block: usize,
    some_block: usize,
    none_block: usize,
    join_block: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TryOrigin {
    Tiled2d,
    CheckedPointer(CheckedPointerOrigin),
}

#[derive(Clone, Debug, Default)]
struct TryBridgePlan {
    origins: BTreeMap<usize, TryOrigin>,
    carriers: BTreeMap<usize, TryCarrier>,
    payload_values: BTreeMap<usize, usize>,
    success_entries: BTreeMap<usize, usize>,
    payload_guard_entries: BTreeMap<usize, usize>,
}

impl TryBridgePlan {
    fn analyze(function: &MirFunction) -> Result<Self, String> {
        let mut plan = Self::default();
        for block in &function.blocks {
            let Some(MirTerminator {
                kind:
                    MirTerminatorKind::Call {
                        callee: Some(callee),
                        destination: Some(destination),
                        ..
                    },
                ..
            }) = block.terminator.as_ref()
            else {
                continue;
            };
            if callee.trusted_item() != Some(TrustedDeviceItem::ThreadIndexCheckedTiled2D)
                || callee.checked_tiled_2d_evidence_v1().is_none()
            {
                continue;
            }
            if !destination.projection.is_empty()
                || !imported_local(function, destination.local)
                    .is_some_and(|local| is_standard_option_shape(&local.ty.shape))
            {
                return Err(
                    "authenticated checked_tiled_2d result is not an unprojected standard Option"
                        .to_owned(),
                );
            }
            plan.insert_carrier(
                destination.local,
                TryCarrier {
                    origin_option: destination.local,
                    kind: TryCarrierKind::Option,
                },
            )?;
            plan.insert_origin(destination.local, TryOrigin::Tiled2d)?;
        }

        plan.collect_checked_pointer_origins(function)?;

        let max_rounds = function.local_count.saturating_add(1);
        let mut converged = false;
        for _ in 0..max_rounds {
            let before = (plan.carriers.len(), plan.payload_values.len());
            for block in &function.blocks {
                for statement in &block.statements {
                    if statement.kind != MirStatementKind::Assign {
                        continue;
                    }
                    let Some(destination) = statement.destination.as_ref() else {
                        continue;
                    };
                    if !destination.projection.is_empty() {
                        continue;
                    }
                    match statement.rvalue {
                        Some(MirRvalueKind::Use) => {
                            let [MirOperandRef::Place(source)] = statement.operands.as_slice()
                            else {
                                continue;
                            };
                            if source.projection.is_empty()
                                && let Some(carrier) = plan.carriers.get(&source.local).copied()
                            {
                                let destination_local = imported_local(function, destination.local)
                                    .ok_or_else(|| {
                                        format!(
                                            "tiled try carrier destination local{} is not imported",
                                            destination.local
                                        )
                                    })?;
                                let exact_shape = match carrier.kind {
                                    TryCarrierKind::Option => {
                                        is_standard_option_shape(&destination_local.ty.shape)
                                    }
                                    TryCarrierKind::Result => {
                                        is_standard_result_shape(&destination_local.ty.shape)
                                    }
                                    TryCarrierKind::ControlFlow => {
                                        is_standard_control_flow_shape(&destination_local.ty.shape)
                                    }
                                };
                                if !exact_shape
                                    || destination.semantic_identity != source.semantic_identity
                                {
                                    return Err(format!(
                                        "tiled try carrier copy local{} does not preserve its exact compiler type identity",
                                        destination.local
                                    ));
                                }
                                plan.insert_carrier(destination.local, carrier)?;
                                continue;
                            }
                            let origin = match source.projection.as_slice() {
                                [] => plan.payload_values.get(&source.local).copied(),
                                [
                                    MirProjectionElem::Downcast { variant },
                                    MirProjectionElem::Field(0),
                                ] => plan.carriers.get(&source.local).and_then(|carrier| {
                                    (*variant == carrier.kind.success_variant())
                                        .then_some(carrier.origin_option)
                                }),
                                _ => None,
                            };
                            let Some(origin) = origin else {
                                continue;
                            };
                            imported_local(function, destination.local).ok_or_else(|| {
                                format!(
                                    "tiled try bridge destination local{} is not imported",
                                    destination.local
                                )
                            })?;
                            if destination.semantic_identity != source.semantic_identity {
                                return Err(format!(
                                    "tiled try bridge payload local{} does not preserve the exact compiler type identity",
                                    destination.local
                                ));
                            }
                            plan.insert_payload_value(destination.local, origin)?;
                        }
                        Some(MirRvalueKind::AdtAggregate {
                            variant: 0,
                            active_field,
                        }) if active_field.is_none() || active_field == Some(0) => {
                            let [MirOperandRef::Place(source)] = statement.operands.as_slice()
                            else {
                                continue;
                            };
                            if !source.projection.is_empty() {
                                continue;
                            }
                            let Some(origin) = plan.payload_values.get(&source.local).copied()
                            else {
                                continue;
                            };
                            let shape = &imported_local(function, destination.local)
                                .ok_or_else(|| {
                                    format!(
                                        "tiled try aggregate local{} is not imported",
                                        destination.local
                                    )
                                })?
                                .ty
                                .shape;
                            let kind = if is_standard_result_shape(shape) {
                                Some(TryCarrierKind::Result)
                            } else if is_standard_control_flow_shape(shape) {
                                Some(TryCarrierKind::ControlFlow)
                            } else {
                                None
                            };
                            if let Some(kind) = kind {
                                plan.insert_carrier(
                                    destination.local,
                                    TryCarrier {
                                        origin_option: origin,
                                        kind,
                                    },
                                )?;
                            }
                        }
                        _ => {}
                    }
                }
            }
            if before == (plan.carriers.len(), plan.payload_values.len()) {
                converged = true;
                break;
            }
        }
        if !converged {
            return Err("tiled try bridge analysis did not reach a bounded fixed point".to_owned());
        }

        let mut discriminants = BTreeMap::new();
        for block in &function.blocks {
            for statement in &block.statements {
                if statement.kind != MirStatementKind::Assign
                    || statement.rvalue != Some(MirRvalueKind::Discriminant)
                {
                    continue;
                }
                let (Some(destination), [MirOperandRef::Place(source)]) = (
                    statement.destination.as_ref(),
                    statement.operands.as_slice(),
                ) else {
                    continue;
                };
                if !destination.projection.is_empty() || !source.projection.is_empty() {
                    continue;
                }
                let Some(carrier) = plan.carriers.get(&source.local).copied() else {
                    continue;
                };
                if let Some(previous) = discriminants.insert(destination.local, source.local)
                    && previous != source.local
                {
                    return Err(format!(
                        "tiled try discriminant local{} has conflicting carrier origins",
                        destination.local
                    ));
                }
                if carrier.origin_option == source.local && carrier.kind != TryCarrierKind::Option {
                    return Err(format!(
                        "tiled try carrier local{} has an invalid self origin",
                        source.local
                    ));
                }
            }
        }
        for block in &function.blocks {
            let Some(MirTerminator {
                kind:
                    MirTerminatorKind::SwitchInt {
                        discriminant: MirOperandRef::Place(discriminant),
                        targets,
                        otherwise,
                    },
                ..
            }) = block.terminator.as_ref()
            else {
                continue;
            };
            if !discriminant.projection.is_empty() {
                continue;
            }
            let Some(carrier_local) = discriminants.get(&discriminant.local).copied() else {
                continue;
            };
            let carrier = plan.carriers[&carrier_local];
            let zero = targets.iter().find(|target| target.value == 0);
            let one = targets.iter().find(|target| target.value == 1);
            let exhaustive = targets.len() == 2
                && zero.is_some()
                && one.is_some()
                && zero.map(|target| target.target) != one.map(|target| target.target)
                && function.blocks.iter().any(|candidate| {
                    candidate.index == *otherwise
                        && matches!(
                            candidate
                                .terminator
                                .as_ref()
                                .map(|terminator| &terminator.kind),
                            Some(MirTerminatorKind::Unreachable)
                        )
                });
            if !exhaustive {
                return Err(format!(
                    "tiled try carrier local{carrier_local} does not use an exact 0/1 switch with unreachable default"
                ));
            }
            let success_entry = if carrier.kind.success_variant() == 0 {
                zero.expect("checked above").target
            } else {
                one.expect("checked above").target
            };
            if let Some(previous) = plan.success_entries.insert(carrier_local, success_entry)
                && previous != success_entry
            {
                return Err(format!(
                    "tiled try carrier local{carrier_local} has conflicting success entries"
                ));
            }
        }

        let mut converged = false;
        for _ in 0..max_rounds {
            let before = plan.payload_guard_entries.len();
            for block in &function.blocks {
                let mut locally_successful_carriers = BTreeMap::new();
                for statement in &block.statements {
                    if statement.kind != MirStatementKind::Assign {
                        continue;
                    }
                    let Some(destination) = statement.destination.as_ref() else {
                        continue;
                    };
                    if !destination.projection.is_empty() {
                        continue;
                    }
                    locally_successful_carriers.remove(&destination.local);
                    match statement.rvalue {
                        Some(MirRvalueKind::Use) => {
                            let [MirOperandRef::Place(source)] = statement.operands.as_slice()
                            else {
                                continue;
                            };
                            let Some(origin) = plan.payload_values.get(&destination.local).copied()
                            else {
                                continue;
                            };
                            let guard = match source.projection.as_slice() {
                                [] => plan.payload_guard_entries.get(&source.local).copied(),
                                [
                                    MirProjectionElem::Downcast { variant },
                                    MirProjectionElem::Field(0),
                                ] => {
                                    let Some(carrier) = plan.carriers.get(&source.local).copied()
                                    else {
                                        continue;
                                    };
                                    if *variant != carrier.kind.success_variant()
                                        || carrier.origin_option != origin
                                    {
                                        continue;
                                    }
                                    locally_successful_carriers
                                        .get(&source.local)
                                        .filter(|(local_origin, _)| *local_origin == origin)
                                        .map(|(_, guard)| *guard)
                                        .or_else(|| {
                                            plan.success_entries.get(&source.local).copied().filter(
                                                |guard| {
                                                    mir_block_dominates(
                                                        function,
                                                        *guard,
                                                        block.index,
                                                    )
                                                },
                                            )
                                        })
                                }
                                _ => None,
                            };
                            if let Some(guard) = guard {
                                plan.insert_payload_guard(destination.local, guard)?;
                            }
                        }
                        Some(MirRvalueKind::AdtAggregate {
                            variant,
                            active_field,
                        }) if active_field.is_none() || active_field == Some(0) => {
                            let Some(carrier) = plan.carriers.get(&destination.local).copied()
                            else {
                                continue;
                            };
                            let [MirOperandRef::Place(source)] = statement.operands.as_slice()
                            else {
                                continue;
                            };
                            if variant != carrier.kind.success_variant()
                                || !source.projection.is_empty()
                                || plan.payload_values.get(&source.local).copied()
                                    != Some(carrier.origin_option)
                            {
                                continue;
                            }
                            if let Some(guard) =
                                plan.payload_guard_entries.get(&source.local).copied()
                            {
                                locally_successful_carriers
                                    .insert(destination.local, (carrier.origin_option, guard));
                            }
                        }
                        _ => {}
                    }
                }
            }
            if before == plan.payload_guard_entries.len() {
                converged = true;
                break;
            }
        }
        if !converged {
            return Err(
                "tiled witness guard analysis did not reach a bounded fixed point".to_owned(),
            );
        }
        if let Some(local) = plan
            .payload_values
            .keys()
            .find(|local| !plan.payload_guard_entries.contains_key(local))
        {
            return Err(format!(
                "tiled witness local{local} has no compiler-authenticated success guard"
            ));
        }
        Ok(plan)
    }

    fn collect_checked_pointer_origins(&mut self, function: &MirFunction) -> Result<(), String> {
        let mut writes = BTreeMap::<usize, Vec<(usize, &MirStatement)>>::new();
        for block in &function.blocks {
            for statement in &block.statements {
                if statement.kind == MirStatementKind::Assign
                    && let Some(destination) = statement.destination.as_ref()
                    && destination.projection.is_empty()
                {
                    writes
                        .entry(destination.local)
                        .or_default()
                        .push((block.index, statement));
                }
            }
        }
        let blocks = function
            .blocks
            .iter()
            .map(|block| (block.index, block))
            .collect::<BTreeMap<_, _>>();

        for bounds_block in &function.blocks {
            let Some(MirTerminator {
                kind:
                    MirTerminatorKind::SwitchInt {
                        discriminant: MirOperandRef::Place(discriminant),
                        targets,
                        otherwise,
                    },
                ..
            }) = bounds_block.terminator.as_ref()
            else {
                continue;
            };
            if !discriminant.projection.is_empty()
                || targets.len() != 1
                || targets[0].value > 1
                || targets[0].target == *otherwise
                || !matches!(
                    imported_local(function, discriminant.local).map(|local| &local.ty.shape),
                    Some(MirTypeShape::Bool)
                )
            {
                continue;
            }
            let Some([(definition_block, bounds_statement)]) =
                writes.get(&discriminant.local).map(Vec::as_slice)
            else {
                continue;
            };
            if *definition_block != bounds_block.index
                || bounds_statement.rvalue != Some(MirRvalueKind::Binary(MirBinaryOp::Lt))
            {
                continue;
            }
            let [MirOperandRef::Place(index), MirOperandRef::Place(length)] =
                bounds_statement.operands.as_slice()
            else {
                continue;
            };
            if !index.projection.is_empty()
                || !length.projection.is_empty()
                || !matches!(
                    imported_local(function, index.local).map(|local| &local.ty.shape),
                    Some(MirTypeShape::USize)
                )
                || !matches!(
                    imported_local(function, length.local).map(|local| &local.ty.shape),
                    Some(MirTypeShape::USize)
                )
            {
                continue;
            }

            let explicit = blocks.get(&targets[0].target).copied().ok_or_else(|| {
                format!(
                    "bounds switch bb{} references missing bb{}",
                    bounds_block.index, targets[0].target
                )
            })?;
            let fallback = blocks.get(otherwise).copied().ok_or_else(|| {
                format!(
                    "bounds switch bb{} references missing bb{}",
                    bounds_block.index, otherwise
                )
            })?;
            let (some_block, none_block) = if targets[0].value == 0 {
                (fallback, explicit)
            } else {
                (explicit, fallback)
            };

            let some_candidates = some_block
                .statements
                .iter()
                .filter_map(|statement| {
                    let Some(MirRvalueKind::AdtAggregate {
                        variant: 1,
                        active_field,
                    }) = statement.rvalue
                    else {
                        return None;
                    };
                    if active_field.is_some_and(|field| field != 0) {
                        return None;
                    }
                    let destination = statement.destination.as_ref()?;
                    let [MirOperandRef::Place(pointer)] = statement.operands.as_slice() else {
                        return None;
                    };
                    (destination.projection.is_empty() && pointer.projection.is_empty())
                        .then_some((statement, destination, pointer))
                })
                .collect::<Vec<_>>();
            if some_candidates.is_empty() {
                continue;
            }
            if some_candidates.len() != 1 {
                return Err(format!(
                    "checked pointer success bb{} has more than one one-field Some construction",
                    some_block.index
                ));
            }
            let (some_statement, some_destination, some_pointer) = some_candidates[0];
            let option_local =
                imported_local(function, some_destination.local).ok_or_else(|| {
                    format!(
                        "checked pointer Option local{} is not imported",
                        some_destination.local
                    )
                })?;
            if !is_standard_option_shape(&option_local.ty.shape) {
                continue;
            }

            let none_candidates = none_block
                .statements
                .iter()
                .filter(|statement| {
                    statement.rvalue == Some(MirRvalueKind::FieldlessEnumVariant(0))
                        && statement.operands.is_empty()
                        && matches!(
                            statement.destination.as_ref(),
                            Some(destination)
                                if destination.local == some_destination.local
                                    && destination.projection.is_empty()
                        )
                })
                .collect::<Vec<_>>();
            if none_candidates.len() != 1 {
                let observed = none_block
                    .statements
                    .iter()
                    .filter(|statement| {
                        matches!(
                            statement.destination.as_ref(),
                            Some(destination) if destination.local == some_destination.local
                        )
                    })
                    .map(|statement| {
                        format!(
                            "rvalue={:?}, operands={:?}, semantic_type={:?}",
                            statement.rvalue, statement.operands, statement.semantic_rvalue_type
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(format!(
                    "checked pointer failure bb{} must construct the exact fieldless Option::None variant; observed {observed}",
                    none_block.index,
                ));
            }
            let none_statement = none_candidates[0];
            if writes
                .get(&some_destination.local)
                .is_none_or(|assignments| assignments.len() != 2)
            {
                return Err(format!(
                    "checked pointer Option local{} must have exactly its Some and None definitions",
                    some_destination.local
                ));
            }

            let Some(MirTerminatorKind::Goto { target: some_join }) = some_block
                .terminator
                .as_ref()
                .map(|terminator| &terminator.kind)
            else {
                return Err(format!(
                    "checked pointer success bb{} does not branch directly to its Option join",
                    some_block.index
                ));
            };
            let Some(MirTerminatorKind::Goto { target: none_join }) = none_block
                .terminator
                .as_ref()
                .map(|terminator| &terminator.kind)
            else {
                return Err(format!(
                    "checked pointer failure bb{} does not branch directly to its Option join",
                    none_block.index
                ));
            };
            if some_join != none_join {
                return Err(format!(
                    "checked pointer Option local{} has different Some and None joins",
                    some_destination.local
                ));
            }

            let pointer_definitions = writes
                .get(&some_pointer.local)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let [(pointer_block, pointer_statement)] = pointer_definitions else {
                return Err(format!(
                    "checked pointer payload local{} must have exactly one definition",
                    some_pointer.local
                ));
            };
            let Some(MirRvalueKind::Reference(crate::mir_import::MirBorrowKind::Shared)) =
                pointer_statement.rvalue
            else {
                return Err(format!(
                    "checked pointer payload local{} is not an exact shared reference",
                    some_pointer.local
                ));
            };
            let (Some(pointer_destination), [MirOperandRef::Place(indexed)]) = (
                pointer_statement.destination.as_ref(),
                pointer_statement.operands.as_slice(),
            ) else {
                return Err(format!(
                    "checked pointer payload local{} has malformed reference MIR",
                    some_pointer.local
                ));
            };
            let [
                MirProjectionElem::Deref,
                MirProjectionElem::Index {
                    local: pointer_index,
                },
            ] = indexed.projection.as_slice()
            else {
                return Err(format!(
                    "checked pointer payload local{} is not borrowed from one indexed slice element",
                    some_pointer.local
                ));
            };
            if *pointer_block != some_block.index
                || pointer_destination.local != some_pointer.local
                || !pointer_destination.projection.is_empty()
                || indexed.local == some_pointer.local
                || *pointer_index != index.local
            {
                return Err(format!(
                    "checked pointer payload local{} does not preserve its guarded slice/index origin",
                    some_pointer.local
                ));
            }

            let Some([(length_block, length_statement)]) =
                writes.get(&length.local).map(Vec::as_slice)
            else {
                return Err(format!(
                    "checked pointer length local{} must have exactly one definition",
                    length.local
                ));
            };
            let (Some(length_destination), [MirOperandRef::Place(length_slice)]) = (
                length_statement.destination.as_ref(),
                length_statement.operands.as_slice(),
            ) else {
                return Err(format!(
                    "checked pointer length local{} has malformed slice metadata MIR",
                    length.local
                ));
            };
            if length_statement.rvalue != Some(MirRvalueKind::Unary(MirUnaryOp::PtrMetadata))
                || !length_destination.projection.is_empty()
                || !length_slice.projection.is_empty()
                || length_slice.local != indexed.local
                || !mir_block_dominates(function, *length_block, bounds_block.index)
            {
                return Err(format!(
                    "checked pointer length local{} is not the dominating metadata of its exact source slice",
                    length.local
                ));
            }

            let base = imported_local(function, indexed.local).ok_or_else(|| {
                format!(
                    "checked pointer base local{} is not imported",
                    indexed.local
                )
            })?;
            let pointer = imported_local(function, some_pointer.local).ok_or_else(|| {
                format!(
                    "checked pointer payload local{} is not imported",
                    some_pointer.local
                )
            })?;
            let exact_types = matches!(
                (&base.ty.shape, &pointer.ty.shape),
                (
                    MirTypeShape::Slice { element, mutable: false },
                    MirTypeShape::Reference { pointee, mutable: false }
                ) if element == pointee
            ) && base.role == crate::mir_import::MirLocalRole::Arg;
            let exact_identities = some_destination.semantic_identity
                == option_local.ty.semantic_identity
                && some_statement.semantic_rvalue_type.as_ref()
                    == Some(&option_local.ty.semantic_identity)
                && none_statement
                    .destination
                    .as_ref()
                    .is_some_and(|destination| {
                        destination.semantic_identity == option_local.ty.semantic_identity
                    })
                && none_statement.semantic_rvalue_type.as_ref()
                    == Some(&option_local.ty.semantic_identity)
                && pointer_destination.semantic_identity == pointer.ty.semantic_identity
                && some_pointer.semantic_identity == pointer.ty.semantic_identity
                && length_destination.semantic_identity
                    == imported_local(function, length.local)
                        .expect("length local checked above")
                        .ty
                        .semantic_identity
                && length_slice.semantic_identity == base.ty.semantic_identity
                && index.semantic_identity
                    == imported_local(function, index.local)
                        .expect("index local checked above")
                        .ty
                        .semantic_identity
                && length.semantic_identity
                    == imported_local(function, length.local)
                        .expect("length local checked above")
                        .ty
                        .semantic_identity;
            if !exact_types || !exact_identities {
                return Err(format!(
                    "checked pointer Option local{} does not preserve exact slice, pointer, and compiler type identities",
                    some_destination.local
                ));
            }

            let origin = CheckedPointerOrigin {
                pointer_local: some_pointer.local,
                base_slice_local: indexed.local,
                index_local: index.local,
                length_local: length.local,
                bounds_local: discriminant.local,
                bounds_block: bounds_block.index,
                some_block: some_block.index,
                none_block: none_block.index,
                join_block: *some_join,
            };
            self.insert_origin(some_destination.local, TryOrigin::CheckedPointer(origin))?;
            self.insert_carrier(
                some_destination.local,
                TryCarrier {
                    origin_option: some_destination.local,
                    kind: TryCarrierKind::Option,
                },
            )?;
            self.insert_payload_value(some_pointer.local, some_destination.local)?;
            self.insert_payload_guard(some_pointer.local, some_block.index)?;
        }
        Ok(())
    }

    fn insert_carrier(&mut self, local: usize, carrier: TryCarrier) -> Result<(), String> {
        if let Some(previous) = self.carriers.insert(local, carrier)
            && previous != carrier
        {
            return Err(format!(
                "tiled try bridge local{local} has conflicting authenticated carrier origins"
            ));
        }
        Ok(())
    }

    fn insert_origin(&mut self, local: usize, origin: TryOrigin) -> Result<(), String> {
        if let Some(previous) = self.origins.insert(local, origin)
            && previous != origin
        {
            return Err(format!(
                "try bridge local{local} has conflicting authenticated origin kinds"
            ));
        }
        Ok(())
    }

    fn insert_payload_value(&mut self, local: usize, origin: usize) -> Result<(), String> {
        if let Some(previous) = self.payload_values.insert(local, origin)
            && previous != origin
        {
            return Err(format!(
                "tiled witness local{local} has conflicting authenticated origins"
            ));
        }
        Ok(())
    }

    fn insert_payload_guard(&mut self, local: usize, guard: usize) -> Result<(), String> {
        if let Some(previous) = self.payload_guard_entries.insert(local, guard)
            && previous != guard
        {
            return Err(format!(
                "tiled witness local{local} has conflicting authenticated success guards"
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct TryDiscriminant {
    carrier_local: usize,
    origin_option: usize,
    success_variant: usize,
}

struct FunctionLowerer<'function, 'declarations> {
    function: &'function MirFunction,
    kernel_context: Option<&'function MirFunction>,
    source_abi: &'function MirFunction,
    elides_generated_result: bool,
    sealed_generated_kernel_binding: Option<KernelBindingIdV1>,
    declarations: &'declarations mut BTreeMap<String, Signature>,
    internal_definitions:
        &'declarations BTreeMap<MirSemanticInstanceIdentity, InternalDefinitionContract>,
    locals: BTreeMap<usize, LocalBinding>,
    value_types: BTreeMap<ValueId, Type>,
    trusted_thread_indices: BTreeSet<ValueId>,
    trusted_disjoint_indices: BTreeSet<ValueId>,
    guarded_pointer_values: BTreeMap<ValueId, usize>,
    checked_pointer_candidates: BTreeMap<usize, ValueId>,
    tiled_error_origins: BTreeMap<usize, usize>,
    elided_tiled_error_values: BTreeSet<ValueId>,
    return_type: Option<Type>,
    next_value: u32,
    trap_block: Option<BlockId>,
    workgroup_size: Option<WorkgroupSize>,
    float_target: Option<Gfx942FloatTarget>,
    collective_target: Option<Gfx942WaveLdsTargetV2>,
    strict_float_policy: StrictFloatPolicy,
    control_flow_ssa: control_flow_ssa::ControlFlowSsaPlan,
    block_parameters: BTreeMap<usize, BTreeMap<usize, Vec<ValueId>>>,
    try_bridge: TryBridgePlan,
    try_discriminants: BTreeMap<ValueId, TryDiscriminant>,
    required_capabilities: BTreeSet<TargetCapability>,
}

impl<'function, 'declarations> FunctionLowerer<'function, 'declarations> {
    fn new(
        function: &'function MirFunction,
        kernel_context: Option<&'function MirFunction>,
        source_abi: &'function MirFunction,
        elides_generated_result: bool,
        sealed_generated_kernel_binding: Option<KernelBindingIdV1>,
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
            kernel_context,
            source_abi,
            elides_generated_result,
            sealed_generated_kernel_binding,
            declarations,
            internal_definitions,
            locals: BTreeMap::new(),
            value_types: BTreeMap::new(),
            trusted_thread_indices: BTreeSet::new(),
            trusted_disjoint_indices: BTreeSet::new(),
            guarded_pointer_values: BTreeMap::new(),
            checked_pointer_candidates: BTreeMap::new(),
            tiled_error_origins: BTreeMap::new(),
            elided_tiled_error_values: BTreeSet::new(),
            return_type: None,
            next_value: 0,
            trap_block: None,
            workgroup_size,
            float_target,
            collective_target,
            strict_float_policy,
            control_flow_ssa: control_flow_ssa::ControlFlowSsaPlan::default(),
            block_parameters: BTreeMap::new(),
            try_bridge: TryBridgePlan::default(),
            try_discriminants: BTreeMap::new(),
            required_capabilities: BTreeSet::new(),
        }
    }

    fn lower(mut self) -> Result<Function, TranslationDiagnostic> {
        self.try_bridge = TryBridgePlan::analyze(self.function).map_err(|reason| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                TranslationLocation::function(self.function),
                format!("authenticated tiled try bridge rejected: {reason}"),
            )
        })?;
        let signature = declared_function_signature(
            self.function,
            self.source_abi,
            self.elides_generated_result,
        )?;
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
                let types = self
                    .control_flow_ssa
                    .types(local)
                    .expect("live-in local is promoted")
                    .to_vec();
                let values = types
                    .into_iter()
                    .map(|ty| {
                        self.fresh_value(
                            ty,
                            &TranslationLocation::block(self.function, source_block),
                        )
                        .map(|parameter| parameter.id)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                parameters.insert(local, values);
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

        let lowering_order = mir_reverse_postorder(self.function)?;
        let mut lowered = BTreeMap::new();
        for source_block in lowering_order {
            lowered.insert(source_block.index, self.lower_block(source_block)?);
        }
        if !self.checked_pointer_candidates.is_empty() {
            let pointer_locals = self
                .checked_pointer_candidates
                .keys()
                .map(|local| format!("local{local}"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(self.function),
                format!(
                    "checked pointer candidates were not consumed by their authenticated success paths: {pointer_locals}"
                ),
            ));
        }
        let mut blocks =
            Vec::with_capacity(source_blocks.len() + usize::from(self.trap_block.is_some()));
        for source_block in source_blocks {
            blocks.push(lowered.remove(&source_block.index).ok_or_else(|| {
                diagnostic(
                    TranslationDiagnosticCode::MalformedMir,
                    TranslationLocation::block(self.function, source_block),
                    format!(
                        "deterministic reverse-postorder lowering omitted bb{}",
                        source_block.index
                    ),
                )
            })?);
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

    fn is_authenticated_general_v3_scalar_context(&self) -> bool {
        self.is_exact_general_v3_alpha_zeta_context()
            || (self.is_general_v3_profile_context()
                && self.sealed_generated_kernel_binding.is_some())
    }

    fn is_authenticated_vecadd_v2_scalar_context(&self) -> bool {
        let Some(context) = self.kernel_context else {
            return false;
        };
        if context.kind != MirFunctionKind::KernelEntry
            || context.typed_profile != Some(MirKernelProfile::VecAddRustcLayoutV2)
        {
            return false;
        }
        let mut arguments = context
            .locals
            .iter()
            .filter(|local| local.role == crate::mir_import::MirLocalRole::Arg)
            .collect::<Vec<_>>();
        arguments.sort_by_key(|local| local.index);
        matches!(
            arguments.as_slice(),
            [a, b, output]
                if is_readonly_f32_slice(&a.ty.shape)
                    && is_readonly_f32_slice(&b.ty.shape)
                    && is_disjoint_f32_slice(&output.ty.shape)
        )
    }

    fn is_authenticated_f32_scalar_context(&self) -> bool {
        self.is_authenticated_general_v3_scalar_context()
            || self.is_authenticated_vecadd_v2_scalar_context()
    }

    fn is_general_v3_profile_context(&self) -> bool {
        self.kernel_context.is_some_and(|context| {
            context.kind == MirFunctionKind::KernelEntry
                && context.typed_profile == Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3)
        })
    }
    fn is_gfx942_memory_v1_context(&self) -> bool {
        self.is_general_v3_profile_context() && self.float_target.is_some()
    }

    fn is_memory_v1_source_context(&self) -> bool {
        self.function.blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|terminator| &terminator.kind),
                Some(MirTerminatorKind::Call {
                    callee: Some(callee),
                    ..
                }) if matches!(
                    callee.trusted_item(),
                    Some(
                        TrustedDeviceItem::MemoryOffsetFrom
                            | TrustedDeviceItem::MemoryVolatileLoad
                            | TrustedDeviceItem::MemoryVolatileStore
                            | TrustedDeviceItem::MemoryCopyNonOverlapping
                            | TrustedDeviceItem::MemoryCopyOneNonOverlapping
                    )
                )
            )
        })
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
        self.kernel_context.is_some()
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
            for (&local, ids) in self
                .block_parameters
                .get(&source.index)
                .expect("non-entry block parameter map")
            {
                let types = self
                    .control_flow_ssa
                    .types(local)
                    .expect("block parameter local is promoted");
                for (&id, ty) in ids.iter().zip(types) {
                    block.parameters.push(ValueDef::new(id, ty.clone()));
                }
                let binding = match (self.control_flow_ssa.kind(local), ids.as_slice()) {
                    (Some(control_flow_ssa::PromotedLocalKind::Scalar), [id]) => {
                        LocalBinding::Value(*id)
                    }
                    (Some(control_flow_ssa::PromotedLocalKind::FieldlessEnum), [id]) => {
                        LocalBinding::FieldlessEnum { discriminant: *id }
                    }
                    (
                        Some(control_flow_ssa::PromotedLocalKind::F32AccumulatorFragment),
                        [v0, v1, v2, v3],
                    ) => LocalBinding::F32AccumulatorFragment([*v0, *v1, *v2, *v3]),
                    (None, _) => unreachable!("block parameter local is promoted"),
                    _ => unreachable!("promoted local value arity matches its kind"),
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
        if self.elides_generated_result
            && statement
                .destination
                .as_ref()
                .is_some_and(|destination| destination.local == 0)
        {
            let return_local = imported_local(self.function, 0).ok_or_else(|| {
                diagnostic(
                    TranslationDiagnosticCode::MalformedMir,
                    location.clone(),
                    "generated kernel body has no return local0",
                )
            })?;
            if statement.kind == MirStatementKind::Assign
                && statement
                    .destination
                    .as_ref()
                    .is_some_and(|destination| destination.projection.is_empty())
                && is_exact_discarded_result_rvalue(self.function, statement, return_local)
            {
                return Ok(());
            }
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location,
                "generated kernel body return write escaped its authenticated Result-elision contract",
            ));
        }
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
                if self.elides_generated_result
                    && statement.destination.is_none()
                    && statement.rvalue.is_none()
                    && statement.semantic_rvalue_type.is_none()
                    && matches!(statement.operands.as_slice(), [MirOperandRef::Place(place)]
                        if place.projection.is_empty()
                            && matches!(self.locals.get(&place.local), Some(LocalBinding::Value(value))
                                if self.elided_tiled_error_values.contains(value)))
                {
                    return Ok(());
                }
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
                if let Some(binding) = self.tiled_witness_binding(place.local, &location)? {
                    let exact_value = place.projection.is_empty()
                        && self.try_bridge.payload_values.contains_key(&place.local);
                    let exact_reference = destination.projection.is_empty()
                        && matches!(
                            self.imported_local_shape(destination.local),
                            Some(MirTypeShape::Reference { mutable: false, .. })
                        );
                    if !exact_value || !exact_reference {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location,
                            "tiled witness autoref requires an exact unprojected DisjointTile2D source and shared reference destination",
                        ));
                    }
                    return self.bind_local(destination.local, binding, location);
                }
                if let Some((origin_option, origin)) =
                    self.try_bridge
                        .origins
                        .iter()
                        .find_map(|(option, origin)| match origin {
                            TryOrigin::CheckedPointer(origin)
                                if origin.pointer_local == destination.local =>
                            {
                                Some((*option, *origin))
                            }
                            TryOrigin::Tiled2d | TryOrigin::CheckedPointer(_) => None,
                        })
                {
                    let exact_place = matches!(
                        place.projection.as_slice(),
                        [MirProjectionElem::Deref, MirProjectionElem::Index { local }]
                            if place.local == origin.base_slice_local
                                && *local == origin.index_local
                    );
                    if !exact_place
                        || !destination.projection.is_empty()
                        || location.block != Some(origin.some_block)
                        || self
                            .try_bridge
                            .payload_values
                            .get(&destination.local)
                            .copied()
                            != Some(origin_option)
                    {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedProjection,
                            location,
                            "checked pointer reference does not preserve its exact authenticated slice, index, and bounds-success block",
                        ));
                    }
                    let pointer = self
                        .checked_pointer_candidates
                        .remove(&destination.local)
                        .ok_or_else(|| {
                            diagnostic(
                                TranslationDiagnosticCode::MalformedMir,
                                location.clone(),
                                "checked pointer candidate was not materialized in its authenticated bounds block",
                            )
                        })?;
                    self.guarded_pointer_values
                        .insert(pointer, origin.some_block);
                    return self.bind_plain_destination(destination, pointer, location);
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
                if self.lower_try_failure_use(destination, operand, &location)? {
                    return Ok(());
                }
                if self.lower_try_carrier_use(destination, operand, &location)? {
                    return Ok(());
                }
                if self.lower_checked_pointer_use(destination, operand, &location)? {
                    return Ok(());
                }
                if self.lower_tiled_witness_use(destination, operand, &location)? {
                    return Ok(());
                }
                if self.lower_matrix_aggregate_use(destination, operand, &location)? {
                    return Ok(());
                }
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
                if let Some(carrier) = self.try_bridge.carriers.get(&place.local).copied() {
                    let present = self.try_option_binding(carrier.origin_option, &location)?.0;
                    let discriminant = if carrier.kind == TryCarrierKind::Option {
                        present
                    } else {
                        self.emit_result(
                            block,
                            Type::BOOL,
                            OperationKind::Unary {
                                op: UnaryOp::Not,
                                operand: present,
                            },
                            &location,
                        )?
                    };
                    self.try_discriminants.insert(
                        discriminant,
                        TryDiscriminant {
                            carrier_local: place.local,
                            origin_option: carrier.origin_option,
                            success_variant: carrier.kind.success_variant(),
                        },
                    );
                    return self.bind_plain_destination(destination, discriminant, location);
                }
                if self.elides_generated_result
                    && self.tiled_error_origins.contains_key(&place.local)
                    && is_standard_result_shape(self.local_shape(place.local, &location)?)
                {
                    let discriminant = self.emit_result(
                        block,
                        Type::Scalar(ScalarType::I64),
                        OperationKind::Constant(Constant::I64(1)),
                        &location,
                    )?;
                    self.elided_tiled_error_values.insert(discriminant);
                    return self.bind_plain_destination(destination, discriminant, location);
                }
                let binding = self
                    .locals
                    .get(&place.local)
                    .copied()
                    .ok_or_else(|| self.undefined_local(place.local, location.clone()))?;
                let discriminant = match (binding.option_discriminant(), binding) {
                    (Some(discriminant), _) => discriminant,
                    (None, LocalBinding::FieldlessEnum { discriminant }) => discriminant,
                    (
                        None,
                        LocalBinding::Value(_)
                        | LocalBinding::OptionPointer { .. }
                        | LocalBinding::OptionTiled2dWitness { .. }
                        | LocalBinding::Tiled2dWitness { .. }
                        | LocalBinding::DeviceMathCapability
                        | LocalBinding::Gfx942CollectiveCapability
                        | LocalBinding::Gfx942StaticLdsU32x256(_)
                        | LocalBinding::DeviceMatrixValueCapability
                        | LocalBinding::DeviceMatrixReferenceCapability
                        | LocalBinding::Bf16MfmaFragment(_)
                        | LocalBinding::F32AccumulatorFragment(_)
                        | LocalBinding::FixedArray4(_),
                    ) => {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location,
                            "discriminant operand is not a translated Option result or authenticated fieldless enum",
                        ));
                    }
                };
                self.bind_plain_destination(destination, discriminant, location)
            }
            MirRvalueKind::AdtAggregate {
                variant,
                active_field,
            } if self.try_bridge.carriers.contains_key(&destination.local) => self
                .lower_try_aggregate(
                    destination,
                    variant,
                    active_field,
                    &statement.operands,
                    &statement.semantic_rvalue_type,
                    &location,
                ),
            MirRvalueKind::AdtAggregate {
                variant,
                active_field,
            } => {
                if self.lower_try_error_aggregate(
                    destination,
                    variant,
                    active_field,
                    &statement.operands,
                    &statement.semantic_rvalue_type,
                    &location,
                )? {
                    Ok(())
                } else {
                    Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedRvalue,
                        location,
                        format!(
                            "unsupported structured MIR rvalue: {:?}",
                            MirRvalueKind::AdtAggregate {
                                variant,
                                active_field,
                            }
                        ),
                    ))
                }
            }
            MirRvalueKind::FieldlessEnumVariant(0)
                if matches!(
                    self.try_bridge.origins.get(&destination.local),
                    Some(TryOrigin::CheckedPointer(_))
                ) =>
            {
                self.validate_checked_pointer_none(
                    destination,
                    &statement.operands,
                    &statement.semantic_rvalue_type,
                    &location,
                )
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
            MirRvalueKind::Repeat { count } => {
                let [operand] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "repeat rvalue must have exactly one element operand",
                    ));
                };
                if !destination.projection.is_empty()
                    || count != Some(4)
                    || !matches!(
                        self.local_shape(destination.local, &location)?,
                        MirTypeShape::Array { element, length: Some(4) }
                            if element.as_ref() == &MirTypeShape::F32
                    )
                    || !self.is_authenticated_general_v3_scalar_context()
                    || !self.is_exact_gfx942_wave64_matrix_context()
                {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedRvalue,
                        location,
                        "repeat lowering requires an exact authenticated `[f32; 4]` matrix-fragment initializer in a gfx942 one-wave General V3 kernel",
                    ));
                }
                self.require_strict_float_policy(&location)?;
                let value = self.lower_operand(operand, block, &location)?;
                if self.value_type(value, &location)? != &Type::F32 {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location,
                        "`[f32; 4]` repeat initializer does not contain an exact f32 value",
                    ));
                }
                self.bind_local(
                    destination.local,
                    LocalBinding::FixedArray4([value; 4]),
                    location,
                )
            }
            MirRvalueKind::ArrayAggregate { element_count } => {
                if !destination.projection.is_empty()
                    || element_count != 4
                    || statement.operands.len() != element_count
                    || !self.is_authenticated_general_v3_scalar_context()
                    || !self.is_exact_gfx942_wave64_matrix_context()
                {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedRvalue,
                        location,
                        "array aggregate lowering requires an exact authenticated four-element matrix-fragment array in a gfx942 one-wave General V3 kernel",
                    ));
                }
                let element_shape = match self.local_shape(destination.local, &location)? {
                    MirTypeShape::Array {
                        element,
                        length: Some(4),
                    } if matches!(element.as_ref(), MirTypeShape::U16 | MirTypeShape::F32) => {
                        element.as_ref().clone()
                    }
                    _ => {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location,
                            "matrix-fragment array aggregate must have exact `[u16; 4]` or `[f32; 4]` type",
                        ));
                    }
                };
                if element_shape == MirTypeShape::F32 {
                    self.require_strict_float_policy(&location)?;
                }
                let expected_type =
                    lower_scalar_type(&element_shape).expect("admitted scalar shape");
                let mut values = Vec::with_capacity(4);
                for operand in &statement.operands {
                    let value = self.lower_operand(operand, block, &location)?;
                    if self.value_type(value, &location)? != &expected_type {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location.clone(),
                            format!(
                                "matrix-fragment array element has type {:?}, expected {expected_type:?}",
                                self.value_type(value, &location)?
                            ),
                        ));
                    }
                    values.push(value);
                }
                let values: [ValueId; 4] = values.try_into().map_err(|_| {
                    diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location.clone(),
                        "authenticated four-element array did not lower to four values",
                    )
                })?;
                self.bind_local(
                    destination.local,
                    LocalBinding::FixedArray4(values),
                    location,
                )
            }
            MirRvalueKind::SemanticCast(MirCastKind::IntToInt) => {
                let [operand] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "integer cast must have one operand",
                    ));
                };
                if !destination.projection.is_empty() {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedProjection,
                        location,
                        "integer cast destination must be an unprojected scalar local",
                    ));
                }
                let value = self.lower_operand(operand, block, &location)?;
                let from = self.value_type(value, &location)?.clone();
                let to = lower_scalar_type(self.local_shape(destination.local, &location)?)
                    .ok_or_else(|| {
                        diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location.clone(),
                            format!(
                                "integer cast destination local{} is not a supported scalar",
                                destination.local
                            ),
                        )
                    })?;
                let (Some(from_scalar), Some(to_scalar)) = (from.as_scalar(), to.as_scalar())
                else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location,
                        format!("integer cast requires scalar types; found {from:?} to {to:?}"),
                    ));
                };
                let from_is_integer = from_scalar.is_integer() || from_scalar == ScalarType::Bool;
                if !from_is_integer || !to_scalar.is_integer() {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location,
                        format!("IntToInt cast does not accept {from:?} to {to:?}"),
                    ));
                }
                if from == to {
                    return self.assign_value(destination, value, block, location);
                }
                let width = |scalar: ScalarType| scalar.bit_width().unwrap_or(64);
                let cast = match width(from_scalar).cmp(&width(to_scalar)) {
                    std::cmp::Ordering::Greater => CastKind::Truncate,
                    std::cmp::Ordering::Less if from_scalar.is_signed_integer() => {
                        CastKind::SignExtend
                    }
                    std::cmp::Ordering::Less => CastKind::ZeroExtend,
                    std::cmp::Ordering::Equal => CastKind::Bitcast,
                };
                let result = self.emit_result(
                    block,
                    to.clone(),
                    OperationKind::Cast {
                        kind: cast,
                        value,
                        to,
                    },
                    &location,
                )?;
                self.assign_value(destination, result, block, location)
            }
            MirRvalueKind::Binary(
                arithmetic @ (MirBinaryOp::Add
                | MirBinaryOp::Sub
                | MirBinaryOp::Mul
                | MirBinaryOp::Div
                | MirBinaryOp::Rem
                | MirBinaryOp::BitXor
                | MirBinaryOp::BitAnd
                | MirBinaryOp::BitOr
                | MirBinaryOp::Shl
                | MirBinaryOp::Shr),
            ) => {
                let [lhs, rhs] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "binary operation must have two operands",
                    ));
                };
                let lhs = self.lower_operand(lhs, block, &location)?;
                let rhs = self.lower_operand(rhs, block, &location)?;
                let lhs_ty = self.value_type(lhs, &location)?.clone();
                let rhs_ty = self.value_type(rhs, &location)?.clone();
                let operation = match arithmetic {
                    MirBinaryOp::Add => BinaryOp::Add,
                    MirBinaryOp::Sub => BinaryOp::Subtract,
                    MirBinaryOp::Mul => BinaryOp::Multiply,
                    MirBinaryOp::Div => BinaryOp::Divide,
                    MirBinaryOp::Rem => BinaryOp::Remainder,
                    MirBinaryOp::BitXor => BinaryOp::BitXor,
                    MirBinaryOp::BitAnd => BinaryOp::BitAnd,
                    MirBinaryOp::BitOr => BinaryOp::BitOr,
                    MirBinaryOp::Shl => BinaryOp::ShiftLeft,
                    MirBinaryOp::Shr => BinaryOp::ShiftRight,
                    _ => unreachable!("binary arm admits only ordinary operations"),
                };
                let lhs_scalar = lhs_ty.as_scalar();
                let rhs_scalar = rhs_ty.as_scalar();
                let valid = match operation {
                    BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                        lhs_scalar.is_some_and(ScalarType::is_integer)
                            && rhs_scalar.is_some_and(ScalarType::is_integer)
                    }
                    BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                        lhs_ty == rhs_ty
                            && lhs_scalar.is_some_and(|scalar| {
                                scalar == ScalarType::Bool || scalar.is_integer()
                            })
                    }
                    _ => lhs_ty == rhs_ty && lhs_scalar.is_some_and(ScalarType::is_numeric),
                };
                if !valid {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location,
                        format!("binary {arithmetic:?} does not accept {lhs_ty:?} and {rhs_ty:?}"),
                    ));
                }
                if lhs_scalar.is_some_and(ScalarType::is_float) {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedRvalue,
                        location,
                        "floating-point binary operation requires an authenticated semantic workload handler",
                    ));
                }
                let result = self.emit_result(
                    block,
                    lhs_ty,
                    OperationKind::Binary {
                        op: operation,
                        lhs,
                        rhs,
                    },
                    &location,
                )?;
                self.assign_value(destination, result, block, location)
            }
            MirRvalueKind::Binary(
                comparison @ (MirBinaryOp::Eq
                | MirBinaryOp::Ne
                | MirBinaryOp::Lt
                | MirBinaryOp::Le
                | MirBinaryOp::Gt
                | MirBinaryOp::Ge),
            ) => {
                let [lhs, rhs] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "comparison must have two operands",
                    ));
                };
                let checked_pointer_origin = if comparison == MirBinaryOp::Lt {
                    let origins = self
                        .try_bridge
                        .origins
                        .values()
                        .filter_map(|origin| match origin {
                            TryOrigin::CheckedPointer(origin)
                                if origin.bounds_local == destination.local =>
                            {
                                Some(*origin)
                            }
                            TryOrigin::Tiled2d | TryOrigin::CheckedPointer(_) => None,
                        })
                        .collect::<Vec<_>>();
                    match origins.as_slice() {
                        [] => None,
                        [origin]
                            if location.block == Some(origin.bounds_block)
                                && matches!(lhs, MirOperandRef::Place(place)
                                    if place.projection.is_empty()
                                        && place.local == origin.index_local)
                                && matches!(rhs, MirOperandRef::Place(place)
                                    if place.projection.is_empty()
                                        && place.local == origin.length_local) =>
                        {
                            Some(*origin)
                        }
                        [_] => {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::MalformedMir,
                                location,
                                "checked pointer bounds comparison changed after origin authentication",
                            ));
                        }
                        _ => {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::MalformedMir,
                                location,
                                "more than one checked pointer origin claims the same bounds local",
                            ));
                        }
                    }
                } else {
                    None
                };
                let elided_error_operand =
                    |operand: &MirOperandRef,
                     locals: &BTreeMap<usize, LocalBinding>,
                     values: &BTreeSet<ValueId>| {
                        matches!(operand, MirOperandRef::Place(place)
                        if place.projection.is_empty()
                            && matches!(locals.get(&place.local), Some(LocalBinding::Value(value))
                                if values.contains(value)))
                    };
                let lhs_elided =
                    elided_error_operand(lhs, &self.locals, &self.elided_tiled_error_values);
                let rhs_elided =
                    elided_error_operand(rhs, &self.locals, &self.elided_tiled_error_values);
                let is_err_discriminant = |operand: &MirOperandRef| {
                    matches!(
                        operand,
                        MirOperandRef::Constant {
                            literal: MirConstant::I64(1) | MirConstant::ISize(1),
                            ..
                        }
                    )
                };
                let elided_error_comparison = comparison == MirBinaryOp::Eq
                    && ((lhs_elided && !rhs_elided && is_err_discriminant(rhs))
                        || (rhs_elided && !lhs_elided && is_err_discriminant(lhs)));
                if (lhs_elided || rhs_elided) && !elided_error_comparison {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedRvalue,
                        location,
                        "elided tiled error discriminant escapes its exact rustc Err validity check",
                    ));
                }
                let lhs = self.lower_operand(lhs, block, &location)?;
                let rhs = self.lower_operand(rhs, block, &location)?;
                let lhs_ty = self.value_type(lhs, &location)?.clone();
                let rhs_ty = self.value_type(rhs, &location)?.clone();
                let predicate = match comparison {
                    MirBinaryOp::Eq => ComparePredicate::Equal,
                    MirBinaryOp::Ne => ComparePredicate::NotEqual,
                    MirBinaryOp::Lt => ComparePredicate::LessThan,
                    MirBinaryOp::Le => ComparePredicate::LessThanOrEqual,
                    MirBinaryOp::Gt => ComparePredicate::GreaterThan,
                    MirBinaryOp::Ge => ComparePredicate::GreaterThanOrEqual,
                    _ => unreachable!("comparison arm admits only six predicates"),
                };
                let comparable = lhs_ty == rhs_ty
                    && lhs_ty.as_scalar().is_some_and(|scalar| {
                        scalar.is_numeric()
                            || (scalar == ScalarType::Bool
                                && matches!(
                                    predicate,
                                    ComparePredicate::Equal | ComparePredicate::NotEqual
                                ))
                    });
                if !comparable {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location,
                        format!(
                            "comparison {comparison:?} does not accept {lhs_ty:?} and {rhs_ty:?}"
                        ),
                    ));
                }
                let result = self.emit_result(
                    block,
                    Type::BOOL,
                    OperationKind::Compare {
                        predicate,
                        lhs,
                        rhs,
                    },
                    &location,
                )?;
                if let Some(origin) = checked_pointer_origin {
                    let pointer = self.indexed_pointer(
                        origin.base_slice_local,
                        origin.index_local,
                        block,
                        &location,
                    )?;
                    if let Some(previous) = self
                        .checked_pointer_candidates
                        .insert(origin.pointer_local, pointer)
                        && previous != pointer
                    {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::MalformedMir,
                            location,
                            "checked pointer candidate was materialized more than once",
                        ));
                    }
                }
                if elided_error_comparison {
                    self.elided_tiled_error_values.insert(result);
                }
                self.bind_plain_destination(destination, result, location)
            }
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
                if let Some(tiled) = self.try_discriminants.get(&selector).copied() {
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
                            "authenticated tiled try switch must have exact 0/1 cases and an unreachable default",
                        ));
                    }
                    let success_target = if tiled.success_variant == 0 {
                        zero.expect("checked above").target
                    } else {
                        one.expect("checked above").target
                    };
                    let failure_target = if tiled.success_variant == 0 {
                        one.expect("checked above").target
                    } else {
                        zero.expect("checked above").target
                    };
                    if self
                        .try_bridge
                        .success_entries
                        .get(&tiled.carrier_local)
                        .copied()
                        != Some(success_target)
                    {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedStatement,
                            location,
                            "tiled try switch disagrees with its compiler-authenticated carrier success entry",
                        ));
                    }
                    let (present, some_entry) =
                        self.try_option_binding(tiled.origin_option, &location)?;
                    if tiled.success_variant == 1 {
                        if !mir_block_dominates(self.function, block_index, success_target) {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::UnsupportedStatement,
                                location,
                                "authenticated tiled Option Some edge is not dominated by its presence predicate",
                            ));
                        }
                        let guarded = self.locals[&tiled.origin_option]
                            .with_option_some_entry(success_target)
                            .expect("tiled origin is an Option binding");
                        self.locals.insert(tiled.origin_option, guarded);
                    } else {
                        let Some(some_entry) = some_entry else {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::UnsupportedStatement,
                                location,
                                "tiled Result/ControlFlow switch precedes its authenticated Option Some guard",
                            ));
                        };
                        if self
                            .try_bridge
                            .success_entries
                            .get(&tiled.origin_option)
                            .copied()
                            != Some(some_entry)
                        {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::UnsupportedStatement,
                                location,
                                "tiled Result/ControlFlow switch is not linked to the authenticated Option success entry",
                            ));
                        }
                    }
                    return Ok(Terminator::ConditionalBranch {
                        condition: present,
                        then_target: self.block_id(success_target, location.clone())?,
                        then_arguments: self.edge_arguments(success_target, &location)?,
                        else_target: self.block_id(failure_target, location.clone())?,
                        else_arguments: self.edge_arguments(failure_target, &location)?,
                    });
                }
                if self.is_authenticated_general_v3_scalar_context()
                    && self.value_type(selector, &location)? == &Type::BOOL
                {
                    let option_locals = self
                        .locals
                        .iter()
                        .filter_map(|(local, binding)| {
                            (binding.option_discriminant() == Some(selector)).then_some(*local)
                        })
                        .collect::<Vec<_>>();
                    if option_locals.len() > 1 {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedStatement,
                            location,
                            "boolean Option switch is bound to more than one translated semantic result",
                        ));
                    }
                    let (then_target, else_target) = if let Some(&option_local) =
                        option_locals.first()
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
                                        block
                                            .terminator
                                            .as_ref()
                                            .map(|terminator| &terminator.kind),
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
                        let guarded = self.locals[&option_local]
                            .with_option_some_entry(some_entry)
                            .expect("selected an Option binding above");
                        self.locals.insert(option_local, guarded);
                        (some_entry, zero.expect("checked above").target)
                    } else {
                        let [explicit] = targets.as_slice() else {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::UnsupportedStatement,
                                location,
                                "ordinary boolean switch must have one explicit 0/1 case and a distinct default",
                            ));
                        };
                        if explicit.target == *otherwise || explicit.value > 1 {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::UnsupportedStatement,
                                location,
                                "ordinary boolean switch must have one explicit 0/1 case and a distinct default",
                            ));
                        }
                        if explicit.value == 0 {
                            (*otherwise, explicit.target)
                        } else {
                            (explicit.target, *otherwise)
                        }
                    };
                    return Ok(Terminator::ConditionalBranch {
                        condition: selector,
                        then_target: self.block_id(then_target, location.clone())?,
                        then_arguments: self.edge_arguments(then_target, &location)?,
                        else_target: self.block_id(else_target, location.clone())?,
                        else_arguments: self.edge_arguments(else_target, &location)?,
                    });
                }
                if self.value_type(selector, &location)? == &Type::BOOL {
                    let mut false_target = None;
                    let mut true_target = None;
                    for target in targets {
                        let selected = match target.value {
                            0 => &mut false_target,
                            1 => &mut true_target,
                            value => {
                                return Err(diagnostic(
                                    TranslationDiagnosticCode::UnsupportedStatement,
                                    location,
                                    format!(
                                        "boolean switch contains non-boolean case value {value}"
                                    ),
                                ));
                            }
                        };
                        if selected.replace(target.target).is_some() {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::MalformedMir,
                                location,
                                format!(
                                    "boolean switch contains duplicate case value {}",
                                    target.value
                                ),
                            ));
                        }
                    }
                    let default_is_unreachable = self.function.blocks.iter().any(|block| {
                        block.index == *otherwise
                            && matches!(
                                block.terminator.as_ref().map(|terminator| &terminator.kind),
                                Some(MirTerminatorKind::Unreachable)
                            )
                    });
                    let (false_target, true_target) = match (false_target, true_target) {
                        (Some(false_target), None) => (false_target, *otherwise),
                        (None, Some(true_target)) => (*otherwise, true_target),
                        (Some(false_target), Some(true_target)) if default_is_unreachable => {
                            (false_target, true_target)
                        }
                        _ => {
                            return Err(diagnostic(
                                TranslationDiagnosticCode::UnsupportedStatement,
                                location,
                                "boolean switch must contain one explicit 0/1 case, or exact 0/1 cases with an unreachable default",
                            ));
                        }
                    };
                    if false_target == true_target {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::MalformedMir,
                            location,
                            "boolean switch true and false edges must be distinct",
                        ));
                    }
                    return Ok(Terminator::ConditionalBranch {
                        condition: selector,
                        then_target: self.block_id(true_target, location.clone())?,
                        then_arguments: self.edge_arguments(true_target, &location)?,
                        else_target: self.block_id(false_target, location.clone())?,
                        else_arguments: self.edge_arguments(false_target, &location)?,
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
                    TrustedDeviceItem::KernelError
                    | TrustedDeviceItem::DisjointSlice
                    | TrustedDeviceItem::DeviceGlobalMutPtr
                    | TrustedDeviceItem::WorkgroupLdsScope
                    | TrustedDeviceItem::Invocation3D
                    | TrustedDeviceItem::ThreadIndex
                    | TrustedDeviceItem::DisjointIndex
                    | TrustedDeviceItem::ShiftedIndexSpace
                    | TrustedDeviceItem::BlockedIndexSpace
                    | TrustedDeviceItem::Tiled2DIndexSpace
                    | TrustedDeviceItem::GridExclusiveIndexSpace
                    | TrustedDeviceItem::DisjointBlock
                    | TrustedDeviceItem::DisjointTile2D
                    | TrustedDeviceItem::GridLeader
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
                    | TrustedDeviceItem::WaveLaneCurrent
                    | TrustedDeviceItem::Gfx942LdsBf16TilePairM16x16
                    | TrustedDeviceItem::Gfx942LdsBf16TilePairPublishM16x16
                    | TrustedDeviceItem::LdsTile16x16WriteMfmaBf16
                    | TrustedDeviceItem::LdsTile16x16ReadMfmaBf16
                    | TrustedDeviceItem::WorkgroupSyncthreads
                    | TrustedDeviceItem::WorkgroupLdsScopeCurrent
                    | TrustedDeviceItem::DynamicLdsExactCurrent
                    | TrustedDeviceItem::Invocation3DCurrent
                    | TrustedDeviceItem::ThreadIndexX
                    | TrustedDeviceItem::ThreadIndexY
                    | TrustedDeviceItem::ThreadIndexZ
                    | TrustedDeviceItem::WorkgroupIndexX
                    | TrustedDeviceItem::WorkgroupIndexY
                    | TrustedDeviceItem::WorkgroupIndexZ
                    | TrustedDeviceItem::WorkgroupDimensionX
                    | TrustedDeviceItem::WorkgroupDimensionY
                    | TrustedDeviceItem::WorkgroupDimensionZ
                    | TrustedDeviceItem::GridDimensionX
                    | TrustedDeviceItem::GridDimensionY
                    | TrustedDeviceItem::GridDimensionZ
                    | TrustedDeviceItem::DisjointSliceLen
                    | TrustedDeviceItem::ThreadIndexIntoDisjoint
                    | TrustedDeviceItem::ThreadIndexCheckedShift
                    | TrustedDeviceItem::DisjointIndexGet
                    | TrustedDeviceItem::DisjointIndexCheckedShift
                    | TrustedDeviceItem::GridLeaderCurrent
                    | TrustedDeviceItem::DisjointSliceGetDisjointMut
                    | TrustedDeviceItem::DisjointSliceGetMutExclusive
                    | TrustedDeviceItem::ThreadIndexCheckedBlock
                    | TrustedDeviceItem::ThreadIndexCheckedTiled2D
                    | TrustedDeviceItem::DisjointBlockComponentIndex
                    | TrustedDeviceItem::DisjointSliceGetBlockMut
                    | TrustedDeviceItem::DisjointSliceGetTiled2DMut
                    | TrustedDeviceItem::DeviceGlobalMutPtrU32AsAtomic
                    | TrustedDeviceItem::DeviceGlobalMutPtrI32AsAtomic
                    | TrustedDeviceItem::DeviceGlobalMutPtrU64AsAtomic
                    | TrustedDeviceItem::DeviceGlobalMutPtrI64AsAtomic,
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
                    | TrustedDeviceItem::MemoryCopyNonOverlapping
                    | TrustedDeviceItem::MemoryCopyOneNonOverlapping,
                ) => {
                    unreachable!("memory operations are handled by semantic lowering")
                }
                Some(
                    TrustedDeviceItem::Gfx942CollectivesCurrent
                    | TrustedDeviceItem::Gfx942SubgroupReduceSumF32
                    | TrustedDeviceItem::Gfx942SubgroupReduceMaxF32
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
                    | TrustedDeviceItem::DeviceMatrixCurrent
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
            [] if external_import.is_some() || internal_definition.is_some() => {
                if internal_definition
                    .as_ref()
                    .is_some_and(|definition| definition.elides_generated_result)
                    && !matches!(
                        self.imported_local_shape(destination.local),
                        Some(shape) if is_standard_result_shape(shape)
                    )
                {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "generated zero-result call does not target its authenticated discarded Result temporary",
                    ));
                }
            }
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
    ) -> Result<Option<MatrixFrontendBindingV2>, TranslationDiagnostic> {
        validate_matrix_frontend_function_abi(self.source_abi)?;
        let Some(evidence) = self.source_abi.matrix_frontend_abi.as_ref() else {
            return Ok(None);
        };
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
        Ok(Some(binding))
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

    fn lower_tiled_witness_use(
        &mut self,
        destination: &MirPlaceRef,
        operand: &MirOperandRef,
        location: &TranslationLocation,
    ) -> Result<bool, TranslationDiagnostic> {
        let MirOperandRef::Place(source) = operand else {
            return Ok(false);
        };
        if destination.projection.is_empty()
            && destination.semantic_identity == source.semantic_identity
            && let Some(binding) = self.tiled_witness_binding(destination.local, location)?
        {
            self.bind_local(destination.local, binding, location.clone())?;
            return Ok(true);
        }
        let binding = if source.projection.is_empty() {
            match self.locals.get(&source.local).copied() {
                Some(binding @ LocalBinding::Tiled2dWitness { .. }) => Some(binding),
                _ => None,
            }
        } else if let [
            MirProjectionElem::Downcast { variant },
            MirProjectionElem::Field(0),
        ] = source.projection.as_slice()
        {
            let Some(carrier) = self.try_bridge.carriers.get(&source.local).copied() else {
                return Ok(false);
            };
            if *variant != carrier.kind.success_variant() {
                return Ok(false);
            }
            let (_, raw, evidence, option_some_entry) =
                self.tiled_option_binding(carrier.origin_option, location)?;
            let Some(option_some_entry) = option_some_entry else {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedProjection,
                    location.clone(),
                    "tiled witness payload is used before an authenticated Some-edge guard",
                ));
            };
            if self
                .try_bridge
                .success_entries
                .get(&carrier.origin_option)
                .copied()
                != Some(option_some_entry)
            {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedProjection,
                    location.clone(),
                    "tiled witness origin does not retain its compiler-authenticated Option success entry",
                ));
            }
            let Some(guard_entry) = self.try_bridge.success_entries.get(&source.local).copied()
            else {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedProjection,
                    location.clone(),
                    "tiled witness carrier has no authenticated success switch",
                ));
            };
            let Some(use_block) = location.block else {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedProjection,
                    location.clone(),
                    "tiled witness payload use has no MIR block identity",
                ));
            };
            if !mir_block_dominates(self.function, guard_entry, use_block) {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedProjection,
                    location.clone(),
                    "tiled witness payload use is not dominated by its authenticated carrier success edge",
                ));
            }
            Some(LocalBinding::Tiled2dWitness {
                raw,
                evidence,
                some_entry: guard_entry,
            })
        } else {
            None
        };
        let Some(binding) = binding else {
            return Ok(false);
        };
        if !destination.projection.is_empty()
            || destination.semantic_identity != source.semantic_identity
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                "tiled witness extraction must preserve the exact compiler-authenticated DisjointTile2D payload identity",
            ));
        }
        self.bind_local(destination.local, binding, location.clone())?;
        Ok(true)
    }

    fn lower_matrix_aggregate_use(
        &mut self,
        destination: &MirPlaceRef,
        operand: &MirOperandRef,
        location: &TranslationLocation,
    ) -> Result<bool, TranslationDiagnostic> {
        let MirOperandRef::Place(source) = operand else {
            return Ok(false);
        };
        if !destination.projection.is_empty() || !source.projection.is_empty() {
            return Ok(false);
        }
        let Some(binding) = self.locals.get(&source.local).copied() else {
            return Ok(false);
        };
        if !matches!(
            binding,
            LocalBinding::Bf16MfmaFragment(_)
                | LocalBinding::F32AccumulatorFragment(_)
                | LocalBinding::FixedArray4(_)
        ) {
            return Ok(false);
        }
        if !self.is_authenticated_general_v3_scalar_context()
            || !self.is_exact_gfx942_wave64_matrix_context()
            || destination.semantic_identity != source.semantic_identity
            || self.imported_local_shape(destination.local)
                != self.imported_local_shape(source.local)
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                "matrix aggregate copy does not preserve its exact authenticated source type and gfx942 one-wave context",
            ));
        }
        let shape_matches_binding = match (binding, self.imported_local_shape(destination.local)) {
            (LocalBinding::Bf16MfmaFragment(_), Some(MirTypeShape::Adt { identity })) => {
                identity == TrustedDeviceItem::Bf16MfmaFragment.canonical_path()
            }
            (LocalBinding::F32AccumulatorFragment(_), Some(MirTypeShape::Adt { identity })) => {
                identity == TrustedDeviceItem::F32AccumulatorFragment.canonical_path()
            }
            (
                LocalBinding::FixedArray4(_),
                Some(MirTypeShape::Array {
                    element,
                    length: Some(4),
                }),
            ) => matches!(element.as_ref(), MirTypeShape::U16 | MirTypeShape::F32),
            _ => false,
        };
        if !shape_matches_binding {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                "matrix aggregate copy binding does not match its exact imported compiler type",
            ));
        }
        self.bind_local(destination.local, binding, location.clone())?;
        Ok(true)
    }

    fn lower_try_carrier_use(
        &self,
        destination: &MirPlaceRef,
        operand: &MirOperandRef,
        location: &TranslationLocation,
    ) -> Result<bool, TranslationDiagnostic> {
        let MirOperandRef::Place(source) = operand else {
            return Ok(false);
        };
        if !destination.projection.is_empty() || !source.projection.is_empty() {
            return Ok(false);
        }
        let Some(destination_carrier) = self.try_bridge.carriers.get(&destination.local).copied()
        else {
            return Ok(false);
        };
        let Some(source_carrier) = self.try_bridge.carriers.get(&source.local).copied() else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedRvalue,
                location.clone(),
                "tiled try carrier copy has no compiler-authenticated source carrier",
            ));
        };
        if destination_carrier != source_carrier
            || destination.semantic_identity != source.semantic_identity
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                "tiled try carrier copy does not preserve its authenticated origin and exact compiler type identity",
            ));
        }
        Ok(true)
    }

    fn lower_checked_pointer_use(
        &mut self,
        destination: &MirPlaceRef,
        operand: &MirOperandRef,
        location: &TranslationLocation,
    ) -> Result<bool, TranslationDiagnostic> {
        let MirOperandRef::Place(source) = operand else {
            return Ok(false);
        };
        if !destination.projection.is_empty() {
            return Ok(false);
        }
        let Some(origin_option) = self
            .try_bridge
            .payload_values
            .get(&destination.local)
            .copied()
        else {
            return Ok(false);
        };
        let Some(TryOrigin::CheckedPointer(_)) =
            self.try_bridge.origins.get(&origin_option).copied()
        else {
            return Ok(false);
        };
        let source_preserves_origin = match source.projection.as_slice() {
            [] => self.try_bridge.payload_values.get(&source.local).copied() == Some(origin_option),
            [
                MirProjectionElem::Downcast { variant },
                MirProjectionElem::Field(0),
            ] => self
                .try_bridge
                .carriers
                .get(&source.local)
                .is_some_and(|carrier| {
                    carrier.origin_option == origin_option
                        && *variant == carrier.kind.success_variant()
                }),
            _ => false,
        };
        if !source_preserves_origin || destination.semantic_identity != source.semantic_identity {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                "checked pointer extraction does not preserve its exact authenticated carrier and compiler type identity",
            ));
        }
        let Some(guard_entry) = self
            .try_bridge
            .payload_guard_entries
            .get(&destination.local)
            .copied()
        else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                "checked pointer payload has no compiler-authenticated success guard",
            ));
        };
        let Some(use_block) = location.block else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                "checked pointer payload use has no MIR block identity",
            ));
        };
        if !mir_block_dominates(self.function, guard_entry, use_block) {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                "checked pointer payload escapes its authenticated carrier success region",
            ));
        }
        let (_, payload, some_entry) = self.pointer_option_binding(origin_option, location)?;
        let Some(some_entry) = some_entry else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                "checked pointer payload is used before its originating Option success edge",
            ));
        };
        if self.try_bridge.success_entries.get(&origin_option).copied() != Some(some_entry) {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                "checked pointer payload is not dominated by its exact bounds and Option success regions",
            ));
        }
        self.guarded_pointer_values.insert(payload, guard_entry);
        self.bind_local(
            destination.local,
            LocalBinding::Value(payload),
            location.clone(),
        )?;
        Ok(true)
    }

    fn lower_try_failure_use(
        &mut self,
        destination: &MirPlaceRef,
        operand: &MirOperandRef,
        location: &TranslationLocation,
    ) -> Result<bool, TranslationDiagnostic> {
        if !self.elides_generated_result || !destination.projection.is_empty() {
            return Ok(false);
        }
        let MirOperandRef::Place(source) = operand else {
            return Ok(false);
        };
        let origin = match source.projection.as_slice() {
            [] => self.tiled_error_origins.get(&source.local).copied(),
            [
                MirProjectionElem::Downcast { variant },
                MirProjectionElem::Field(0),
            ] => {
                if let Some(carrier) = self.try_bridge.carriers.get(&source.local).copied() {
                    if carrier.kind == TryCarrierKind::Option
                        || *variant == carrier.kind.success_variant()
                    {
                        return Ok(false);
                    }
                    let destination_shape = self.local_shape(destination.local, location)?;
                    let exact_payload = match carrier.kind {
                        TryCarrierKind::ControlFlow => is_standard_result_shape(destination_shape),
                        TryCarrierKind::Result => {
                            matches!(destination_shape, MirTypeShape::Adt { .. })
                        }
                        TryCarrierKind::Option => false,
                    };
                    if !exact_payload || destination.semantic_identity != source.semantic_identity {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location.clone(),
                            "tiled try failure projection does not preserve its exact compiler payload identity",
                        ));
                    }
                    Some(carrier.origin_option)
                } else if let Some(origin) = self.tiled_error_origins.get(&source.local).copied() {
                    if *variant != 1
                        || !is_standard_result_shape(self.local_shape(source.local, location)?)
                        || !matches!(
                            self.local_shape(destination.local, location)?,
                            MirTypeShape::Adt { .. }
                        )
                        || destination.semantic_identity != source.semantic_identity
                    {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location.clone(),
                            "elided tiled Result error projection is not the exact KernelError payload",
                        ));
                    }
                    Some(origin)
                } else {
                    None
                }
            }
            _ => None,
        };
        let Some(origin) = origin else {
            return Ok(false);
        };
        if let Some(previous) = self.tiled_error_origins.insert(destination.local, origin)
            && previous != origin
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                format!(
                    "elided tiled error local{} has conflicting authenticated origins",
                    destination.local
                ),
            ));
        }
        Ok(true)
    }

    fn tiled_witness_binding(
        &self,
        local: usize,
        location: &TranslationLocation,
    ) -> Result<Option<LocalBinding>, TranslationDiagnostic> {
        if let Some(binding @ LocalBinding::Tiled2dWitness { .. }) =
            self.locals.get(&local).copied()
        {
            return Ok(Some(binding));
        }
        let Some(origin_option) = self.try_bridge.payload_values.get(&local).copied() else {
            return Ok(None);
        };
        let Some(guard_entry) = self.try_bridge.payload_guard_entries.get(&local).copied() else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                format!("tiled witness local{local} has no compiler-authenticated success guard"),
            ));
        };
        let (_, raw, evidence, option_some_entry) =
            self.tiled_option_binding(origin_option, location)?;
        let Some(option_some_entry) = option_some_entry else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                "tiled witness is used before its originating Option success edge is lowered",
            ));
        };
        if self.try_bridge.success_entries.get(&origin_option).copied() != Some(option_some_entry) {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                "tiled witness origin does not retain its compiler-authenticated Option success entry",
            ));
        }
        let Some(use_block) = location.block else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                "tiled witness use has no MIR block identity",
            ));
        };
        if !mir_block_dominates(self.function, guard_entry, use_block) {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location.clone(),
                "tiled witness use escapes its compiler-authenticated success region",
            ));
        }
        Ok(Some(LocalBinding::Tiled2dWitness {
            raw,
            evidence,
            some_entry: guard_entry,
        }))
    }

    fn try_option_binding(
        &self,
        option_local: usize,
        location: &TranslationLocation,
    ) -> Result<(ValueId, Option<usize>), TranslationDiagnostic> {
        match self.try_bridge.origins.get(&option_local).copied() {
            Some(TryOrigin::Tiled2d) => {
                let (discriminant, _, _, some_entry) =
                    self.tiled_option_binding(option_local, location)?;
                Ok((discriminant, some_entry))
            }
            Some(TryOrigin::CheckedPointer(_)) => {
                let (discriminant, _, some_entry) =
                    self.pointer_option_binding(option_local, location)?;
                Ok((discriminant, some_entry))
            }
            None => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                format!(
                    "try bridge origin local{option_local} has no authenticated semantic origin"
                ),
            )),
        }
    }

    fn pointer_option_binding(
        &self,
        option_local: usize,
        location: &TranslationLocation,
    ) -> Result<(ValueId, ValueId, Option<usize>), TranslationDiagnostic> {
        match self.locals.get(&option_local).copied() {
            Some(LocalBinding::OptionPointer {
                discriminant,
                payload,
                some_entry,
            }) => Ok((discriminant, payload, some_entry)),
            Some(_) => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                format!(
                    "try bridge origin local{option_local} is not an authenticated checked-pointer Option"
                ),
            )),
            None => Err(self.undefined_local(option_local, location.clone())),
        }
    }

    fn tiled_option_binding(
        &self,
        option_local: usize,
        location: &TranslationLocation,
    ) -> Result<
        (
            ValueId,
            ValueId,
            MirCheckedTiled2dCallEvidenceV1,
            Option<usize>,
        ),
        TranslationDiagnostic,
    > {
        match self.locals.get(&option_local).copied() {
            Some(LocalBinding::OptionTiled2dWitness {
                discriminant,
                raw,
                evidence,
                some_entry,
            }) => Ok((discriminant, raw, evidence, some_entry)),
            Some(_) => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                format!(
                    "tiled try bridge origin local{option_local} is not an authenticated checked_tiled_2d result"
                ),
            )),
            None => Err(self.undefined_local(option_local, location.clone())),
        }
    }

    fn lower_try_aggregate(
        &mut self,
        destination: &MirPlaceRef,
        variant: usize,
        active_field: Option<usize>,
        operands: &[MirOperandRef],
        semantic_rvalue_type: &Option<crate::mir_import::MirSemanticTypeEvidence>,
        location: &TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        let carrier = self.try_bridge.carriers[&destination.local];
        let local = imported_local(self.function, destination.local).ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                format!(
                    "tiled try carrier local{} is not imported",
                    destination.local
                ),
            )
        })?;
        if !destination.projection.is_empty()
            || active_field.is_some_and(|field| field != 0)
            || semantic_rvalue_type.as_ref() != Some(&local.ty.semantic_identity)
            || variant > 1
            || operands.len() != 1
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedRvalue,
                location.clone(),
                "tiled try carrier construction is not an exact one-field Result or ControlFlow variant",
            ));
        }
        if variant == carrier.kind.success_variant() {
            let [MirOperandRef::Place(source)] = operands else {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedRvalue,
                    location.clone(),
                    "tiled try success carrier requires one authenticated tile local",
                ));
            };
            if !source.projection.is_empty()
                || self.try_bridge.payload_values.get(&source.local).copied()
                    != Some(carrier.origin_option)
            {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedRvalue,
                    location.clone(),
                    "tiled try success carrier does not preserve its authenticated tile origin",
                ));
            }
        } else {
            match carrier.kind {
                TryCarrierKind::Option => {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedRvalue,
                        location.clone(),
                        "checked Option failure must use the separately authenticated fieldless None construction",
                    ));
                }
                TryCarrierKind::Result => {
                    let [MirOperandRef::Constant { ty, literal, .. }] = operands else {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedRvalue,
                            location.clone(),
                            "try Result::Err must carry the exact constant KernelError::OutOfBounds payload",
                        ));
                    };
                    if !is_trusted_adt_shape(&ty.shape, TrustedDeviceItem::KernelError)
                        || literal != &MirConstant::FieldlessEnumVariant(2)
                    {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location.clone(),
                            "try Result::Err payload is not the exact reviewed KernelError::OutOfBounds variant",
                        ));
                    }
                }
                TryCarrierKind::ControlFlow => {
                    let [MirOperandRef::Place(source)] = operands else {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedRvalue,
                            location.clone(),
                            "try ControlFlow::Break must carry its exact generated residual Result",
                        ));
                    };
                    if !source.projection.is_empty()
                        || self.tiled_error_origins.get(&source.local).copied()
                            != Some(carrier.origin_option)
                        || !is_standard_result_shape(self.local_shape(source.local, location)?)
                    {
                        return Err(diagnostic(
                            TranslationDiagnosticCode::UnsupportedType,
                            location.clone(),
                            "try ControlFlow::Break does not preserve its authenticated residual Result origin",
                        ));
                    }
                }
            }
        }
        if carrier.kind == TryCarrierKind::Option {
            let Some(TryOrigin::CheckedPointer(origin)) =
                self.try_bridge.origins.get(&carrier.origin_option).copied()
            else {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedRvalue,
                    location.clone(),
                    "only an authenticated checked-pointer Option may be reconstructed as an ADT aggregate",
                ));
            };
            if variant != TryCarrierKind::Option.success_variant()
                || location.block != Some(origin.some_block)
            {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedRvalue,
                    location.clone(),
                    "checked-pointer Option::Some is not constructed in its exact bounds-success block",
                ));
            }
            let [MirOperandRef::Place(source)] = operands else {
                unreachable!("one place operand validated above");
            };
            if source.local != origin.pointer_local {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedRvalue,
                    location.clone(),
                    "checked-pointer Option::Some does not carry its exact indexed reference local",
                ));
            }
            let payload = self.plain_local(source.local, location)?;
            let discriminant = self.plain_local(origin.bounds_local, location)?;
            if self.value_type(discriminant, location)? != &Type::BOOL {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedType,
                    location.clone(),
                    "checked-pointer bounds predicate did not lower to an exact boolean",
                ));
            }
            return self.bind_local(
                destination.local,
                LocalBinding::OptionPointer {
                    discriminant,
                    payload,
                    some_entry: None,
                },
                location.clone(),
            );
        }
        Ok(())
    }

    fn lower_try_error_aggregate(
        &mut self,
        destination: &MirPlaceRef,
        variant: usize,
        active_field: Option<usize>,
        operands: &[MirOperandRef],
        semantic_rvalue_type: &Option<crate::mir_import::MirSemanticTypeEvidence>,
        location: &TranslationLocation,
    ) -> Result<bool, TranslationDiagnostic> {
        if variant != 1
            || active_field.is_some_and(|field| field != 0)
            || !destination.projection.is_empty()
            || !is_standard_result_shape(self.local_shape(destination.local, location)?)
        {
            return Ok(false);
        }
        let [MirOperandRef::Place(source)] = operands else {
            return Ok(false);
        };
        if !source.projection.is_empty() {
            return Ok(false);
        }
        let Some(origin) = self.tiled_error_origins.get(&source.local).copied() else {
            return Ok(false);
        };
        let local = imported_local(self.function, destination.local).ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                format!("try residual local{} is not imported", destination.local),
            )
        })?;
        if semantic_rvalue_type.as_ref() != Some(&local.ty.semantic_identity) {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                "generated try residual Result does not preserve its exact compiler type identity",
            ));
        }
        if let Some(previous) = self.tiled_error_origins.insert(destination.local, origin)
            && previous != origin
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                format!(
                    "generated try residual local{} has conflicting authenticated origins",
                    destination.local
                ),
            ));
        }
        Ok(true)
    }

    fn validate_checked_pointer_none(
        &self,
        destination: &MirPlaceRef,
        operands: &[MirOperandRef],
        semantic_rvalue_type: &Option<crate::mir_import::MirSemanticTypeEvidence>,
        location: &TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        let Some(TryOrigin::CheckedPointer(origin)) =
            self.try_bridge.origins.get(&destination.local).copied()
        else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedRvalue,
                location.clone(),
                "fieldless Option::None is not linked to an authenticated checked pointer",
            ));
        };
        let local = imported_local(self.function, destination.local).ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                format!(
                    "checked pointer Option local{} is not imported",
                    destination.local
                ),
            )
        })?;
        if !destination.projection.is_empty()
            || !operands.is_empty()
            || semantic_rvalue_type.as_ref() != Some(&local.ty.semantic_identity)
            || location.block != Some(origin.none_block)
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedRvalue,
                location.clone(),
                "checked-pointer Option::None is not the exact fieldless failure construction",
            ));
        }
        Ok(())
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
                    | LocalBinding::OptionTiled2dWitness { .. }
                    | LocalBinding::Tiled2dWitness { .. }
                    | LocalBinding::FieldlessEnum { .. }
                    | LocalBinding::DeviceMathCapability
                    | LocalBinding::Gfx942CollectiveCapability
                    | LocalBinding::Gfx942StaticLdsU32x256(_)
                    | LocalBinding::DeviceMatrixValueCapability
                    | LocalBinding::DeviceMatrixReferenceCapability
                    | LocalBinding::Bf16MfmaFragment(_)
                    | LocalBinding::F32AccumulatorFragment(_)
                    | LocalBinding::FixedArray4(_),
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
                    if self.is_authenticated_general_v3_scalar_context() {
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
                    | LocalBinding::OptionTiled2dWitness { .. }
                    | LocalBinding::Tiled2dWitness { .. }
                    | LocalBinding::FieldlessEnum { .. }
                    | LocalBinding::DeviceMathCapability
                    | LocalBinding::Gfx942CollectiveCapability
                    | LocalBinding::Gfx942StaticLdsU32x256(_)
                    | LocalBinding::DeviceMatrixValueCapability
                    | LocalBinding::DeviceMatrixReferenceCapability
                    | LocalBinding::Bf16MfmaFragment(_)
                    | LocalBinding::F32AccumulatorFragment(_)
                    | LocalBinding::FixedArray4(_),
                ) => Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedType,
                    location.clone(),
                    format!("local{} is not a translated Option pointer", place.local),
                )),
                None => Err(self.undefined_local(place.local, location.clone())),
            },
            [
                MirProjectionElem::ConstantIndex {
                    offset,
                    min_length,
                    from_end: false,
                },
            ] if *offset < 4 && *min_length == *offset + 1 => {
                if !matches!(
                    self.imported_local_shape(place.local),
                    Some(MirTypeShape::Array { element, length: Some(4) })
                        if element.as_ref() == &MirTypeShape::F32
                ) {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location.clone(),
                        "constant fragment projection requires an exact `[f32; 4]` source local",
                    ));
                }
                let Some(LocalBinding::FixedArray4(values)) =
                    self.locals.get(&place.local).copied()
                else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedProjection,
                        location.clone(),
                        "constant fragment projection requires an authenticated fixed-array binding",
                    ));
                };
                let value = values[*offset as usize];
                if self.value_type(value, location)? != &Type::F32 {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location.clone(),
                        "constant fragment projection selected a non-f32 value",
                    ));
                }
                Ok(value)
            }
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
            projection => {
                let imported_type = imported_local(self.function, place.local)
                    .map(|local| local.ty.rust.as_str())
                    .unwrap_or("<missing imported type>");
                Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedProjection,
                    location.clone(),
                    format!(
                        "unsupported place projection on local{} of type `{imported_type}` with binding {:?}: {projection:?}",
                        place.local,
                        self.locals.get(&place.local)
                    ),
                ))
            }
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
                | LocalBinding::OptionTiled2dWitness { .. }
                | LocalBinding::Tiled2dWitness { .. }
                | LocalBinding::FieldlessEnum { .. }
                | LocalBinding::DeviceMathCapability
                | LocalBinding::Gfx942CollectiveCapability
                | LocalBinding::Gfx942StaticLdsU32x256(_)
                | LocalBinding::DeviceMatrixValueCapability
                | LocalBinding::DeviceMatrixReferenceCapability
                | LocalBinding::Bf16MfmaFragment(_)
                | LocalBinding::F32AccumulatorFragment(_)
                | LocalBinding::FixedArray4(_),
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
        let mut arguments = Vec::new();
        for &local in self.control_flow_ssa.live_in(target) {
            let values = match (
                self.control_flow_ssa.kind(local),
                self.locals.get(&local).copied(),
            ) {
                (
                    Some(control_flow_ssa::PromotedLocalKind::Scalar),
                    Some(LocalBinding::Value(value)),
                )
                | (
                    Some(control_flow_ssa::PromotedLocalKind::FieldlessEnum),
                    Some(LocalBinding::FieldlessEnum {
                        discriminant: value,
                    }),
                ) => vec![value],
                (
                    Some(control_flow_ssa::PromotedLocalKind::F32AccumulatorFragment),
                    Some(LocalBinding::F32AccumulatorFragment(values)),
                ) => values.to_vec(),
                (Some(_), Some(_)) => {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location.clone(),
                        format!(
                            "local{local} binding does not match its authenticated control-flow promotion kind"
                        ),
                    ));
                }
                (Some(_), None) => {
                    return Err(self.undefined_local(local, location.clone()));
                }
                (None, _) => {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location.clone(),
                        format!("local{local} is live on an edge but has no promotion plan"),
                    ));
                }
            };
            let expected_types = self
                .control_flow_ssa
                .types(local)
                .expect("live-in local is promoted");
            if values.len() != expected_types.len()
                || values.iter().zip(expected_types).any(|(value, expected)| {
                    self.value_types
                        .get(value)
                        .is_none_or(|actual| actual != expected)
                })
            {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedType,
                    location.clone(),
                    format!(
                        "local{local} control-flow values do not preserve their promoted compiler types"
                    ),
                ));
            }
            arguments.extend(values);
        }
        Ok(arguments)
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
        let imported_type = imported_local(self.function, local)
            .map(|local| local.ty.rust.as_str())
            .unwrap_or("<missing imported type>");
        diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            location,
            format!("local{local} is used before it is defined (imported type `{imported_type}`)"),
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

fn declared_function_signature(
    function: &MirFunction,
    source_abi: &MirFunction,
    elides_generated_result: bool,
) -> Result<Signature, TranslationDiagnostic> {
    validate_matrix_frontend_function_abi(source_abi)?;
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
    let results = if elides_generated_result {
        Vec::new()
    } else {
        match (&function.kind, &return_local.ty.shape) {
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

fn mir_reverse_postorder(function: &MirFunction) -> Result<Vec<&MirBlock>, TranslationDiagnostic> {
    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.index, block))
        .collect::<BTreeMap<_, _>>();
    if !blocks.contains_key(&0) {
        return Err(diagnostic(
            TranslationDiagnosticCode::MalformedMir,
            TranslationLocation::function(function),
            "kernel must contain entry block bb0",
        ));
    }

    let mut seen = BTreeSet::new();
    let mut postorder = Vec::with_capacity(blocks.len());
    let mut stack = vec![(0usize, false)];
    while let Some((index, expanded)) = stack.pop() {
        if expanded {
            postorder.push(index);
            continue;
        }
        if !seen.insert(index) {
            continue;
        }
        let block = blocks.get(&index).copied().ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(function),
                format!("control-flow graph references missing bb{index}"),
            )
        })?;
        let terminator = block.terminator.as_ref().ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::block(function, block),
                "basic block has no terminator",
            )
        })?;
        stack.push((index, true));
        let mut successors = mir_successors(&terminator.kind);
        successors.sort_unstable();
        successors.dedup();
        for successor in successors.into_iter().rev() {
            if !blocks.contains_key(&successor) {
                return Err(diagnostic(
                    TranslationDiagnosticCode::MalformedMir,
                    TranslationLocation::block(function, block),
                    format!("control-flow graph references missing bb{successor}"),
                ));
            }
            if !seen.contains(&successor) {
                stack.push((successor, false));
            }
        }
    }

    postorder.reverse();
    let mut result = postorder
        .into_iter()
        .map(|index| blocks[&index])
        .collect::<Vec<_>>();
    result.extend(
        blocks
            .iter()
            .filter(|(index, _)| !seen.contains(index))
            .map(|(_, block)| *block),
    );
    Ok(result)
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
        MirTypeShape::U16 => Some(Type::Scalar(ScalarType::U16)),
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
    lower_scalar_type(shape)
}

fn lower_constant(constant: &MirConstant) -> Option<Constant> {
    match constant {
        MirConstant::Bool(value) => Some(Constant::Bool(*value)),
        MirConstant::U16(value) => Some(Constant::U16(*value)),
        MirConstant::I32(value) => Some(Constant::I32(*value)),
        MirConstant::U32(value) => Some(Constant::U32(*value)),
        MirConstant::I64(value) | MirConstant::ISize(value) => Some(Constant::I64(*value)),
        MirConstant::U64(value) => Some(Constant::U64(*value)),
        MirConstant::USize(value) => Some(Constant::Index(*value)),
        MirConstant::F32Bits(value) => Some(Constant::F32Bits(*value)),
        MirConstant::F64Bits(value) => Some(Constant::F64Bits(*value)),
        MirConstant::ZeroSized
        | MirConstant::FieldlessEnumVariant(_)
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
    use fe2o3_kernel_ir::tiled_gemm_lds_v1_module;
    use fe2o3_rustc_front::{
        FrontendLaunchBoundsV1, FrontendWorkgroupDimensionsV1, KernelFrontendContractV1,
    };

    fn exact_frontend_launch_for_test(
        x: u32,
    ) -> crate::collector::AuthenticatedKernelFrontendContractV1 {
        let dimensions = FrontendWorkgroupDimensionsV1::new([x, 1, 1]).unwrap();
        let launch = FrontendLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
        crate::collector::AuthenticatedKernelFrontendContractV1::for_test(
            KernelFrontendContractV1::new(Some(launch), None).unwrap(),
        )
    }

    #[test]
    fn typed_launch_contract_matches_the_profile_specific_macro_domain() {
        let mut function = scalar_fixture().functions.remove(0);
        function.typed_profile = Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3);
        for x in [64, 256] {
            function.frontend_contract = Some(exact_frontend_launch_for_test(x));
            assert_eq!(
                kernel_ir_launch_contract(&function).unwrap(),
                Some(WorkgroupSize::new(x, 1, 1))
            );
        }

        function.frontend_contract = Some(exact_frontend_launch_for_test(32));
        assert!(kernel_ir_launch_contract(&function).is_err());

        function.typed_profile = Some(MirKernelProfile::VecAddRustcLayoutV2);
        function.frontend_contract = Some(exact_frontend_launch_for_test(64));
        assert!(kernel_ir_launch_contract(&function).is_err());
    }

    #[test]
    fn slice_elements_accept_every_admitted_scalar_shape() {
        for shape in [
            MirTypeShape::Bool,
            MirTypeShape::I32,
            MirTypeShape::U32,
            MirTypeShape::I64,
            MirTypeShape::ISize,
            MirTypeShape::USize,
            MirTypeShape::F16,
            MirTypeShape::Bf16,
            MirTypeShape::Bf16x2,
            MirTypeShape::F32,
            MirTypeShape::F64,
        ] {
            assert_eq!(lower_element_type(&shape), lower_scalar_type(&shape));
            assert!(lower_element_type(&shape).is_some(), "{shape:?}");
        }
        assert_eq!(lower_element_type(&MirTypeShape::Unknown), None);
    }

    #[test]
    fn exact_u16_slice_types_and_constants_lower_without_widening() {
        assert_eq!(
            lower_parameter_type(&MirTypeShape::Slice {
                element: Box::new(MirTypeShape::U16),
                mutable: false,
            }),
            Some(Type::slice(
                Type::Scalar(ScalarType::U16),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ))
        );
        assert_eq!(
            lower_constant(&MirConstant::U16(0xa55a)),
            Some(Constant::U16(0xa55a))
        );
    }

    #[test]
    fn production_kernel_ir_boundary_runs_general_checks() {
        verify_and_check_module(tiled_gemm_lds_v1_module())
            .expect("canonical tiled GEMM has published LDS reads");

        let mut missing_publish = tiled_gemm_lds_v1_module();
        missing_publish.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .retain(|operation| !matches!(operation.kind, OperationKind::WorkgroupBarrier(_)));
        let errors = verify_and_check_module(missing_publish).unwrap_err();
        assert!(errors.contains(TranslationDiagnosticCode::KernelCheckRejected));
        assert!(
            errors
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("may observe unpublished data") })
        );
    }

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
    fn general_v3_launch_geometry_accepts_only_its_authenticated_64_or_256_profiles() {
        for dimensions in [[64, 1, 1], [256, 1, 1]] {
            let mut kernel = empty_kernel_with_contract(launch_contract(
                Some(dimensions),
                Some(dimensions),
                None,
            ));
            kernel.typed_profile = Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3);
            let module = translate_and_verify(&MirModule {
                functions: vec![kernel],
            })
            .expect("authenticated General V3 workgroup");
            assert_eq!(
                module.kernels[0].workgroup_size,
                Some(WorkgroupSize::new(
                    dimensions[0],
                    dimensions[1],
                    dimensions[2]
                ))
            );
        }

        let mut invalid =
            empty_kernel_with_contract(launch_contract(Some([128, 1, 1]), Some([128, 1, 1]), None));
        invalid.typed_profile = Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3);
        let error = translate_and_verify(&MirModule {
            functions: vec![invalid],
        })
        .expect_err("non-profile General V3 workgroup must fail");
        assert!(error.to_string().contains("exact 64x1x1 or 256x1x1"));
    }

    #[test]
    fn control_flow_ssa_accepts_collector_bounded_graphs_above_the_legacy_128_cap() {
        let mut function =
            empty_kernel_with_contract(launch_contract(Some([256, 1, 1]), Some([256, 1, 1]), None));
        function.blocks = (0..203)
            .map(|index| MirBlock {
                index,
                statements: Vec::new(),
                terminator: Some(terminator(if index == 202 {
                    MirTerminatorKind::Return
                } else {
                    MirTerminatorKind::Goto { target: index + 1 }
                })),
            })
            .collect();

        let plan = control_flow_ssa::ControlFlowSsaPlan::analyze(&function, false)
            .expect("collector-bounded 203-block graph");
        assert_eq!(plan.promoted_locals().count(), 0);
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
    fn scalar_comparison_family_lowers_with_exact_predicates() {
        let mut fixture = scalar_fixture();
        let function = &mut fixture.functions[0];
        function.blocks.truncate(1);
        function.locals.truncate(4);
        function
            .locals
            .extend((4..10).map(|index| local(index, MirLocalRole::Temp, MirTypeShape::Bool)));
        function.local_count = function.locals.len();
        function.blocks[0].statements = [
            MirBinaryOp::Eq,
            MirBinaryOp::Ne,
            MirBinaryOp::Lt,
            MirBinaryOp::Le,
            MirBinaryOp::Gt,
            MirBinaryOp::Ge,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, operation)| {
            assign(
                index,
                index + 4,
                vec![operand(1), operand(2)],
                MirRvalueKind::Binary(operation),
            )
        })
        .collect();
        function.blocks[0].terminator = Some(terminator(MirTerminatorKind::Return));

        let module = translate_and_verify(&fixture).expect("comparison family");
        let predicates = module.functions[0].body.as_ref().expect("body").blocks[0]
            .operations
            .iter()
            .filter_map(|operation| match &operation.kind {
                OperationKind::Compare { predicate, .. } => Some(*predicate),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            predicates,
            [
                ComparePredicate::Equal,
                ComparePredicate::NotEqual,
                ComparePredicate::LessThan,
                ComparePredicate::LessThanOrEqual,
                ComparePredicate::GreaterThan,
                ComparePredicate::GreaterThanOrEqual,
            ]
        );
    }

    #[test]
    fn integer_binary_family_lowers_without_bypassing_float_authority() {
        let mut fixture = scalar_fixture();
        let function = &mut fixture.functions[0];
        let operations = [
            MirBinaryOp::Add,
            MirBinaryOp::Sub,
            MirBinaryOp::Mul,
            MirBinaryOp::Div,
            MirBinaryOp::Rem,
            MirBinaryOp::BitXor,
            MirBinaryOp::BitAnd,
            MirBinaryOp::BitOr,
            MirBinaryOp::Shl,
            MirBinaryOp::Shr,
        ];
        function.blocks.truncate(1);
        function.locals = vec![
            local(0, MirLocalRole::Return, MirTypeShape::Unit),
            local(1, MirLocalRole::Arg, MirTypeShape::U32),
            local(2, MirLocalRole::Arg, MirTypeShape::U32),
        ];
        function.locals.extend(
            (0..operations.len())
                .map(|index| local(index + 3, MirLocalRole::Temp, MirTypeShape::U32)),
        );
        function.local_count = function.locals.len();
        function.blocks[0].statements = operations
            .into_iter()
            .enumerate()
            .map(|(index, operation)| {
                assign(
                    index,
                    index + 3,
                    vec![operand(1), operand(2)],
                    MirRvalueKind::Binary(operation),
                )
            })
            .collect();
        function.blocks[0].terminator = Some(terminator(MirTerminatorKind::Return));

        let module = translate_and_verify(&fixture).expect("integer binary family");
        let lowered = module.functions[0].body.as_ref().expect("body").blocks[0]
            .operations
            .iter()
            .filter_map(|operation| match &operation.kind {
                OperationKind::Binary { op, .. } => Some(*op),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            lowered,
            [
                BinaryOp::Add,
                BinaryOp::Subtract,
                BinaryOp::Multiply,
                BinaryOp::Divide,
                BinaryOp::Remainder,
                BinaryOp::BitXor,
                BinaryOp::BitAnd,
                BinaryOp::BitOr,
                BinaryOp::ShiftLeft,
                BinaryOp::ShiftRight,
            ]
        );

        let mut float_multiply = scalar_fixture();
        for local in &mut float_multiply.functions[0].locals[1..=3] {
            local.ty.kind = MirType::F32;
            local.ty.rust = "f32".to_string();
            local.ty.shape = MirTypeShape::F32;
        }
        float_multiply.functions[0].blocks[0].statements.truncate(1);
        float_multiply.functions[0].blocks[0].statements[0].rvalue =
            Some(MirRvalueKind::Binary(MirBinaryOp::Mul));
        let error = translate_and_verify(&float_multiply)
            .expect_err("unowned float multiply must remain rejected");
        assert!(error.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("authenticated semantic workload handler")
        }));
    }

    #[test]
    fn integer_cast_family_preserves_width_and_signedness() {
        let mut fixture = scalar_fixture();
        let function = &mut fixture.functions[0];
        function.arg_count = 3;
        function.locals = vec![
            local(0, MirLocalRole::Return, MirTypeShape::Unit),
            local(1, MirLocalRole::Arg, MirTypeShape::U32),
            local(2, MirLocalRole::Arg, MirTypeShape::I32),
            local(3, MirLocalRole::Arg, MirTypeShape::USize),
            local(4, MirLocalRole::Temp, MirTypeShape::USize),
            local(5, MirLocalRole::Temp, MirTypeShape::I64),
            local(6, MirLocalRole::Temp, MirTypeShape::U32),
            local(7, MirLocalRole::Temp, MirTypeShape::I32),
            local(8, MirLocalRole::Temp, MirTypeShape::U32),
        ];
        function.local_count = function.locals.len();
        function.blocks = vec![MirBlock {
            index: 0,
            statements: [(1, 4), (2, 5), (3, 6), (1, 7), (1, 8)]
                .into_iter()
                .enumerate()
                .map(|(index, (source, destination))| {
                    assign(
                        index,
                        destination,
                        vec![operand(source)],
                        MirRvalueKind::SemanticCast(MirCastKind::IntToInt),
                    )
                })
                .collect(),
            terminator: Some(terminator(MirTerminatorKind::Return)),
        }];

        let module = translate_and_verify(&fixture).expect("integer casts");
        let casts = module.functions[0].body.as_ref().expect("body").blocks[0]
            .operations
            .iter()
            .filter_map(|operation| match operation.kind {
                OperationKind::Cast { kind, .. } => Some(kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            casts,
            [
                CastKind::ZeroExtend,
                CastKind::SignExtend,
                CastKind::Truncate,
                CastKind::Bitcast,
            ]
        );
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
    fn session_recognized_primitive_rejects_a_wrong_typed_destination() {
        let mut fixture = helper_call_fixture(
            MirCallee::trusted_for_test(TrustedDeviceItem::ThreadIndex1d),
            &[],
        );
        fixture.functions[0].typed_profile =
            Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3);

        let errors = translate_and_verify(&fixture)
            .expect_err("recognized General V3 primitive with the wrong destination must fail");

        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedType));
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("destination is not the trusted ThreadIndex type")
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
    fn generated_result_body_inherits_kernel_context_and_lowers_to_zero_results() {
        let fixture = generated_result_bridge_fixture();
        let helper = &fixture.functions[1];
        let contexts = internal_kernel_contexts_v1(&fixture).expect("generated bridge context");
        let context = contexts
            .get(&helper.semantic_instance_v1())
            .expect("generated body context");
        assert_eq!(
            context.root.typed_profile,
            Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3)
        );
        assert_eq!(context.source_abi, Some(&fixture.functions[0]));
        assert!(context.elides_generated_result);

        let module = translate_and_verify(&fixture).expect("generated Result bridge");
        let helper = module
            .functions
            .iter()
            .find(|function| function.id.as_str() == "generated_body")
            .expect("generated body definition");
        assert!(helper.signature.results.is_empty());
        assert!(
            helper
                .body
                .as_ref()
                .expect("generated body")
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(operation.kind, OperationKind::Intrinsic(_)))
        );
        assert!(
            helper
                .body
                .as_ref()
                .expect("generated body")
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                ))
        );
        assert!(matches!(
            helper.body.as_ref().expect("generated body").blocks[2].terminator,
            Some(Terminator::Return { ref values }) if values.is_empty()
        ));
        let entry = module
            .functions
            .iter()
            .find(|function| function.role == fe2o3_kernel_ir::FunctionRole::KernelEntry)
            .expect("generated entry definition");
        assert!(
            entry
                .body
                .as_ref()
                .expect("generated entry")
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    &operation.kind,
                    OperationKind::Call { callee, .. } if callee.as_str() == "generated_body"
                ) && operation.results.is_empty())
        );
    }

    #[test]
    fn checked_tiled_2d_requires_compiler_evidence_and_exact_geometry() {
        let mutate_callee = |fixture: &mut MirModule, replacement: MirCallee| {
            let MirTerminatorKind::Call { callee, .. } = &mut fixture.functions[1].blocks[1]
                .terminator
                .as_mut()
                .expect("checked tiled call")
                .kind
            else {
                panic!("checked tiled call")
            };
            *callee = Some(replacement);
        };

        let mut missing = generated_result_bridge_fixture();
        mutate_callee(
            &mut missing,
            MirCallee::trusted_for_test(TrustedDeviceItem::ThreadIndexCheckedTiled2D),
        );
        let errors = translate_and_verify(&missing).expect_err("missing const-generic evidence");
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("lacks compiler-authenticated const-generic evidence")
        }));

        let mut malformed = generated_result_bridge_fixture();
        mutate_callee(
            &mut malformed,
            MirCallee::checked_tiled_2d_for_test(64, 15, 16, 4),
        );
        let errors = translate_and_verify(&malformed).expect_err("malformed tiled geometry");
        assert!(errors.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("requires exact authenticated Index1D -> Tiled2D")
        }));
    }

    #[test]
    fn generated_result_bridge_rejects_every_observability_mutation() {
        let replace_callee = |fixture: &mut MirModule, replacement: MirCallee| {
            let Some(MirTerminator {
                kind: MirTerminatorKind::Call { callee, .. },
                ..
            }) = fixture.functions[0].blocks[0].terminator.as_mut()
            else {
                panic!("generated call fixture")
            };
            *callee = Some(replacement);
        };

        let mut missing_seal = generated_result_bridge_fixture();
        let helper_path = missing_seal.functions[1].rust_path.clone();
        replace_callee(
            &mut missing_seal,
            MirCallee::untrusted_for_test(helper_path),
        );

        let mut wrong_binding = generated_result_bridge_fixture();
        let root_path = wrong_binding.functions[0].rust_path.clone();
        let helper_path = wrong_binding.functions[1].rust_path.clone();
        replace_callee(
            &mut wrong_binding,
            MirCallee::authenticated_kernel_body_for_test(
                root_path,
                helper_path,
                reserved_fe2o3_symbols::KernelBindingIdV1::from_bytes([0xee; 32]),
            ),
        );

        let mut wrong_root = generated_result_bridge_fixture();
        let helper_path = wrong_root.functions[1].rust_path.clone();
        replace_callee(
            &mut wrong_root,
            MirCallee::authenticated_kernel_body_for_test(
                "tests::not_the_authenticated_root",
                helper_path,
                reserved_fe2o3_symbols::KernelBindingIdV1::from_bytes([
                    0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89,
                    0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23,
                    0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
                ]),
            ),
        );

        let mut reused = generated_result_bridge_fixture();
        reused.functions[0].blocks[1]
            .statements
            .insert(0, assign(1, 0, vec![operand(2)], MirRvalueKind::Use));

        let mut projected_destination = generated_result_bridge_fixture();
        let Some(MirTerminator {
            kind:
                MirTerminatorKind::Call {
                    destination: Some(destination),
                    ..
                },
            ..
        }) = projected_destination.functions[0].blocks[0]
            .terminator
            .as_mut()
        else {
            panic!("generated call fixture")
        };
        destination.projection.push(MirProjectionElem::Deref);

        let mut second_call_site = generated_result_bridge_fixture();
        let mut caller = second_call_site.functions[0].clone();
        caller.export_name = "second_kernel".to_owned();
        caller.rust_path = "tests::second_kernel".to_owned();
        caller.typed_profile = None;
        second_call_site.functions.push(caller);

        let mut reads_return = generated_result_bridge_fixture();
        let helper = &mut reads_return.functions[1];
        helper.blocks[0].terminator = Some(terminator(MirTerminatorKind::SwitchInt {
            discriminant: operand(0),
            targets: Vec::new(),
            otherwise: 1,
        }));

        for (name, fixture) in [
            ("missing compiler seal", missing_seal),
            ("mismatched kernel binding", wrong_binding),
            ("mismatched root identity", wrong_root),
            ("reused result", reused),
            ("projected destination", projected_destination),
            ("second call site", second_call_site),
            ("return read", reads_return),
        ] {
            let error = translate_and_verify(&fixture)
                .expect_err("generated bridge observability mutation must fail");
            assert!(
                error
                    .to_string()
                    .contains("authenticated generated kernel result bridge rejected"),
                "{name} escaped the generated bridge guard: {error}"
            );
        }
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
                    local(1, MirLocalRole::Arg, MirTypeShape::U32),
                    local(2, MirLocalRole::Arg, MirTypeShape::U32),
                    local(3, MirLocalRole::Temp, MirTypeShape::U32),
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

    fn generated_result_bridge_fixture() -> MirModule {
        const SUFFIX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        const BINDING: reserved_fe2o3_symbols::KernelBindingIdV1 =
            reserved_fe2o3_symbols::KernelBindingIdV1::from_bytes([
                0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
                0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
                0x89, 0xab, 0xcd, 0xef,
            ]);
        let root_path = format!("tests::{GENERATED_KERNEL_ENTRY_PREFIX_V1}{SUFFIX}");
        let helper_path = format!("tests::{GENERATED_KERNEL_BODY_PREFIX_V1}{SUFFIX}");
        let helper_return = local(
            0,
            MirLocalRole::Return,
            MirTypeShape::Adt {
                identity: "core::result::Result".to_owned(),
            },
        );
        let mut discarded_result = helper_return.clone();
        discarded_result.index = 2;
        discarded_result.role = MirLocalRole::Temp;
        let helper_result_constant = MirOperandRef::Constant {
            ty: helper_return.ty.clone(),
            literal: MirConstant::StructuredValue(vec![0]),
            value: "Result::Ok(())".to_owned(),
        };

        let root =
            MirFunction {
                semantic_instance: None,
                export_name: "generated_kernel".to_owned(),
                rust_path: root_path.clone(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
                frontend_contract: Some(
                    crate::collector::AuthenticatedKernelFrontendContractV1::for_test(
                        launch_contract(Some([64, 1, 1]), Some([64, 1, 1]), None),
                    ),
                ),
                matrix_frontend_abi: None,
                arg_count: 1,
                local_count: 3,
                locals: vec![
                    local(0, MirLocalRole::Return, MirTypeShape::Unit),
                    local(1, MirLocalRole::Arg, MirTypeShape::U32),
                    discarded_result,
                ],
                blocks: vec![
                    MirBlock {
                        index: 0,
                        statements: vec![storage_statement(0, MirStatementKind::StorageLive, 2)],
                        terminator: Some(terminator(MirTerminatorKind::Call {
                            callee: Some(MirCallee::authenticated_kernel_body_for_test(
                                root_path,
                                helper_path.clone(),
                                BINDING,
                            )),
                            target: Some(1),
                            destination: Some(place(2)),
                            operands: vec![operand(1)],
                        })),
                    },
                    MirBlock {
                        index: 1,
                        statements: vec![storage_statement(0, MirStatementKind::StorageDead, 2)],
                        terminator: Some(terminator(MirTerminatorKind::Return)),
                    },
                ],
            };
        let helper = MirFunction {
            semantic_instance: None,
            export_name: "generated_body".to_owned(),
            rust_path: helper_path,
            kind: MirFunctionKind::InternalHelper,
            typed_profile: None,
            frontend_contract: None,
            matrix_frontend_abi: None,
            arg_count: 1,
            local_count: 5,
            locals: vec![
                helper_return,
                local(1, MirLocalRole::Arg, MirTypeShape::U32),
                local(
                    2,
                    MirLocalRole::Temp,
                    MirTypeShape::Adt {
                        identity: TrustedDeviceItem::ThreadIndex.canonical_path().to_owned(),
                    },
                ),
                local(3, MirLocalRole::Temp, MirTypeShape::U32),
                local(
                    4,
                    MirLocalRole::Temp,
                    MirTypeShape::Adt {
                        identity: "core::option::Option".to_owned(),
                    },
                ),
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
                        destination: Some(place(2)),
                        operands: Vec::new(),
                    })),
                },
                MirBlock {
                    index: 1,
                    statements: vec![assign(
                        0,
                        3,
                        vec![operand(1), operand(1)],
                        MirRvalueKind::Binary(MirBinaryOp::Add),
                    )],
                    terminator: Some(terminator(MirTerminatorKind::Call {
                        callee: Some(MirCallee::checked_tiled_2d_for_test(64, 16, 16, 4)),
                        target: Some(2),
                        destination: Some(place(4)),
                        operands: vec![operand(2)],
                    })),
                },
                MirBlock {
                    index: 2,
                    statements: vec![assign(
                        0,
                        0,
                        vec![helper_result_constant],
                        MirRvalueKind::Use,
                    )],
                    terminator: Some(terminator(MirTerminatorKind::Return)),
                },
            ],
        };
        MirModule {
            functions: vec![root, helper],
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

    fn storage_statement(index: usize, kind: MirStatementKind, local: usize) -> MirStatement {
        MirStatement {
            index,
            kind,
            destination: Some(place(local)),
            operands: Vec::new(),
            rvalue: None,
            semantic_rvalue_type: None,
            operation: None,
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
