//! Identity-bound collected entry for the executable scalar-control-flow pilot.

use std::error::Error;
use std::fmt;

use dialect_amdgcn::{LoweringErrors, lower_compiler_module_to_gfx942_llvm_ir};
use dialect_mir::{MirExecutableDecodeError, MirExecutableModule};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Function, FunctionRole, Kernel, LaunchDomain, LaunchExtent, Module,
    Operation, OperationKind, ScalarType, Signature, TargetCapability, Terminator, Type, ValueDef,
    ValueId, VerificationErrors, WaveWidth, WorkgroupSize, verify_module,
};
use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{EarlyBinder, Instance, InstanceKind, TyCtxt, TyKind, TypingEnv, UintTy};

use crate::AmdGpuTarget;
use crate::collector::{CollectedFunction, CollectedFunctionRole, CollectionResult};
use crate::executable_scalar_control_flow_v1::{
    ExecutableScalarControlFlowArtifactV1, ExecutableScalarControlFlowErrorV1,
    lower_executable_scalar_control_flow_v1,
};
use crate::scalar_mir_v2::EXACT_SCALAR_V2_TARGET;

pub(crate) const COLLECTED_SCALAR_CONTROL_FLOW_PIPELINE_V1: &str =
    "executable-scalar-control-flow-v1";
const FIXED_KERNEL_ID: &str = "scalar_control_flow_v1";
const FIXED_ENTRY_ID: &str = "__fe2o3_scalar_control_flow_v1_entry";
const FIXED_HELPER_ID: &str = "__fe2o3_scalar_control_flow_v1_helper";
const FIXED_LOGICAL_NAME: &str = "scalar_control_flow_v1";
const FIXED_WORKGROUP_X: u32 = 64;
const FIXED_HELPER_MIR: &str =
    include_str!("../../dialect-mir/tests/fixtures/nested-loop.mir.json");

const ROOT_CFG_IDENTITY: [u8; 32] = [
    0xa3, 0xb2, 0xfb, 0x44, 0x1d, 0x62, 0x53, 0x21, 0x26, 0xd4, 0x4c, 0x05, 0x1c, 0xa1, 0x62, 0x56,
    0xb2, 0xbe, 0x79, 0xf5, 0x9b, 0x26, 0xef, 0x4d, 0x20, 0x51, 0x0b, 0xcd, 0x33, 0x1b, 0xa6, 0x21,
];
const HELPER_CFG_IDENTITY: [u8; 32] = [
    0xbd, 0xed, 0x0e, 0x3d, 0xa6, 0x30, 0x3a, 0x42, 0xb6, 0x3c, 0xd2, 0x35, 0x28, 0x4c, 0xce, 0xc6,
    0xa0, 0x3e, 0x92, 0xb7, 0xb2, 0xf5, 0xc4, 0xe7, 0x6b, 0x41, 0x29, 0x85, 0x97, 0xe8, 0x6c, 0x2d,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExactSignatureV1 {
    KernelU32ToUnit,
    HelperU32ToU32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedFunctionV1<I> {
    identity: I,
    role: CollectedFunctionRole,
    signature: ExactSignatureV1,
    cfg_identity: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedClosureV1<I> {
    functions: Vec<ObservedFunctionV1<I>>,
    root_call_target: I,
    helper_call_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CollectedExecutableScalarControlFlowArtifactV1 {
    pub(crate) helper: ExecutableScalarControlFlowArtifactV1,
    pub(crate) kernel_ir: Module,
    pub(crate) gfx942_llvm: String,
}

#[derive(Debug)]
pub(crate) enum CollectedExecutableScalarControlFlowErrorV1 {
    WrongTarget {
        actual: String,
    },
    CustomPipeline,
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
    FixtureDecode(MirExecutableDecodeError),
    Helper(ExecutableScalarControlFlowErrorV1),
    InvalidKernelIr(VerificationErrors),
    Backend(LoweringErrors),
}

impl fmt::Display for CollectedExecutableScalarControlFlowErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongTarget { actual } => write!(
                formatter,
                "collected scalar-control-flow V1 requires exact target `{EXACT_SCALAR_V2_TARGET}`, found `{actual}`"
            ),
            Self::CustomPipeline => formatter.write_str(
                "collected scalar-control-flow V1 rejects custom LLVM pipeline selection",
            ),
            Self::UnsupportedCollection { detail } => write!(
                formatter,
                "unsupported collected scalar-control-flow V1 shape: {detail}"
            ),
            Self::IdentityMismatch {
                role,
                expected,
                actual,
            } => write!(
                formatter,
                "collected scalar-control-flow V1 {role} MIR identity mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::AbiMismatch { role, detail } => write!(
                formatter,
                "collected scalar-control-flow V1 {role} ABI mismatch: {detail}"
            ),
            Self::CallTargetSubstitution => formatter.write_str(
                "collected scalar-control-flow V1 root call does not target the exact collected helper instance",
            ),
            Self::FixtureDecode(error) => write!(
                formatter,
                "fixed scalar-control-flow V1 executable-MIR fixture is invalid: {error}"
            ),
            Self::Helper(error) => write!(
                formatter,
                "fixed scalar-control-flow V1 helper lowering failed: {error}"
            ),
            Self::InvalidKernelIr(error) => write!(
                formatter,
                "collected scalar-control-flow V1 Kernel IR is invalid: {error}"
            ),
            Self::Backend(error) => write!(
                formatter,
                "collected scalar-control-flow V1 direct gfx942 LLVM lowering failed: {error}"
            ),
        }
    }
}

impl Error for CollectedExecutableScalarControlFlowErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FixtureDecode(error) => Some(error),
            Self::Helper(error) => Some(error),
            Self::InvalidKernelIr(error) => Some(error),
            Self::Backend(error) => Some(error),
            Self::WrongTarget { .. }
            | Self::CustomPipeline
            | Self::UnsupportedCollection { .. }
            | Self::IdentityMismatch { .. }
            | Self::AbiMismatch { .. }
            | Self::CallTargetSubstitution => None,
        }
    }
}

pub(crate) fn lower_collected_executable_scalar_control_flow_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
    custom_llvm_pipeline: bool,
) -> Result<
    CollectedExecutableScalarControlFlowArtifactV1,
    CollectedExecutableScalarControlFlowErrorV1,
> {
    admit_execution_context(target.as_str(), custom_llvm_pipeline)?;

    let (root, helper) = exact_collected_pair(&collection.functions)?;
    if root.export_name != FIXED_KERNEL_ID
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
    lower_fixed_helper_and_compose_kernel()
}

fn admit_execution_context(
    target: &str,
    custom_llvm_pipeline: bool,
) -> Result<(), CollectedExecutableScalarControlFlowErrorV1> {
    if target != EXACT_SCALAR_V2_TARGET {
        return Err(CollectedExecutableScalarControlFlowErrorV1::WrongTarget {
            actual: target.to_owned(),
        });
    }
    if custom_llvm_pipeline {
        return Err(CollectedExecutableScalarControlFlowErrorV1::CustomPipeline);
    }
    Ok(())
}

fn exact_collected_pair<'a, 'tcx>(
    functions: &'a [CollectedFunction<'tcx>],
) -> Result<
    (&'a CollectedFunction<'tcx>, &'a CollectedFunction<'tcx>),
    CollectedExecutableScalarControlFlowErrorV1,
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
) -> Result<ObservedClosureV1<Instance<'tcx>>, CollectedExecutableScalarControlFlowErrorV1> {
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
    Ok(ObservedClosureV1 {
        functions: vec![
            ObservedFunctionV1 {
                identity: root.instance,
                role: root.role,
                signature: root_signature,
                cfg_identity: root_cfg,
            },
            ObservedFunctionV1 {
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
    observed: &ObservedClosureV1<I>,
) -> Result<(), CollectedExecutableScalarControlFlowErrorV1> {
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
    require_signature(root, ExactSignatureV1::KernelU32ToUnit, "root")?;
    require_signature(helper, ExactSignatureV1::HelperU32ToU32, "helper")?;
    require_cfg_identity(root, ROOT_CFG_IDENTITY, "root")?;
    require_cfg_identity(helper, HELPER_CFG_IDENTITY, "helper")?;
    if observed.root_call_target != helper.identity {
        return Err(CollectedExecutableScalarControlFlowErrorV1::CallTargetSubstitution);
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
    function: &ObservedFunctionV1<I>,
    expected: ExactSignatureV1,
    role: &'static str,
) -> Result<(), CollectedExecutableScalarControlFlowErrorV1> {
    if function.signature != expected {
        return Err(CollectedExecutableScalarControlFlowErrorV1::AbiMismatch {
            role,
            detail: format!("expected {expected:?}, found {:?}", function.signature),
        });
    }
    Ok(())
}

fn require_cfg_identity<I>(
    function: &ObservedFunctionV1<I>,
    expected: [u8; 32],
    role: &'static str,
) -> Result<(), CollectedExecutableScalarControlFlowErrorV1> {
    if function.cfg_identity != expected {
        return Err(
            CollectedExecutableScalarControlFlowErrorV1::IdentityMismatch {
                role,
                expected,
                actual: function.cfg_identity,
            },
        );
    }
    Ok(())
}

fn exact_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    role: &'static str,
) -> Result<ExactSignatureV1, CollectedExecutableScalarControlFlowErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_)) {
        return Err(abi_mismatch(
            role,
            format!("expected an ordinary item, found {:?}", instance.def),
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
        Ok(ExactSignatureV1::KernelU32ToUnit)
    } else if matches!(signature.output().kind(), TyKind::Uint(UintTy::U32)) {
        Ok(ExactSignatureV1::HelperU32ToU32)
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
) -> Result<Vec<Instance<'tcx>>, CollectedExecutableScalarControlFlowErrorV1> {
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
) -> Result<[u8; 32], CollectedExecutableScalarControlFlowErrorV1> {
    function
        .dead_branches
        .as_ref()
        .map(|observation| observation.evidence().context().cfg_identity())
        .ok_or_else(|| unsupported_collection(format!("{role} has no compiler MIR observation")))
}

fn lower_fixed_helper_and_compose_kernel() -> Result<
    CollectedExecutableScalarControlFlowArtifactV1,
    CollectedExecutableScalarControlFlowErrorV1,
> {
    let executable = MirExecutableModule::from_canonical_text(FIXED_HELPER_MIR)
        .map_err(CollectedExecutableScalarControlFlowErrorV1::FixtureDecode)?;
    let mut helper = lower_executable_scalar_control_flow_v1(&executable)
        .map_err(CollectedExecutableScalarControlFlowErrorV1::Helper)?;
    let mut helper_function = helper
        .kernel_ir
        .functions
        .pop()
        .expect("validated fixed helper module has one function");
    helper_function.id = FIXED_HELPER_ID.into();
    helper_function.role = FunctionRole::InternalHelper;

    let u32_type = Type::Scalar(ScalarType::U32);
    let mut entry_block = BasicBlock::new(BlockId(0));
    entry_block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), u32_type.clone()),
        OperationKind::Call {
            callee: FIXED_HELPER_ID.into(),
            arguments: vec![ValueId(0)],
        },
    ));
    entry_block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        FIXED_ENTRY_ID,
        Signature::new(vec![u32_type], vec![]),
        vec![ValueId(0)],
        vec![entry_block],
    );
    entry
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));

    let mut kernel = Kernel::new(
        FIXED_KERNEL_ID,
        FIXED_ENTRY_ID,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(FIXED_WORKGROUP_X, 1, 1));
    kernel
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));

    let mut kernel_ir = Module::new("rustc::collected_scalar_control_flow_v1");
    kernel_ir.functions = vec![entry, helper_function];
    kernel_ir.kernels.push(kernel);
    verify_module(&kernel_ir)
        .map_err(CollectedExecutableScalarControlFlowErrorV1::InvalidKernelIr)?;
    let gfx942_llvm = lower_compiler_module_to_gfx942_llvm_ir(&kernel_ir)
        .map_err(CollectedExecutableScalarControlFlowErrorV1::Backend)?;
    Ok(CollectedExecutableScalarControlFlowArtifactV1 {
        helper,
        kernel_ir,
        gfx942_llvm,
    })
}

fn unsupported_collection(
    detail: impl Into<String>,
) -> CollectedExecutableScalarControlFlowErrorV1 {
    CollectedExecutableScalarControlFlowErrorV1::UnsupportedCollection {
        detail: detail.into(),
    }
}

fn abi_mismatch(
    role: &'static str,
    detail: impl Into<String>,
) -> CollectedExecutableScalarControlFlowErrorV1 {
    CollectedExecutableScalarControlFlowErrorV1::AbiMismatch {
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

    fn observed() -> ObservedClosureV1<u8> {
        ObservedClosureV1 {
            functions: vec![
                ObservedFunctionV1 {
                    identity: 1,
                    role: CollectedFunctionRole::KernelEntry,
                    signature: ExactSignatureV1::KernelU32ToUnit,
                    cfg_identity: ROOT_CFG_IDENTITY,
                },
                ObservedFunctionV1 {
                    identity: 2,
                    role: CollectedFunctionRole::InternalHelper,
                    signature: ExactSignatureV1::HelperU32ToU32,
                    cfg_identity: HELPER_CFG_IDENTITY,
                },
            ],
            root_call_target: 2,
            helper_call_count: 0,
        }
    }

    #[test]
    fn exact_observation_composes_kernel_helper_and_direct_gfx942_llvm() {
        admit_observed_closure(&observed()).unwrap();
        let artifact = lower_fixed_helper_and_compose_kernel().unwrap();

        assert_eq!(artifact.kernel_ir.kernels.len(), 1);
        assert_eq!(artifact.kernel_ir.functions.len(), 2);
        assert_eq!(artifact.helper.summary.blocks, 9);
        assert_eq!(artifact.helper.summary.loops, 2);
        assert_eq!(artifact.helper.summary.maximum_loop_depth, 2);
        assert!(
            artifact
                .gfx942_llvm
                .starts_with("target triple = \"amdgcn-amd-amdhsa\"")
        );
        assert!(
            artifact
                .gfx942_llvm
                .contains("define amdgpu_kernel void @scalar_control_flow_v1(i32 %arg0)"),
            "{}",
            artifact.gfx942_llvm
        );
        assert!(
            artifact
                .gfx942_llvm
                .contains("define internal i32 @__fe2o3_scalar_control_flow_v1_helper(i32 %arg0)")
        );
        assert!(
            artifact
                .gfx942_llvm
                .contains("call i32 @__fe2o3_scalar_control_flow_v1_helper(i32 %arg0)")
        );
        assert!(artifact.gfx942_llvm.contains("\"target-cpu\"=\"gfx942\""));
    }

    #[test]
    fn unsupported_collection_shapes_fail_closed() {
        let mut missing = observed();
        missing.functions.pop();
        assert!(matches!(
            admit_observed_closure(&missing),
            Err(CollectedExecutableScalarControlFlowErrorV1::UnsupportedCollection { .. })
        ));

        let mut additional = observed();
        additional.functions.push(ObservedFunctionV1 {
            identity: 3,
            role: CollectedFunctionRole::DeviceFfiExport,
            signature: ExactSignatureV1::HelperU32ToU32,
            cfg_identity: HELPER_CFG_IDENTITY,
        });
        assert!(matches!(
            admit_observed_closure(&additional),
            Err(CollectedExecutableScalarControlFlowErrorV1::UnsupportedCollection { .. })
        ));
    }

    #[test]
    fn role_identity_signature_and_call_substitutions_are_distinct_rejections() {
        let mut wrong_root = observed();
        wrong_root.functions[0].cfg_identity[0] ^= 1;
        assert!(matches!(
            admit_observed_closure(&wrong_root),
            Err(CollectedExecutableScalarControlFlowErrorV1::IdentityMismatch { role: "root", .. })
        ));

        let mut wrong_helper = observed();
        wrong_helper.functions[1].cfg_identity[0] ^= 1;
        assert!(matches!(
            admit_observed_closure(&wrong_helper),
            Err(
                CollectedExecutableScalarControlFlowErrorV1::IdentityMismatch {
                    role: "helper",
                    ..
                }
            )
        ));

        let mut wrong_signature = observed();
        wrong_signature.functions[1].signature = ExactSignatureV1::KernelU32ToUnit;
        assert!(matches!(
            admit_observed_closure(&wrong_signature),
            Err(CollectedExecutableScalarControlFlowErrorV1::AbiMismatch { role: "helper", .. })
        ));

        let mut substituted = observed();
        substituted.root_call_target = 3;
        assert!(matches!(
            admit_observed_closure(&substituted),
            Err(CollectedExecutableScalarControlFlowErrorV1::CallTargetSubstitution)
        ));
    }

    #[test]
    fn helper_calls_are_not_silently_added_to_the_fixed_closure() {
        let mut observed = observed();
        observed.helper_call_count = 1;
        assert_eq!(
            admit_observed_closure(&observed).unwrap_err().to_string(),
            "unsupported collected scalar-control-flow V1 shape: fixed helper must have no direct calls, found 1"
        );
    }

    #[test]
    fn target_and_pipeline_are_exact_vetoes() {
        admit_execution_context(EXACT_SCALAR_V2_TARGET, false).unwrap();
        assert_eq!(
            admit_execution_context("gfx942:xnack+", false)
                .unwrap_err()
                .to_string(),
            "collected scalar-control-flow V1 requires exact target `gfx942:xnack-`, found `gfx942:xnack+`"
        );
        assert_eq!(
            admit_execution_context(EXACT_SCALAR_V2_TARGET, true)
                .unwrap_err()
                .to_string(),
            "collected scalar-control-flow V1 rejects custom LLVM pipeline selection"
        );
    }
}
