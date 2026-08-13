//! Source-authenticated admission for the collected scalar-control-flow V2 pilot.

use std::error::Error;
use std::fmt;

use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{EarlyBinder, Instance, InstanceKind, TyCtxt, TyKind, TypingEnv, UintTy};
use sha2::{Digest as _, Sha256};

use crate::AmdGpuTarget;
use crate::collector::{CollectedFunction, CollectedFunctionRole, CollectionResult};
use crate::scalar_mir_v2::EXACT_SCALAR_V2_TARGET;

pub(crate) const COLLECTED_SCALAR_CONTROL_FLOW_PIPELINE_V2: &str =
    "collected-executable-scalar-control-flow-v2";
pub(crate) const NEXT_LOWERING_DEPENDENCY: &str = "exact executable-MIR capture/import remains required before constructing any body-bound Scalar V1 lowering authority";
const FIXED_KERNEL_EXPORT: &str = "scalar_control_flow_v1";
const FIXED_LOGICAL_NAME: &str = "scalar_control_flow_v1";
const COMPILER_SEMANTICS_DOMAIN_V2: &[u8] = b"fe2o3.scalar-control-flow.compiler-semantics.v2";
const COLLECTED_AUTHORITY_DOMAIN_V2: &[u8] = b"fe2o3.scalar-control-flow.collected-authority.v2";
const REVIEWED_RUSTC_RELEASE: &str = "1.96.0-nightly";
const REVIEWED_RUSTC_COMMIT: &str = "55e86c996809902e8bbad512cfb4d2c18be446d9";
const REVIEWED_RUSTC_LLVM: &str = "22.1.2";
const PORTABLE_CLOSURE_IDENTITY: [u8; 32] = [
    0x47, 0xf3, 0xdd, 0x95, 0x22, 0x68, 0x91, 0x3b, 0x8f, 0xe9, 0xec, 0xe9, 0x94, 0x6c, 0x1a, 0x5f,
    0xaf, 0x42, 0xac, 0xbd, 0xe3, 0x54, 0x4d, 0xf5, 0x1b, 0x4d, 0xd8, 0x3e, 0xde, 0x6b, 0x03, 0x65,
];
const ROOT_CFG_IDENTITY: [u8; 32] = [
    0x9a, 0x21, 0x79, 0xa1, 0x3a, 0x84, 0x27, 0xe1, 0x7e, 0x22, 0x4b, 0xe6, 0x51, 0x68, 0xc3, 0x9b,
    0x34, 0xc2, 0x12, 0xcf, 0x9f, 0x10, 0x01, 0xa0, 0x3d, 0x04, 0x0b, 0xd6, 0xcd, 0x32, 0x48, 0x1c,
];
const HELPER_CFG_IDENTITY: [u8; 32] = [
    0xb0, 0x63, 0x4e, 0xea, 0x02, 0xb2, 0x3d, 0x62, 0xec, 0x50, 0xb4, 0x48, 0xd6, 0x32, 0xdf, 0x02,
    0xd5, 0x52, 0xc6, 0x25, 0x1b, 0x65, 0x42, 0x39, 0xd4, 0x1f, 0x64, 0xd3, 0x79, 0xab, 0x8e, 0x19,
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompilerSemanticsV2 {
    rustc_release: &'static str,
    rustc_commit: &'static str,
    llvm_version: &'static str,
    panic_strategy: String,
    overflow_checks: bool,
    optimize: String,
    debug_assertions: bool,
    mir_opt_level: usize,
    mir_enable_passes: Vec<(String, bool)>,
    llvm_args: Vec<String>,
    llvm_passes: Vec<String>,
    target_cpu: Option<String>,
    target_features: String,
    rustc_codegen_opt_level: String,
    remap_path_destinations: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExactSignatureV2 {
    KernelU32ToUnit,
    HelperU32ToU32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedFunctionV2<I> {
    identity: I,
    role: CollectedFunctionRole,
    signature: ExactSignatureV2,
    cfg_identity: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedClosureV2<I> {
    functions: Vec<ObservedFunctionV2<I>>,
    root_call_target: I,
    helper_call_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedCollectedScalarControlFlowV2 {
    kernel_export: String,
    root_instance_identity: String,
    helper_instance_identity: String,
    root_cfg_identity: [u8; 32],
    helper_cfg_identity: [u8; 32],
    portable_mir_semantic_commitment: [u8; 32],
    compiler_semantics_commitment: [u8; 32],
    authority_commitment: [u8; 32],
}

impl AuthenticatedCollectedScalarControlFlowV2 {
    pub(crate) fn kernel_export(&self) -> &str {
        &self.kernel_export
    }

    pub(crate) fn root_instance_identity(&self) -> &str {
        &self.root_instance_identity
    }

    pub(crate) fn helper_instance_identity(&self) -> &str {
        &self.helper_instance_identity
    }

    pub(crate) fn root_identity_hex(&self) -> String {
        encode_hex(&self.root_cfg_identity)
    }

    pub(crate) fn helper_identity_hex(&self) -> String {
        encode_hex(&self.helper_cfg_identity)
    }

    pub(crate) fn compiler_semantics_hex(&self) -> String {
        encode_hex(&self.compiler_semantics_commitment)
    }

    pub(crate) fn portable_mir_semantic_hex(&self) -> String {
        encode_hex(&self.portable_mir_semantic_commitment)
    }

    pub(crate) fn authority_hex(&self) -> String {
        encode_hex(&self.authority_commitment)
    }
}

#[derive(Debug)]
pub(crate) enum CollectedExecutableScalarControlFlowErrorV2 {
    WrongTarget {
        actual: String,
    },
    CustomPipeline,
    CompilerSemantics {
        detail: String,
    },
    UnsupportedCollection {
        detail: String,
    },
    IdentityMismatch {
        role: &'static str,
        expected: [u8; 32],
        actual: [u8; 32],
    },
    AbiMismatch {
        role: &'static str,
        detail: String,
    },
    CallTargetSubstitution,
    PortableMir {
        detail: String,
    },
}

impl fmt::Display for CollectedExecutableScalarControlFlowErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongTarget { actual } => write!(
                formatter,
                "collected scalar-control-flow V2 requires exact target `{EXACT_SCALAR_V2_TARGET}`, found `{actual}`"
            ),
            Self::CustomPipeline => formatter.write_str(
                "collected scalar-control-flow V2 rejects custom LLVM pipeline selection",
            ),
            Self::CompilerSemantics { detail } => write!(
                formatter,
                "collected scalar-control-flow V2 compiler semantics mismatch: {detail}"
            ),
            Self::UnsupportedCollection { detail } => write!(
                formatter,
                "unsupported collected scalar-control-flow V2 shape: {detail}"
            ),
            Self::IdentityMismatch {
                role,
                expected,
                actual,
            } => write!(
                formatter,
                "collected scalar-control-flow V2 {role} MIR identity mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::AbiMismatch { role, detail } => write!(
                formatter,
                "collected scalar-control-flow V2 {role} ABI mismatch: {detail}"
            ),
            Self::CallTargetSubstitution => formatter.write_str(
                "collected scalar-control-flow V2 root call does not target the exact collected helper instance",
            ),
            Self::PortableMir { detail } => write!(
                formatter,
                "collected scalar-control-flow V2 portable MIR rejected: {detail}"
            ),
        }
    }
}

impl Error for CollectedExecutableScalarControlFlowErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

pub(crate) fn authenticate_collected_executable_scalar_control_flow_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
    custom_llvm_pipeline: bool,
) -> Result<AuthenticatedCollectedScalarControlFlowV2, CollectedExecutableScalarControlFlowErrorV2>
{
    admit_execution_context(target.as_str(), custom_llvm_pipeline)?;
    let compiler_semantics = observe_compiler_semantics(tcx);
    let compiler_semantics_commitment = require_compiler_semantics(&compiler_semantics)?;

    let (root, helper) = exact_collected_pair(&collection.functions)?;
    if root.export_name != FIXED_KERNEL_EXPORT
        || root.logical_name.as_deref() != Some(FIXED_LOGICAL_NAME)
        || root.typed_profile.is_some()
        || root.kernel_binding.is_some()
        || root.typed_layout_identities.is_some()
        || root.general_typed_contract.is_some()
        || root.frontend_contract.is_some()
    {
        return Err(unsupported_collection(
            "kernel registration metadata is not the fixed untyped scalar_control_flow_v1 profile",
        ));
    }

    let observed = observe_closure(tcx, root, helper)?;
    admit_observed_closure(&observed)?;
    let portable_identity = crate::mir_import::import_collection(tcx, collection)
        .and_then(|module| module.collected_scalar_control_flow_digest_v2(FIXED_KERNEL_EXPORT))
        .map_err(
            |error| CollectedExecutableScalarControlFlowErrorV2::PortableMir {
                detail: error.to_string(),
            },
        )?;
    let portable_mir_semantic_commitment = *portable_identity.as_bytes();
    require_portable_closure_identity(portable_mir_semantic_commitment)?;
    let root_instance_identity = tcx.def_path_str(root.instance.def_id());
    let helper_instance_identity = tcx.def_path_str(helper.instance.def_id());
    let authority_commitment = collected_authority_commitment(
        portable_mir_semantic_commitment,
        observed.functions[0].cfg_identity,
        observed.functions[1].cfg_identity,
        compiler_semantics_commitment,
        &root_instance_identity,
        &helper_instance_identity,
        &root.export_name,
    );
    Ok(AuthenticatedCollectedScalarControlFlowV2 {
        kernel_export: root.export_name.clone(),
        root_instance_identity,
        helper_instance_identity,
        root_cfg_identity: observed.functions[0].cfg_identity,
        helper_cfg_identity: observed.functions[1].cfg_identity,
        portable_mir_semantic_commitment,
        compiler_semantics_commitment,
        authority_commitment,
    })
}

fn admit_execution_context(
    target: &str,
    custom_llvm_pipeline: bool,
) -> Result<(), CollectedExecutableScalarControlFlowErrorV2> {
    if target != EXACT_SCALAR_V2_TARGET {
        return Err(CollectedExecutableScalarControlFlowErrorV2::WrongTarget {
            actual: target.to_owned(),
        });
    }
    if custom_llvm_pipeline {
        return Err(CollectedExecutableScalarControlFlowErrorV2::CustomPipeline);
    }
    Ok(())
}

fn observe_compiler_semantics(tcx: TyCtxt<'_>) -> CompilerSemanticsV2 {
    CompilerSemanticsV2 {
        rustc_release: env!("FE2O3_BUILD_RUSTC_RELEASE"),
        rustc_commit: env!("FE2O3_BUILD_RUSTC_COMMIT"),
        llvm_version: env!("FE2O3_BUILD_RUSTC_LLVM"),
        panic_strategy: format!("{:?}", tcx.sess.panic_strategy()),
        overflow_checks: tcx.sess.overflow_checks(),
        optimize: format!("{:?}", tcx.sess.opts.optimize),
        debug_assertions: tcx.sess.opts.debug_assertions,
        mir_opt_level: tcx.sess.mir_opt_level(),
        mir_enable_passes: tcx.sess.opts.unstable_opts.mir_enable_passes.clone(),
        llvm_args: tcx.sess.opts.cg.llvm_args.clone(),
        llvm_passes: tcx.sess.opts.cg.passes.clone(),
        target_cpu: tcx.sess.opts.cg.target_cpu.clone(),
        target_features: tcx.sess.opts.cg.target_feature.clone(),
        rustc_codegen_opt_level: tcx.sess.opts.cg.opt_level.clone(),
        remap_path_destinations: tcx
            .sess
            .opts
            .remap_path_prefix
            .iter()
            .map(|(_, destination)| destination.display().to_string())
            .collect(),
    }
}

fn require_compiler_semantics(
    observed: &CompilerSemanticsV2,
) -> Result<[u8; 32], CollectedExecutableScalarControlFlowErrorV2> {
    let expected_mir_passes = [("JumpThreading".to_owned(), false)];
    let mismatch = if observed.rustc_release != REVIEWED_RUSTC_RELEASE {
        Some(format!(
            "rustc release must be {REVIEWED_RUSTC_RELEASE}, found {}",
            observed.rustc_release
        ))
    } else if observed.rustc_commit != REVIEWED_RUSTC_COMMIT {
        Some(format!(
            "rustc commit must be {REVIEWED_RUSTC_COMMIT}, found {}",
            observed.rustc_commit
        ))
    } else if observed.llvm_version != REVIEWED_RUSTC_LLVM {
        Some(format!(
            "rustc LLVM must be {REVIEWED_RUSTC_LLVM}, found {}",
            observed.llvm_version
        ))
    } else if observed.panic_strategy != "Unwind" {
        Some(format!(
            "panic strategy must be Unwind, found {}",
            observed.panic_strategy
        ))
    } else if observed.overflow_checks {
        Some("overflow checks must be disabled".to_owned())
    } else if observed.optimize != "No" || observed.rustc_codegen_opt_level != "0" {
        Some(format!(
            "rustc optimization must be No/0, found {}/{}",
            observed.optimize, observed.rustc_codegen_opt_level
        ))
    } else if !observed.debug_assertions {
        Some("debug assertions must be enabled".to_owned())
    } else if observed.mir_opt_level != 1 {
        Some(format!(
            "effective MIR optimization level must be 1, found {}",
            observed.mir_opt_level
        ))
    } else if observed.mir_enable_passes != expected_mir_passes {
        Some(format!(
            "MIR pass overrides must be exactly -JumpThreading, found {:?}",
            observed.mir_enable_passes
        ))
    } else if !observed.llvm_args.is_empty() || !observed.llvm_passes.is_empty() {
        Some("custom LLVM arguments or passes are forbidden".to_owned())
    } else if observed.target_cpu.is_some() || !observed.target_features.is_empty() {
        Some(format!(
            "rustc target CPU/features must be unset, found {:?}/{:?}",
            observed.target_cpu, observed.target_features
        ))
    } else if observed.remap_path_destinations
        != ["/fe2o3-reviewed-workspace/scalar-control-flow-v1.rs"]
    {
        Some(format!(
            "source remapping must contain exactly one canonical fixture destination, found {:?}",
            observed.remap_path_destinations
        ))
    } else {
        None
    };
    if let Some(detail) = mismatch {
        return Err(CollectedExecutableScalarControlFlowErrorV2::CompilerSemantics { detail });
    }

    let mut digest = Sha256::new();
    hash_field(&mut digest, COMPILER_SEMANTICS_DOMAIN_V2);
    hash_field(&mut digest, observed.rustc_release.as_bytes());
    hash_field(&mut digest, observed.rustc_commit.as_bytes());
    hash_field(&mut digest, observed.llvm_version.as_bytes());
    hash_field(&mut digest, observed.panic_strategy.as_bytes());
    hash_field(&mut digest, &[u8::from(observed.overflow_checks)]);
    hash_field(&mut digest, observed.optimize.as_bytes());
    hash_field(&mut digest, &[u8::from(observed.debug_assertions)]);
    hash_field(&mut digest, &(observed.mir_opt_level as u64).to_le_bytes());
    for (name, enabled) in &observed.mir_enable_passes {
        hash_field(&mut digest, name.as_bytes());
        hash_field(&mut digest, &[u8::from(*enabled)]);
    }
    for argument in &observed.llvm_args {
        hash_field(&mut digest, argument.as_bytes());
    }
    for pass in &observed.llvm_passes {
        hash_field(&mut digest, pass.as_bytes());
    }
    match &observed.target_cpu {
        Some(cpu) => {
            hash_field(&mut digest, &[1]);
            hash_field(&mut digest, cpu.as_bytes());
        }
        None => hash_field(&mut digest, &[0]),
    }
    hash_field(&mut digest, observed.target_features.as_bytes());
    hash_field(&mut digest, observed.rustc_codegen_opt_level.as_bytes());
    hash_field(&mut digest, observed.remap_path_destinations[0].as_bytes());
    Ok(digest.finalize().into())
}

fn collected_authority_commitment(
    portable_identity: [u8; 32],
    root_cfg_identity: [u8; 32],
    helper_cfg_identity: [u8; 32],
    compiler_semantics: [u8; 32],
    root_instance_identity: &str,
    helper_instance_identity: &str,
    kernel_export: &str,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, COLLECTED_AUTHORITY_DOMAIN_V2);
    hash_field(&mut digest, &portable_identity);
    hash_field(&mut digest, &root_cfg_identity);
    hash_field(&mut digest, &helper_cfg_identity);
    hash_field(&mut digest, &compiler_semantics);
    hash_field(&mut digest, root_instance_identity.as_bytes());
    hash_field(&mut digest, helper_instance_identity.as_bytes());
    hash_field(&mut digest, kernel_export.as_bytes());
    digest.finalize().into()
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn exact_collected_pair<'a, 'tcx>(
    functions: &'a [CollectedFunction<'tcx>],
) -> Result<
    (&'a CollectedFunction<'tcx>, &'a CollectedFunction<'tcx>),
    CollectedExecutableScalarControlFlowErrorV2,
> {
    if functions.len() != 2 {
        return Err(unsupported_collection(format!(
            "requires exactly two collected functions, found {}",
            functions.len()
        )));
    }
    let roots = functions
        .iter()
        .filter(|function| function.role == CollectedFunctionRole::KernelEntry)
        .collect::<Vec<_>>();
    let helpers = functions
        .iter()
        .filter(|function| function.role == CollectedFunctionRole::InternalHelper)
        .collect::<Vec<_>>();
    if roots.len() != 1 || helpers.len() != 1 {
        return Err(unsupported_collection(format!(
            "requires one authenticated kernel entry and one internal helper, found {} root(s) and {} helper(s)",
            roots.len(),
            helpers.len()
        )));
    }
    Ok((roots[0], helpers[0]))
}

fn observe_closure<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: &CollectedFunction<'tcx>,
    helper: &CollectedFunction<'tcx>,
) -> Result<ObservedClosureV2<Instance<'tcx>>, CollectedExecutableScalarControlFlowErrorV2> {
    let root_cfg = collected_cfg_identity(root, "root")?;
    let helper_cfg = collected_cfg_identity(helper, "helper")?;
    let root_signature = exact_signature(tcx, root.instance, "root")?;
    let helper_signature = exact_signature(tcx, helper.instance, "helper")?;
    let root_calls = direct_calls(tcx, root.instance)?;
    if root_calls.len() != 1 {
        return Err(unsupported_collection(format!(
            "root requires exactly one direct call, found {}",
            root_calls.len()
        )));
    }
    let helper_calls = direct_calls(tcx, helper.instance)?;
    Ok(ObservedClosureV2 {
        functions: vec![
            ObservedFunctionV2 {
                identity: root.instance,
                role: root.role,
                signature: root_signature,
                cfg_identity: root_cfg,
            },
            ObservedFunctionV2 {
                identity: helper.instance,
                role: helper.role,
                signature: helper_signature,
                cfg_identity: helper_cfg,
            },
        ],
        root_call_target: root_calls[0],
        helper_call_count: helper_calls.len(),
    })
}

fn admit_observed_closure<I: Eq>(
    observed: &ObservedClosureV2<I>,
) -> Result<(), CollectedExecutableScalarControlFlowErrorV2> {
    if observed.functions.len() != 2 {
        return Err(unsupported_collection(format!(
            "requires exactly two observed functions, found {}",
            observed.functions.len()
        )));
    }
    let root = observed
        .functions
        .iter()
        .find(|function| function.role == CollectedFunctionRole::KernelEntry)
        .ok_or_else(|| unsupported_collection("missing authenticated kernel root"))?;
    let helper = observed
        .functions
        .iter()
        .find(|function| function.role == CollectedFunctionRole::InternalHelper)
        .ok_or_else(|| unsupported_collection("missing exact internal helper"))?;
    if observed.functions.iter().any(|function| {
        !matches!(
            function.role,
            CollectedFunctionRole::KernelEntry | CollectedFunctionRole::InternalHelper
        )
    }) {
        return Err(unsupported_collection(
            "device FFI exports and other collected roles are not admitted",
        ));
    }
    require_signature(root, ExactSignatureV2::KernelU32ToUnit, "root")?;
    require_signature(helper, ExactSignatureV2::HelperU32ToU32, "helper")?;
    require_cfg_identity(root, ROOT_CFG_IDENTITY, "root")?;
    require_cfg_identity(helper, HELPER_CFG_IDENTITY, "helper")?;
    if observed.root_call_target != helper.identity {
        return Err(CollectedExecutableScalarControlFlowErrorV2::CallTargetSubstitution);
    }
    if observed.helper_call_count != 0 {
        return Err(unsupported_collection(format!(
            "fixed helper must have no direct calls, found {}",
            observed.helper_call_count
        )));
    }
    Ok(())
}

fn require_signature<I>(
    function: &ObservedFunctionV2<I>,
    expected: ExactSignatureV2,
    role: &'static str,
) -> Result<(), CollectedExecutableScalarControlFlowErrorV2> {
    if function.signature != expected {
        return Err(CollectedExecutableScalarControlFlowErrorV2::AbiMismatch {
            role,
            detail: format!("expected {expected:?}, found {:?}", function.signature),
        });
    }
    Ok(())
}

fn require_portable_closure_identity(
    actual: [u8; 32],
) -> Result<(), CollectedExecutableScalarControlFlowErrorV2> {
    if actual != PORTABLE_CLOSURE_IDENTITY {
        return Err(
            CollectedExecutableScalarControlFlowErrorV2::IdentityMismatch {
                role: "portable closure",
                expected: PORTABLE_CLOSURE_IDENTITY,
                actual,
            },
        );
    }
    Ok(())
}

fn exact_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    role: &'static str,
) -> Result<ExactSignatureV2, CollectedExecutableScalarControlFlowErrorV2> {
    if !matches!(instance.def, InstanceKind::Item(_)) {
        return Err(abi_mismatch(
            role,
            format!("expected an ordinary item, found {:?}", instance.def),
        ));
    }
    if !instance.args.is_empty() {
        return Err(abi_mismatch(
            role,
            "fixed scalar-control-flow V2 does not admit generic instances",
        ));
    }
    let signature = tcx
        .try_instantiate_and_normalize_erasing_regions(
            instance.args,
            TypingEnv::fully_monomorphized(),
            tcx.fn_sig(instance.def_id()),
        )
        .map_err(|_| abi_mismatch(role, "signature normalization failed"))?;
    let signature = tcx.instantiate_bound_regions_with_erased(signature);
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.inputs().len() != 1
        || !matches!(signature.inputs()[0].kind(), TyKind::Uint(UintTy::U32))
    {
        return Err(abi_mismatch(
            role,
            format!("expected safe non-variadic Rust ABI with one u32 argument, found {signature}"),
        ));
    }
    if signature.output() == tcx.types.unit {
        Ok(ExactSignatureV2::KernelU32ToUnit)
    } else if matches!(signature.output().kind(), TyKind::Uint(UintTy::U32)) {
        Ok(ExactSignatureV2::HelperU32ToU32)
    } else {
        Err(abi_mismatch(
            role,
            format!("expected unit or u32 result, found {signature}"),
        ))
    }
}

fn direct_calls<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
) -> Result<Vec<Instance<'tcx>>, CollectedExecutableScalarControlFlowErrorV2> {
    if !tcx.is_mir_available(caller.def_id()) {
        return Err(unsupported_collection("collected function has no MIR"));
    }
    let body = tcx.instance_mir(caller.def);
    let mut calls = Vec::new();
    for block in body.basic_blocks.iter() {
        let Some(terminator) = &block.terminator else {
            return Err(unsupported_collection(
                "collected MIR contains a block without a terminator",
            ));
        };
        let TerminatorKind::Call { func, .. } = &terminator.kind else {
            continue;
        };
        let Operand::Constant(constant) = func else {
            return Err(unsupported_collection(
                "root/helper closure contains an indirect call",
            ));
        };
        let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
            return Err(unsupported_collection(
                "call operand is not an exact function definition",
            ));
        };
        let args = tcx.instantiate_and_normalize_erasing_regions(
            caller.args,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(*args),
        );
        let resolved = Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, args)
            .map_err(|_| unsupported_collection("direct call normalization failed"))?
            .ok_or_else(|| unsupported_collection("direct call did not resolve to an instance"))?;
        calls.push(resolved);
    }
    Ok(calls)
}

fn collected_cfg_identity(
    function: &CollectedFunction<'_>,
    role: &'static str,
) -> Result<[u8; 32], CollectedExecutableScalarControlFlowErrorV2> {
    function
        .dead_branches
        .as_ref()
        .map(|observation| observation.evidence().context().cfg_identity())
        .ok_or_else(|| unsupported_collection(format!("{role} has no compiler MIR observation")))
}

fn require_cfg_identity<I>(
    function: &ObservedFunctionV2<I>,
    expected: [u8; 32],
    role: &'static str,
) -> Result<(), CollectedExecutableScalarControlFlowErrorV2> {
    if function.cfg_identity != expected {
        return Err(
            CollectedExecutableScalarControlFlowErrorV2::IdentityMismatch {
                role,
                expected,
                actual: function.cfg_identity,
            },
        );
    }
    Ok(())
}

fn unsupported_collection(
    detail: impl Into<String>,
) -> CollectedExecutableScalarControlFlowErrorV2 {
    CollectedExecutableScalarControlFlowErrorV2::UnsupportedCollection {
        detail: detail.into(),
    }
}

fn abi_mismatch(
    role: &'static str,
    detail: impl Into<String>,
) -> CollectedExecutableScalarControlFlowErrorV2 {
    CollectedExecutableScalarControlFlowErrorV2::AbiMismatch {
        role,
        detail: detail.into(),
    }
}

fn encode_hex(bytes: &[u8; 32]) -> String {
    use fmt::Write as _;

    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiler_semantics() -> CompilerSemanticsV2 {
        CompilerSemanticsV2 {
            rustc_release: REVIEWED_RUSTC_RELEASE,
            rustc_commit: REVIEWED_RUSTC_COMMIT,
            llvm_version: REVIEWED_RUSTC_LLVM,
            panic_strategy: "Unwind".to_owned(),
            overflow_checks: false,
            optimize: "No".to_owned(),
            debug_assertions: true,
            mir_opt_level: 1,
            mir_enable_passes: vec![("JumpThreading".to_owned(), false)],
            llvm_args: Vec::new(),
            llvm_passes: Vec::new(),
            target_cpu: None,
            target_features: String::new(),
            rustc_codegen_opt_level: "0".to_owned(),
            remap_path_destinations: vec![
                "/fe2o3-reviewed-workspace/scalar-control-flow-v1.rs".to_owned(),
            ],
        }
    }

    fn observed() -> ObservedClosureV2<u8> {
        ObservedClosureV2 {
            functions: vec![
                ObservedFunctionV2 {
                    identity: 1,
                    role: CollectedFunctionRole::KernelEntry,
                    signature: ExactSignatureV2::KernelU32ToUnit,
                    cfg_identity: ROOT_CFG_IDENTITY,
                },
                ObservedFunctionV2 {
                    identity: 2,
                    role: CollectedFunctionRole::InternalHelper,
                    signature: ExactSignatureV2::HelperU32ToU32,
                    cfg_identity: HELPER_CFG_IDENTITY,
                },
            ],
            root_call_target: 2,
            helper_call_count: 0,
        }
    }

    #[test]
    fn exact_observation_is_admitted_without_constructing_an_export() {
        admit_observed_closure(&observed()).unwrap();
    }

    #[test]
    fn unsupported_collection_shapes_fail_closed() {
        let mut missing = observed();
        missing.functions.pop();
        assert!(matches!(
            admit_observed_closure(&missing),
            Err(CollectedExecutableScalarControlFlowErrorV2::UnsupportedCollection { .. })
        ));

        let mut additional = observed();
        additional.functions.push(ObservedFunctionV2 {
            identity: 3,
            role: CollectedFunctionRole::DeviceFfiExport,
            signature: ExactSignatureV2::HelperU32ToU32,
            cfg_identity: HELPER_CFG_IDENTITY,
        });
        assert!(matches!(
            admit_observed_closure(&additional),
            Err(CollectedExecutableScalarControlFlowErrorV2::UnsupportedCollection { .. })
        ));

        let mut wrong_role = observed();
        wrong_role.functions[0].role = CollectedFunctionRole::DeviceFfiExport;
        assert!(matches!(
            admit_observed_closure(&wrong_role),
            Err(CollectedExecutableScalarControlFlowErrorV2::UnsupportedCollection { .. })
        ));
    }

    #[test]
    fn role_identity_signature_and_call_substitutions_are_distinct_rejections() {
        let mut wrong_signature = observed();
        wrong_signature.functions[1].signature = ExactSignatureV2::KernelU32ToUnit;
        assert!(matches!(
            admit_observed_closure(&wrong_signature),
            Err(CollectedExecutableScalarControlFlowErrorV2::AbiMismatch { role: "helper", .. })
        ));

        let mut substituted = observed();
        substituted.root_call_target = 3;
        assert!(matches!(
            admit_observed_closure(&substituted),
            Err(CollectedExecutableScalarControlFlowErrorV2::CallTargetSubstitution)
        ));

        let mut changed_mir = observed();
        changed_mir.functions[0].cfg_identity[0] ^= 1;
        assert!(matches!(
            admit_observed_closure(&changed_mir),
            Err(CollectedExecutableScalarControlFlowErrorV2::IdentityMismatch { role: "root", .. })
        ));
    }

    #[test]
    fn helper_calls_are_not_silently_added_to_the_fixed_closure() {
        let mut observed = observed();
        observed.helper_call_count = 1;
        assert_eq!(
            admit_observed_closure(&observed).unwrap_err().to_string(),
            "unsupported collected scalar-control-flow V2 shape: fixed helper must have no direct calls, found 1"
        );
    }

    #[test]
    fn portable_closure_identity_substitutions_fail_closed() {
        require_portable_closure_identity(PORTABLE_CLOSURE_IDENTITY).unwrap();
        for byte in [0, 7, 31] {
            let mut changed_body = PORTABLE_CLOSURE_IDENTITY;
            changed_body[byte] ^= 1;
            assert!(matches!(
                require_portable_closure_identity(changed_body),
                Err(
                    CollectedExecutableScalarControlFlowErrorV2::IdentityMismatch {
                        role: "portable closure",
                        ..
                    }
                )
            ));
        }
    }

    #[test]
    fn target_and_pipeline_are_exact_vetoes() {
        admit_execution_context(EXACT_SCALAR_V2_TARGET, false).unwrap();
        assert_eq!(
            admit_execution_context("gfx942:xnack+", false)
                .unwrap_err()
                .to_string(),
            "collected scalar-control-flow V2 requires exact target `gfx942:xnack-`, found `gfx942:xnack+`"
        );
        assert_eq!(
            admit_execution_context(EXACT_SCALAR_V2_TARGET, true)
                .unwrap_err()
                .to_string(),
            "collected scalar-control-flow V2 rejects custom LLVM pipeline selection"
        );
    }

    #[test]
    fn compiler_semantics_commitment_rejects_mir_and_codegen_mutations() {
        let baseline = require_compiler_semantics(&compiler_semantics()).unwrap();
        assert_eq!(
            baseline,
            require_compiler_semantics(&compiler_semantics()).unwrap()
        );

        let mut panic_abort = compiler_semantics();
        panic_abort.panic_strategy = "Abort".to_owned();
        assert!(matches!(
            require_compiler_semantics(&panic_abort),
            Err(CollectedExecutableScalarControlFlowErrorV2::CompilerSemantics { .. })
        ));

        let mut changed_mir_passes = compiler_semantics();
        changed_mir_passes.mir_enable_passes.clear();
        assert!(require_compiler_semantics(&changed_mir_passes).is_err());

        let mut optimized = compiler_semantics();
        optimized.optimize = "More".to_owned();
        optimized.rustc_codegen_opt_level = "2".to_owned();
        assert!(require_compiler_semantics(&optimized).is_err());

        let mut target_cpu = compiler_semantics();
        target_cpu.target_cpu = Some("native".to_owned());
        assert!(require_compiler_semantics(&target_cpu).is_err());

        let mut target_features = compiler_semantics();
        target_features.target_features = "+avx2".to_owned();
        assert!(require_compiler_semantics(&target_features).is_err());

        let mut llvm_argument = compiler_semantics();
        llvm_argument.llvm_args.push("-verify-each".to_owned());
        assert!(require_compiler_semantics(&llvm_argument).is_err());

        let mut overflow_checks = compiler_semantics();
        overflow_checks.overflow_checks = true;
        assert!(require_compiler_semantics(&overflow_checks).is_err());

        let mut debug_assertions = compiler_semantics();
        debug_assertions.debug_assertions = false;
        assert!(require_compiler_semantics(&debug_assertions).is_err());

        let mut remap_substitution = compiler_semantics();
        remap_substitution.remap_path_destinations = vec!["/attacker.rs".to_owned()];
        assert!(require_compiler_semantics(&remap_substitution).is_err());

        let mut extra_remap = compiler_semantics();
        extra_remap
            .remap_path_destinations
            .push("/attacker.rs".to_owned());
        assert!(require_compiler_semantics(&extra_remap).is_err());

        let mut different_compiler = compiler_semantics();
        different_compiler.rustc_commit = "0000000000000000000000000000000000000000";
        assert!(require_compiler_semantics(&different_compiler).is_err());
    }
}
