//! Bounded executable refinement for one production-lowered `u32` call/CFG shape.
//!
//! The accepted language is deliberately narrow: a kernel directly calls one
//! non-recursive helper, and that helper implements a four-block diamond
//! `if x == 0 { x } else { C }` whose arms join through one SSA block argument.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{FunctionRole, OperationKind, ScalarType, Terminator, Type, ValueId};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticConstantValueV1, SemanticEdgeRoleV1, SemanticFunctionRoleV1, SemanticLocalRoleV1,
    SemanticOperandV1, SemanticRvalueKindV1, SemanticScalarTypeV1, SemanticStatementKindV1,
    SemanticTerminatorKindV1, SemanticTypeShapeV1, SemanticUnwindActionV1,
};
use sha2::{Digest, Sha256};

use crate::{ProductionCanonicalKernelIrIdentityV1, ProductionSemanticKirOwnerV1};

/// Version of the bounded call/CFG model.
pub const MIR_KIR_CFG_REFINEMENT_MODEL_VERSION_V2: u16 = 1;
/// Stable Verus theorem name.
pub const MIR_KIR_CFG_REFINEMENT_THEOREM_V2: &str = "fe2o3_mir_kir_u32_diamond_call_refines_v2";
/// Digest of the exact positive Verus source.
pub const MIR_KIR_CFG_REFINEMENT_PROOF_SHA256_V2: [u8; 32] = [
    0xd2, 0xa8, 0x49, 0x52, 0xe5, 0x85, 0x67, 0x3c, 0x68, 0x6a, 0x95, 0x62, 0x7a, 0x9e, 0x18, 0xf7,
    0x5c, 0xe4, 0xba, 0x80, 0x9b, 0xa0, 0xa6, 0xf4, 0x0a, 0x7c, 0x5f, 0x93, 0x7b, 0x1b, 0x2a, 0xf9,
];
/// Digest of the pinned Verus executable.
pub const MIR_KIR_CFG_REFINEMENT_VERUS_SHA256_V2: [u8; 32] = [
    0xad, 0x26, 0x69, 0xf5, 0x79, 0xd8, 0x98, 0xed, 0xe5, 0x3f, 0x2b, 0xf8, 0x4e, 0x80, 0xa1, 0xda,
    0xf4, 0xe3, 0x57, 0x87, 0x39, 0xb0, 0xf5, 0x80, 0x7e, 0xf2, 0x09, 0xa0, 0xc9, 0xf3, 0x82, 0xdd,
];
/// Digest of the pinned Verus/vstd/Z3 closure manifest.
pub const MIR_KIR_CFG_REFINEMENT_CLOSURE_SHA256_V2: [u8; 32] = [
    0xd2, 0x8d, 0xf3, 0xfb, 0x5e, 0x0d, 0x74, 0x76, 0x37, 0x54, 0x39, 0x33, 0xdf, 0xc3, 0x8c, 0xff,
    0x45, 0x57, 0x6d, 0xa9, 0xb9, 0x20, 0xd7, 0x55, 0xb4, 0xb7, 0xe9, 0x19, 0xe4, 0x7a, 0x60, 0x19,
];

const MODEL_DOMAIN: &[u8] = b"FE2O3/MIR-KIR/U32-DIAMOND-CALL/MODEL/V2\0";
const EVIDENCE_DOMAIN: &[u8] = b"FE2O3/MIR-KIR/U32-DIAMOND-CALL/EVIDENCE/V2\0";
/// Exact source and target step budget for the closed program.
pub const MIR_KIR_CFG_REQUIRED_FUEL_V2: u8 = 6;

/// Observable control-flow event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirKirCfgEventV2 {
    /// Caller enters the helper.
    Call,
    /// The zero case is taken.
    ZeroArm,
    /// The default case is taken.
    NonzeroArm,
    /// The selected value enters the join block.
    Join,
    /// The helper returns the selected value.
    Return(u32),
}

/// Result and exact observable trace of the bounded execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirKirCfgObservationV2 {
    /// Returned value.
    pub result: u32,
    /// Ordered call, branch, join, and return events.
    pub trace: [MirKirCfgEventV2; 4],
}

fn consume_step_v2(fuel: &mut u8) -> Option<()> {
    if *fuel == 0 {
        None
    } else {
        *fuel -= 1;
        Some(())
    }
}

/// Executes the semantic-MIR model, returning `None` on insufficient fuel.
pub fn execute_mir_u32_diamond_call_v2(
    input: u32,
    fallback: u32,
    fuel: u8,
) -> Option<MirKirCfgObservationV2> {
    let mut remaining = fuel;
    consume_step_v2(&mut remaining)?; // caller call terminator
    consume_step_v2(&mut remaining)?; // helper switch terminator
    let (result, arm) = if input == 0 {
        consume_step_v2(&mut remaining)?; // copy assignment
        (input, MirKirCfgEventV2::ZeroArm)
    } else {
        consume_step_v2(&mut remaining)?; // constant assignment
        (fallback, MirKirCfgEventV2::NonzeroArm)
    };
    consume_step_v2(&mut remaining)?; // arm goto
    consume_step_v2(&mut remaining)?; // helper return
    consume_step_v2(&mut remaining)?; // caller return
    Some(MirKirCfgObservationV2 {
        result,
        trace: [
            MirKirCfgEventV2::Call,
            arm,
            MirKirCfgEventV2::Join,
            MirKirCfgEventV2::Return(result),
        ],
    })
}

/// Executes the canonical-KIR model, returning `None` on insufficient fuel.
pub fn execute_kir_u32_diamond_call_v2(
    input: u32,
    fallback: u32,
    fuel: u8,
) -> Option<MirKirCfgObservationV2> {
    execute_kir_with_shape_v2(input, fallback, fuel, KirCfgShapeV2::EXACT)
}

#[derive(Clone, Copy)]
struct KirCfgShapeV2 {
    callee_is_helper: bool,
    reverse_branch: bool,
    swap_edge_arguments: bool,
    return_join_parameter: bool,
}

impl KirCfgShapeV2 {
    const EXACT: Self = Self {
        callee_is_helper: true,
        reverse_branch: false,
        swap_edge_arguments: false,
        return_join_parameter: true,
    };
}

fn execute_kir_with_shape_v2(
    input: u32,
    fallback: u32,
    fuel: u8,
    shape: KirCfgShapeV2,
) -> Option<MirKirCfgObservationV2> {
    let mut remaining = fuel;
    consume_step_v2(&mut remaining)?; // call operation and branch to continuation
    if !shape.callee_is_helper {
        return None;
    }
    let mut block = 0_u8;
    let mut incoming = None;
    let mut arm = None;
    loop {
        consume_step_v2(&mut remaining)?;
        match block {
            0 => {
                block = if (input == 0) != shape.reverse_branch {
                    1
                } else {
                    2
                }
            }
            1 => {
                incoming = Some(if shape.swap_edge_arguments {
                    fallback
                } else {
                    input
                });
                arm = Some(MirKirCfgEventV2::ZeroArm);
                consume_step_v2(&mut remaining)?; // branch with join argument
                block = 3;
            }
            2 => {
                incoming = Some(if shape.swap_edge_arguments {
                    input
                } else {
                    fallback
                });
                arm = Some(MirKirCfgEventV2::NonzeroArm);
                consume_step_v2(&mut remaining)?; // branch with join argument
                block = 3;
            }
            3 => {
                let joined = incoming?;
                let result = if shape.return_join_parameter {
                    joined
                } else {
                    joined.wrapping_add(1)
                };
                consume_step_v2(&mut remaining)?; // caller return
                return Some(MirKirCfgObservationV2 {
                    result,
                    trace: [
                        MirKirCfgEventV2::Call,
                        arm?,
                        MirKirCfgEventV2::Join,
                        MirKirCfgEventV2::Return(result),
                    ],
                });
            }
            _ => return None,
        }
    }
}

/// Checks equality of the two executable observations.
pub fn mir_kir_u32_diamond_call_refines_v2(input: u32, fallback: u32, fuel: u8) -> bool {
    execute_mir_u32_diamond_call_v2(input, fallback, fuel)
        == execute_kir_u32_diamond_call_v2(input, fallback, fuel)
}

/// Every semantic and KIR value/block identity checked by the relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirKirCfgBindingsV2 {
    /// Semantic kernel-root index.
    pub semantic_root: u32,
    /// Semantic helper index.
    pub semantic_helper: u32,
    /// Semantic caller argument local.
    pub semantic_root_argument: u32,
    /// Semantic caller local receiving the helper result.
    pub semantic_call_destination: u32,
    /// Semantic helper argument local.
    pub semantic_helper_argument: u32,
    /// Semantic helper return local.
    pub semantic_helper_return: u32,
    /// KIR caller parameter.
    pub kir_root_parameter: ValueId,
    /// KIR direct-call result.
    pub kir_call_result: ValueId,
    /// KIR caller entry block.
    pub kir_root_entry: u32,
    /// KIR caller continuation block.
    pub kir_root_continuation: u32,
    /// KIR helper parameter.
    pub kir_helper_parameter: ValueId,
    /// KIR helper entry block.
    pub kir_entry: u32,
    /// KIR zero arm.
    pub kir_zero_arm: u32,
    /// KIR default arm.
    pub kir_nonzero_arm: u32,
    /// KIR join block.
    pub kir_join: u32,
    /// KIR definition of the fallback constant.
    pub kir_fallback_value: ValueId,
    /// KIR join/phi parameter.
    pub kir_join_parameter: ValueId,
}

/// Authority-free proof evidence derived from one replayed production owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertMirKirCfgRefinementEvidenceV2 {
    identity: [u8; 32],
    model_identity: [u8; 32],
    semantic_sha256: [u8; 32],
    canonical_kernel_ir: ProductionCanonicalKernelIrIdentityV1,
    fallback: u32,
    bindings: MirKirCfgBindingsV2,
}

impl InertMirKirCfgRefinementEvidenceV2 {
    /// Replays and validates the exact closed semantic and KIR shapes.
    pub fn from_live_owner(
        owner: &ProductionSemanticKirOwnerV1,
    ) -> Result<Self, MirKirCfgRefinementErrorV2> {
        owner
            .verify_equivalence()
            .map_err(|error| MirKirCfgRefinementErrorV2::LiveOwner(error.to_string()))?;
        let semantic = owner.semantic().semantic();
        if semantic.functions().len() != 2 || owner.module().functions.len() != 2 {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }
        let functions = semantic
            .functions()
            .iter()
            .enumerate()
            .filter_map(|(index, function)| {
                matches!(
                    function.role(),
                    SemanticFunctionRoleV1::KernelRoot | SemanticFunctionRoleV1::InternalHelper
                )
                .then_some((index as u32, function))
            })
            .collect::<Vec<_>>();
        let [(root_id, root), (helper_id, helper)] = functions.as_slice() else {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        };
        let (root_id, helper_id) = (*root_id, *helper_id);
        if root.role() != SemanticFunctionRoleV1::KernelRoot
            || helper.role() != SemanticFunctionRoleV1::InternalHelper
            || root.blocks().len() != 2
            || root.entry().index() != 0
            || root.locals().len() != 3
            || helper.entry().index() != 0
            || helper.blocks().len() != 4
            || helper.locals().len() != 2
        {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }

        let helper_argument = unique_local_with_role(helper, |role| {
            matches!(role, SemanticLocalRoleV1::Argument(0))
        })?;
        let helper_return =
            unique_local_with_role(helper, |role| role == SemanticLocalRoleV1::Return)?;
        require_u32_local(semantic.types(), helper, helper_argument)?;
        require_u32_local(semantic.types(), helper, helper_return)?;

        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = helper.blocks()[0].terminator().kind()
        else {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        };
        if !helper.blocks()[0].statements().is_empty()
            || copied_local(discriminant) != Some(helper_argument)
            || targets.values().len() != 1
            || targets.values()[0].value() != 0
            || targets.values()[0].edge().role() != SemanticEdgeRoleV1::SwitchValue
            || targets.values()[0].edge().target().index() != 1
            || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
            || targets.otherwise().target().index() != 2
        {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }
        require_arm_copy(helper, 1, helper_return, helper_argument, 3)?;
        let fallback = require_arm_constant(semantic.types(), helper, 2, helper_return, 3)?;
        if !helper.blocks()[3].statements().is_empty()
            || !matches!(
                helper.blocks()[3].terminator().kind(),
                SemanticTerminatorKindV1::Return
            )
        {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }

        let SemanticTerminatorKindV1::Call(call) = root.blocks()[0].terminator().kind() else {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        };
        let root_argument = unique_local_with_role(root, |role| {
            matches!(role, SemanticLocalRoleV1::Argument(0))
        })?;
        require_u32_local(semantic.types(), root, root_argument)?;
        let call_destination = call
            .destination()
            .ok_or(MirKirCfgRefinementErrorV2::UnsupportedShape)?;
        let root_call_result = call_destination.place().local().index();
        require_u32_local(semantic.types(), root, root_call_result)?;
        if root.locals()[root_call_result as usize].role() != SemanticLocalRoleV1::Temporary {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }
        if !root.blocks()[0].statements().is_empty()
            || call.callee().index() != helper_id
            || call.arguments().len() != 1
            || copied_local(&call.arguments()[0]) != Some(root_argument)
            || call.unwind() != SemanticUnwindActionV1::Unreachable
            || !call_destination.place().projections().is_empty()
            || call_destination.edge().role() != SemanticEdgeRoleV1::CallReturn
            || call_destination.edge().target().index() != 1
            || !root.blocks()[1].statements().is_empty()
            || !matches!(
                root.blocks()[1].terminator().kind(),
                SemanticTerminatorKindV1::Return
            )
        {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }

        let correspondence = owner.correspondence();
        let root_binding = correspondence
            .lowered_functions()
            .iter()
            .find(|item| item.semantic_function().index() == root_id)
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        let helper_binding = correspondence
            .lowered_functions()
            .iter()
            .find(|item| item.semantic_function().index() == helper_id)
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        let kir_root = owner
            .module()
            .function(root_binding.kernel_ir_function())
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        let kir_helper = owner
            .module()
            .function(helper_binding.kernel_ir_function())
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        if kir_root.role != FunctionRole::KernelEntry
            || kir_helper.role != FunctionRole::InternalHelper
            || kir_root.signature.parameters.as_slice() != [Type::Scalar(ScalarType::U32)]
            || !kir_root.signature.results.is_empty()
            || kir_helper.signature.parameters.as_slice() != [Type::Scalar(ScalarType::U32)]
            || kir_helper.signature.results.as_slice() != [Type::Scalar(ScalarType::U32)]
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }
        let kir_root_parameter = parameter_value(correspondence, root_id, root_argument)?;
        let kir_helper_parameter = parameter_value(correspondence, helper_id, helper_argument)?;
        let root_body = kir_root
            .body
            .as_ref()
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        let helper_body = kir_helper
            .body
            .as_ref()
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        let root_entry = block_for(correspondence, root_id, 0)?;
        let root_continuation = block_for(correspondence, root_id, 1)?;
        let root_block = root_body
            .blocks
            .iter()
            .find(|block| block.id == root_entry)
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        let call_span = correspondence
            .terminator_operation_spans()
            .iter()
            .find(|span| {
                span.semantic_function().index() == root_id && span.semantic_block().index() == 0
            })
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        if call_span.kernel_ir_block() != root_entry
            || call_span.first_operation_ordinal() != 0
            || call_span.operation_count() != 1
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }
        let [call_operation] = root_block.operations.as_slice() else {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        };
        let ([call_result], OperationKind::Call { callee, arguments }) =
            (call_operation.results.as_slice(), &call_operation.kind)
        else {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        };
        if callee != helper_binding.kernel_ir_function()
            || arguments.as_slice() != [kir_root_parameter]
            || call_result.ty != Type::Scalar(ScalarType::U32)
            || !call_operation.memory_effects().is_empty()
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }
        if root_body.blocks.len() != 2
            || root_body.parameters.as_slice() != [kir_root_parameter]
            || !matches!(root_block.terminator.as_ref(), Some(Terminator::Branch { target, arguments }) if *target == root_continuation && arguments.is_empty())
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }
        let root_return = root_body
            .blocks
            .iter()
            .find(|block| block.id == root_continuation)
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        if !root_return.parameters.is_empty()
            || !root_return.operations.is_empty()
            || !matches!(root_return.terminator.as_ref(), Some(Terminator::Return { values }) if values.is_empty())
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }

        let entry = block_for(correspondence, helper_id, 0)?;
        let zero = block_for(correspondence, helper_id, 1)?;
        let nonzero = block_for(correspondence, helper_id, 2)?;
        let join = block_for(correspondence, helper_id, 3)?;
        let kir_entry_block = helper_body
            .blocks
            .iter()
            .find(|block| block.id == entry)
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        match kir_entry_block.terminator.as_ref() {
            Some(Terminator::Switch {
                selector,
                cases,
                default_target,
                default_arguments,
            }) if *selector == kir_helper_parameter
                && cases.len() == 1
                && cases[0].value == 0
                && cases[0].target == zero
                && cases[0].arguments.is_empty()
                && *default_target == nonzero
                && default_arguments.is_empty() => {}
            _ => return Err(MirKirCfgRefinementErrorV2::Correspondence),
        }
        if !kir_entry_block.parameters.is_empty() || !kir_entry_block.operations.is_empty() {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }
        let zero_block = helper_body
            .blocks
            .iter()
            .find(|block| block.id == zero)
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        let nonzero_block = helper_body
            .blocks
            .iter()
            .find(|block| block.id == nonzero)
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        let join_block = helper_body
            .blocks
            .iter()
            .find(|block| block.id == join)
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        let fallback_value = match nonzero_block.operations.as_slice() {
            [operation]
                if operation.kind
                    == OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(fallback))
                    && operation.results.len() == 1
                    && operation.results[0].ty == Type::Scalar(ScalarType::U32)
                    && operation.memory_effects().is_empty() =>
            {
                operation.results[0].id
            }
            _ => return Err(MirKirCfgRefinementErrorV2::Correspondence),
        };
        if !zero_block.parameters.is_empty()
            || !zero_block.operations.is_empty()
            || !nonzero_block.parameters.is_empty()
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }
        require_branch(zero_block, join, kir_helper_parameter)?;
        require_branch(nonzero_block, join, fallback_value)?;
        let [join_parameter] = join_block.parameters.as_slice() else {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        };
        if join_parameter.ty != Type::Scalar(ScalarType::U32)
            || !join_block.operations.is_empty()
            || !matches!(join_block.terminator.as_ref(), Some(Terminator::Return { values }) if values.as_slice() == [join_parameter.id])
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }
        if helper_body.blocks.len() != 4
            || helper_body.parameters.as_slice() != [kir_helper_parameter]
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }
        let bindings = MirKirCfgBindingsV2 {
            semantic_root: root_id,
            semantic_helper: helper_id,
            semantic_root_argument: root_argument,
            semantic_call_destination: root_call_result,
            semantic_helper_argument: helper_argument,
            semantic_helper_return: helper_return,
            kir_root_parameter,
            kir_call_result: call_result.id,
            kir_root_entry: root_entry.0,
            kir_root_continuation: root_continuation.0,
            kir_helper_parameter,
            kir_entry: entry.0,
            kir_zero_arm: zero.0,
            kir_nonzero_arm: nonzero.0,
            kir_join: join.0,
            kir_fallback_value: fallback_value,
            kir_join_parameter: join_parameter.id,
        };
        let model_identity = model_identity_v2();
        let semantic_sha256 = *semantic.semantic_sha256().as_bytes();
        let canonical_kernel_ir = owner.canonical_kernel_ir_identity();
        let identity = evidence_identity_v2(
            model_identity,
            semantic_sha256,
            canonical_kernel_ir,
            fallback,
            bindings,
        );
        let evidence = Self {
            identity,
            model_identity,
            semantic_sha256,
            canonical_kernel_ir,
            fallback,
            bindings,
        };
        evidence.revalidate()?;
        Ok(evidence)
    }

    /// Rechecks proof identity and canonical evidence identity.
    pub fn revalidate(&self) -> Result<(), MirKirCfgRefinementErrorV2> {
        if self.model_identity != model_identity_v2()
            || self.semantic_sha256 == [0; 32]
            || self.canonical_kernel_ir.digest() == &[0; 32]
            || self.identity
                != evidence_identity_v2(
                    self.model_identity,
                    self.semantic_sha256,
                    self.canonical_kernel_ir,
                    self.fallback,
                    self.bindings,
                )
        {
            return Err(MirKirCfgRefinementErrorV2::NonCanonicalEvidence);
        }
        Ok(())
    }
    /// Returns the domain-separated evidence identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    /// Returns the executable proof-model identity.
    pub const fn model_identity(&self) -> &[u8; 32] {
        &self.model_identity
    }
    /// Returns the exact admitted semantic-MIR identity.
    pub const fn semantic_sha256(&self) -> &[u8; 32] {
        &self.semantic_sha256
    }
    /// Returns the exact canonical-KIR identity.
    pub const fn canonical_kernel_ir_identity(&self) -> ProductionCanonicalKernelIrIdentityV1 {
        self.canonical_kernel_ir
    }
    /// Returns the checked fallback constant.
    pub const fn fallback(&self) -> u32 {
        self.fallback
    }
    /// Returns the checked source/target locator relation.
    pub const fn bindings(&self) -> MirKirCfgBindingsV2 {
        self.bindings
    }
    /// This evidence never grants artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Fail-closed derivation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirKirCfgRefinementErrorV2 {
    /// Full owner replay failed.
    LiveOwner(String),
    /// Semantic MIR is outside the exact bounded language.
    UnsupportedShape,
    /// Canonical KIR does not have the exact related shape.
    Correspondence,
    /// Retained evidence was mutated or noncanonical.
    NonCanonicalEvidence,
}
impl fmt::Display for MirKirCfgRefinementErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LiveOwner(e) => write!(f, "live owner failed: {e}"),
            Self::UnsupportedShape => {
                f.write_str("semantic program is outside the bounded u32 diamond-call language")
            }
            Self::Correspondence => {
                f.write_str("KIR does not exactly implement the admitted diamond/call relation")
            }
            Self::NonCanonicalEvidence => f.write_str("CFG refinement evidence is not canonical"),
        }
    }
}
impl Error for MirKirCfgRefinementErrorV2 {}

fn unique_local_with_role(
    function: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1,
    predicate: impl Fn(SemanticLocalRoleV1) -> bool,
) -> Result<u32, MirKirCfgRefinementErrorV2> {
    let matches = function
        .locals()
        .iter()
        .enumerate()
        .filter_map(|(i, local)| predicate(local.role()).then_some(i as u32))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [local] => Ok(*local),
        _ => Err(MirKirCfgRefinementErrorV2::UnsupportedShape),
    }
}
fn require_u32_local(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1,
    local: u32,
) -> Result<(), MirKirCfgRefinementErrorV2> {
    let declaration = function
        .locals()
        .get(local as usize)
        .and_then(|local| types.get(local.ty().index() as usize))
        .ok_or(MirKirCfgRefinementErrorV2::UnsupportedShape)?;
    matches!(
        declaration.shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        })
    )
    .then_some(())
    .ok_or(MirKirCfgRefinementErrorV2::UnsupportedShape)
}
fn copied_local(operand: &SemanticOperandV1) -> Option<u32> {
    match operand {
        SemanticOperandV1::Copy(place) if place.projections().is_empty() => {
            Some(place.local().index())
        }
        _ => None,
    }
}
fn require_goto(
    function: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1,
    block: usize,
    target: u32,
) -> Result<(), MirKirCfgRefinementErrorV2> {
    matches!(function.blocks()[block].terminator().kind(), SemanticTerminatorKindV1::Goto(edge) if edge.role() == SemanticEdgeRoleV1::Goto && edge.target().index() == target).then_some(()).ok_or(MirKirCfgRefinementErrorV2::UnsupportedShape)
}
fn require_arm_copy(
    function: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1,
    block: usize,
    destination: u32,
    source: u32,
    join: u32,
) -> Result<(), MirKirCfgRefinementErrorV2> {
    let [statement] = function.blocks()[block].statements() else {
        return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
    };
    let SemanticStatementKindV1::Assign(assign) = statement.kind() else {
        return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
    };
    if !assign.destination().projections().is_empty()
        || assign.destination().local().index() != destination
        || !matches!(assign.value().kind(), SemanticRvalueKindV1::Use(operand) if copied_local(operand) == Some(source))
    {
        return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
    }
    require_goto(function, block, join)
}
fn require_arm_constant(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1,
    block: usize,
    destination: u32,
    join: u32,
) -> Result<u32, MirKirCfgRefinementErrorV2> {
    let [statement] = function.blocks()[block].statements() else {
        return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
    };
    let SemanticStatementKindV1::Assign(assign) = statement.kind() else {
        return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
    };
    if !assign.destination().projections().is_empty()
        || assign.destination().local().index() != destination
    {
        return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
    }
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)) = assign.value().kind()
    else {
        return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
    };
    if !matches!(
        types
            .get(constant.ty().index() as usize)
            .map(|ty| ty.shape()),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        }))
    ) {
        return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
    }
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
    };
    let result =
        u32::try_from(value.bits()).map_err(|_| MirKirCfgRefinementErrorV2::UnsupportedShape)?;
    if value.size_bytes() != 4 {
        return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
    }
    require_goto(function, block, join)?;
    Ok(result)
}
fn parameter_value(
    correspondence: &crate::SemanticKirCorrespondenceV1,
    function: u32,
    local: u32,
) -> Result<ValueId, MirKirCfgRefinementErrorV2> {
    correspondence
        .parameter_bindings()
        .iter()
        .find(|item| {
            item.semantic_function().index() == function && item.semantic_local().index() == local
        })
        .map(|item| item.kernel_ir_value())
        .ok_or(MirKirCfgRefinementErrorV2::Correspondence)
}
fn block_for(
    correspondence: &crate::SemanticKirCorrespondenceV1,
    function: u32,
    block: u32,
) -> Result<fe2o3_kernel_ir::BlockId, MirKirCfgRefinementErrorV2> {
    correspondence
        .blocks()
        .iter()
        .find(|item| {
            item.semantic_function().index() == function && item.semantic_block().index() == block
        })
        .map(|item| item.kernel_ir_block())
        .ok_or(MirKirCfgRefinementErrorV2::Correspondence)
}
fn require_branch(
    block: &fe2o3_kernel_ir::BasicBlock,
    target: fe2o3_kernel_ir::BlockId,
    argument: ValueId,
) -> Result<(), MirKirCfgRefinementErrorV2> {
    matches!(block.terminator.as_ref(), Some(Terminator::Branch { target: actual, arguments }) if *actual == target && arguments.as_slice() == [argument]).then_some(()).ok_or(MirKirCfgRefinementErrorV2::Correspondence)
}
fn model_identity_v2() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(MODEL_DOMAIN);
    hash.update(MIR_KIR_CFG_REFINEMENT_MODEL_VERSION_V2.to_le_bytes());
    hash.update(MIR_KIR_CFG_REFINEMENT_THEOREM_V2.as_bytes());
    hash.update(MIR_KIR_CFG_REFINEMENT_PROOF_SHA256_V2);
    hash.update(MIR_KIR_CFG_REFINEMENT_VERUS_SHA256_V2);
    hash.update(MIR_KIR_CFG_REFINEMENT_CLOSURE_SHA256_V2);
    hash.finalize().into()
}
fn evidence_identity_v2(
    model: [u8; 32],
    semantic: [u8; 32],
    kir: ProductionCanonicalKernelIrIdentityV1,
    fallback: u32,
    bindings: MirKirCfgBindingsV2,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(EVIDENCE_DOMAIN);
    hash.update(model);
    hash.update(semantic);
    hash.update(kir.digest());
    hash.update(kir.canonical_length().to_le_bytes());
    hash.update(fallback.to_le_bytes());
    for value in [
        bindings.semantic_root,
        bindings.semantic_helper,
        bindings.semantic_root_argument,
        bindings.semantic_call_destination,
        bindings.semantic_helper_argument,
        bindings.semantic_helper_return,
        bindings.kir_root_parameter.0,
        bindings.kir_call_result.0,
        bindings.kir_root_entry,
        bindings.kir_root_continuation,
        bindings.kir_helper_parameter.0,
        bindings.kir_entry,
        bindings.kir_zero_arm,
        bindings.kir_nonzero_arm,
        bindings.kir_join,
        bindings.kir_fallback_value.0,
        bindings.kir_join_parameter.0,
    ] {
        hash.update(value.to_le_bytes());
    }
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executable_semantics_are_nonvacuous_and_fuel_bounded() {
        for input in [0, 1, u32::MAX] {
            assert!(mir_kir_u32_diamond_call_refines_v2(input, 17, 6));
        }
        assert_eq!(execute_mir_u32_diamond_call_v2(1, 17, 5), None);
        assert_eq!(execute_mir_u32_diamond_call_v2(0, 17, 6).unwrap().result, 0);
        assert_eq!(
            execute_mir_u32_diamond_call_v2(1, 17, 6).unwrap().result,
            17
        );
    }
    #[test]
    fn branch_direction_phi_and_return_mutations_change_observation() {
        let expected = execute_mir_u32_diamond_call_v2(1, 17, 6).unwrap();
        for hostile in [
            KirCfgShapeV2 {
                reverse_branch: true,
                ..KirCfgShapeV2::EXACT
            },
            KirCfgShapeV2 {
                swap_edge_arguments: true,
                ..KirCfgShapeV2::EXACT
            },
            KirCfgShapeV2 {
                return_join_parameter: false,
                ..KirCfgShapeV2::EXACT
            },
        ] {
            assert_ne!(
                expected,
                execute_kir_with_shape_v2(1, 17, 6, hostile).unwrap()
            );
        }
        assert_eq!(
            execute_kir_with_shape_v2(
                1,
                17,
                6,
                KirCfgShapeV2 {
                    callee_is_helper: false,
                    ..KirCfgShapeV2::EXACT
                }
            ),
            None
        );
    }
    #[test]
    fn model_binds_exact_proof_and_closure() {
        assert_eq!(
            Sha256::digest(include_bytes!("../verus/mir_kir_cfg_refinement_v2.rs")).as_slice(),
            MIR_KIR_CFG_REFINEMENT_PROOF_SHA256_V2
        );
        assert_eq!(
            Sha256::digest(include_bytes!("../verus/pins/VERUS_CLOSURE_MANIFEST")).as_slice(),
            MIR_KIR_CFG_REFINEMENT_CLOSURE_SHA256_V2
        );
    }
}
