//! Source-authenticated admission for the collected scalar-control-flow V2 pilot.

use std::error::Error;
use std::fmt;

use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{EarlyBinder, Instance, InstanceKind, TyCtxt, TyKind, TypingEnv, UintTy};

use crate::AmdGpuTarget;
use crate::collector::{CollectedFunction, CollectedFunctionRole, CollectionResult};
use crate::executable_scalar_control_flow_v1::{
    AuthenticatedScalarControlFlowCompositionV1, ExecutableScalarControlFlowErrorV1,
};
use crate::scalar_mir_v2::EXACT_SCALAR_V2_TARGET;

pub(crate) const COLLECTED_SCALAR_CONTROL_FLOW_PIPELINE_V2: &str =
    "collected-executable-scalar-control-flow-v2";
pub(crate) const NEXT_LOWERING_DEPENDENCY: &str = "repaired Scalar V1 accepted the role-preserving composition contract; executable-MIR capture/import for the authenticated helper remains required before lowering";
const FIXED_KERNEL_EXPORT: &str = "scalar_control_flow_v1";
const FIXED_LOGICAL_NAME: &str = "scalar_control_flow_v1";
const PORTABLE_CLOSURE_IDENTITY: [u8; 32] = [
    0x47, 0xf3, 0xdd, 0x95, 0x22, 0x68, 0x91, 0x3b, 0x8f, 0xe9, 0xec, 0xe9, 0x94, 0x6c, 0x1a, 0x5f,
    0xaf, 0x42, 0xac, 0xbd, 0xe3, 0x54, 0x4d, 0xf5, 0x1b, 0x4d, 0xd8, 0x3e, 0xde, 0x6b, 0x03, 0x65,
];

#[cfg(test)]
const ROOT_CFG_IDENTITY: [u8; 32] = [
    0xa3, 0xb2, 0xfb, 0x44, 0x1d, 0x62, 0x53, 0x21, 0x26, 0xd4, 0x4c, 0x05, 0x1c, 0xa1, 0x62, 0x56,
    0xb2, 0xbe, 0x79, 0xf5, 0x9b, 0x26, 0xef, 0x4d, 0x20, 0x51, 0x0b, 0xcd, 0x33, 0x1b, 0xa6, 0x21,
];
#[cfg(test)]
const HELPER_CFG_IDENTITY: [u8; 32] = [
    0xbd, 0xed, 0x0e, 0x3d, 0xa6, 0x30, 0x3a, 0x42, 0xb6, 0x3c, 0xd2, 0x35, 0x28, 0x4c, 0xce, 0xc6,
    0xa0, 0x3e, 0x92, 0xb7, 0xb2, 0xf5, 0xc4, 0xe7, 0x6b, 0x41, 0x29, 0x85, 0x97, 0xe8, 0x6c, 0x2d,
];

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
    composition: AuthenticatedScalarControlFlowCompositionV1,
    root_cfg_identity: [u8; 32],
    helper_cfg_identity: [u8; 32],
}

impl AuthenticatedCollectedScalarControlFlowV2 {
    pub(crate) fn kernel_export(&self) -> &str {
        self.composition.kernel_export_symbol()
    }

    pub(crate) const fn composition(&self) -> &AuthenticatedScalarControlFlowCompositionV1 {
        &self.composition
    }

    pub(crate) fn root_identity_hex(&self) -> String {
        encode_hex(&self.root_cfg_identity)
    }

    pub(crate) fn helper_identity_hex(&self) -> String {
        encode_hex(&self.helper_cfg_identity)
    }
}

#[derive(Debug)]
pub(crate) enum CollectedExecutableScalarControlFlowErrorV2 {
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
    PortableMir {
        detail: String,
    },
    Composition(ExecutableScalarControlFlowErrorV1),
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
            Self::Composition(error) => write!(
                formatter,
                "collected scalar-control-flow V2 composition authority failed: {error}"
            ),
        }
    }
}

impl Error for CollectedExecutableScalarControlFlowErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Composition(error) => Some(error),
            _ => None,
        }
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
    require_portable_closure_identity(*portable_identity.as_bytes())?;
    let composition =
        AuthenticatedScalarControlFlowCompositionV1::from_authenticated_collected_pair(
            tcx, root, helper,
        )
        .map_err(CollectedExecutableScalarControlFlowErrorV2::Composition)?;
    Ok(AuthenticatedCollectedScalarControlFlowV2 {
        composition,
        root_cfg_identity: observed.functions[0].cfg_identity,
        helper_cfg_identity: observed.functions[1].cfg_identity,
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
}
