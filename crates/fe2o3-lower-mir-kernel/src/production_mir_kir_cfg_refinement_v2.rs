//! Bounded executable refinement for one production-lowered `u32` call/CFG slice.
//!
//! The accepted language is deliberately narrow: a kernel entry begins by
//! directly calling one non-recursive helper, and that helper implements a
//! four-block diamond `if x == 0 { x } else { C }` whose arms join through one
//! SSA block argument. The observation ends when the call result reaches the
//! caller continuation; separately verified continuation behavior is excluded.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{FunctionRole, OperationKind, ScalarType, Terminator, Type, ValueId};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticConstantValueV1, SemanticEdgeRoleV1, SemanticFunctionRoleV1, SemanticLocalRoleV1,
    SemanticOperandV1, SemanticRvalueKindV1, SemanticScalarTypeV1, SemanticStatementKindV1,
    SemanticSwitchTargetV1, SemanticSwitchTargetsV1, SemanticTerminatorKindV1, SemanticTypeShapeV1,
    SemanticUnwindActionV1,
};
use sha2::{Digest, Sha256};

use crate::{ProductionCanonicalKernelIrIdentityV1, ProductionSemanticKirOwnerV1};

/// Version of the bounded call/CFG model.
pub const MIR_KIR_CFG_REFINEMENT_MODEL_VERSION_V2: u16 = 2;
/// Stable Verus theorem name.
pub const MIR_KIR_CFG_REFINEMENT_THEOREM_V2: &str = "fe2o3_mir_kir_u32_diamond_call_refines_v2";
/// Digest of the exact positive Verus source.
pub const MIR_KIR_CFG_REFINEMENT_PROOF_SHA256_V2: [u8; 32] = [
    0xc9, 0xd5, 0x88, 0x1f, 0xf9, 0xf0, 0x3e, 0x01, 0x6e, 0xbe, 0xc7, 0x53, 0xbc, 0x15, 0xee, 0x5a,
    0x1f, 0xf0, 0xe5, 0xec, 0xac, 0xd9, 0xf8, 0xf5, 0x31, 0xe2, 0x81, 0x0e, 0xbd, 0xe0, 0x6a, 0x34,
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
/// Exact source and target semantic-macro-step budget for the closed call slice.
///
/// A macro step is one of [`MirKirCfgMacroStepV2`], not a raw KIR operation or
/// terminator count. Production validation binds every macro step to its exact
/// semantic statement/terminator and KIR operation/edge sites.
pub const MIR_KIR_CFG_REQUIRED_FUEL_V2: u8 = 6;

/// The six charged semantic transitions in the bounded call/diamond machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MirKirCfgMacroStepV2 {
    /// Transfer the caller argument into the helper frame.
    EnterHelper = 1,
    /// Select the zero or default helper successor.
    SelectArm = 2,
    /// Define the selected arm value.
    DefineArmValue = 3,
    /// Transfer the selected edge value into the join parameter.
    EnterJoin = 4,
    /// Read the helper join parameter as the helper return operand.
    ReturnFromHelper = 5,
    /// Bind the helper return to the caller result at the continuation boundary.
    ObserveCallResult = 6,
}

/// Canonical charged macro-step roster, in execution order.
pub const MIR_KIR_CFG_MACRO_STEPS_V2: [MirKirCfgMacroStepV2; 6] = [
    MirKirCfgMacroStepV2::EnterHelper,
    MirKirCfgMacroStepV2::SelectArm,
    MirKirCfgMacroStepV2::DefineArmValue,
    MirKirCfgMacroStepV2::EnterJoin,
    MirKirCfgMacroStepV2::ReturnFromHelper,
    MirKirCfgMacroStepV2::ObserveCallResult,
];

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
    /// Internal helper result delivered to the caller's call destination.
    pub result: u32,
    /// Ordered call, branch, join, and return events.
    pub trace: [MirKirCfgEventV2; 4],
}

fn consume_macro_step_v2(fuel: &mut u8) -> Option<()> {
    if *fuel == 0 {
        None
    } else {
        *fuel -= 1;
        Some(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MirPcV2 {
    CallerCall,
    HelperSwitch,
    ZeroArm,
    NonzeroArm,
    HelperJoin,
    HelperReturn,
    CallerContinuation,
    Done,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MirMachineStateV2 {
    pc: MirPcV2,
    root_argument: u32,
    helper_argument: u32,
    helper_return: u32,
    call_destination: u32,
    arm: Option<MirKirCfgEventV2>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KirPcV2 {
    RootCall,
    HelperEntry,
    ZeroBlock,
    NonzeroBlock,
    JoinBlock,
    HelperReturn,
    RootContinuation,
    Done,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KirMachineStateV2 {
    pc: KirPcV2,
    root_parameter: u32,
    helper_parameter: u32,
    fallback_value: u32,
    selected_edge_value: u32,
    join_parameter: u32,
    helper_return_operand: u32,
    call_result: u32,
    arm: Option<MirKirCfgEventV2>,
}

fn initial_mir_state_v2(input: u32) -> MirMachineStateV2 {
    MirMachineStateV2 {
        pc: MirPcV2::CallerCall,
        root_argument: input,
        helper_argument: 0,
        helper_return: 0,
        call_destination: 0,
        arm: None,
    }
}

fn mir_macro_step_v2(state: &mut MirMachineStateV2, fallback: u32) -> Option<()> {
    match state.pc {
        MirPcV2::CallerCall => {
            state.helper_argument = state.root_argument;
            state.pc = MirPcV2::HelperSwitch;
        }
        MirPcV2::HelperSwitch => {
            state.pc = if state.helper_argument == 0 {
                MirPcV2::ZeroArm
            } else {
                MirPcV2::NonzeroArm
            };
        }
        MirPcV2::ZeroArm => {
            state.helper_return = state.helper_argument;
            state.arm = Some(MirKirCfgEventV2::ZeroArm);
            state.pc = MirPcV2::HelperJoin;
        }
        MirPcV2::NonzeroArm => {
            state.helper_return = fallback;
            state.arm = Some(MirKirCfgEventV2::NonzeroArm);
            state.pc = MirPcV2::HelperJoin;
        }
        MirPcV2::HelperJoin => state.pc = MirPcV2::HelperReturn,
        MirPcV2::HelperReturn => state.pc = MirPcV2::CallerContinuation,
        MirPcV2::CallerContinuation => {
            state.call_destination = state.helper_return;
            state.pc = MirPcV2::Done;
        }
        MirPcV2::Done => return None,
    }
    Some(())
}

/// Executes the semantic-MIR model, returning `None` on insufficient fuel.
pub fn execute_mir_u32_diamond_call_v2(
    input: u32,
    fallback: u32,
    fuel: u8,
) -> Option<MirKirCfgObservationV2> {
    let mut remaining = fuel;
    let mut state = initial_mir_state_v2(input);
    for _ in MIR_KIR_CFG_MACRO_STEPS_V2 {
        consume_macro_step_v2(&mut remaining)?;
        mir_macro_step_v2(&mut state, fallback)?;
    }
    if state.pc != MirPcV2::Done {
        return None;
    }
    Some(MirKirCfgObservationV2 {
        result: state.call_destination,
        trace: [
            MirKirCfgEventV2::Call,
            state.arm?,
            MirKirCfgEventV2::Join,
            MirKirCfgEventV2::Return(state.call_destination),
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
    call_result_is_helper_return: bool,
}

impl KirCfgShapeV2 {
    const EXACT: Self = Self {
        callee_is_helper: true,
        reverse_branch: false,
        swap_edge_arguments: false,
        return_join_parameter: true,
        call_result_is_helper_return: true,
    };
}

fn execute_kir_with_shape_v2(
    input: u32,
    fallback: u32,
    fuel: u8,
    shape: KirCfgShapeV2,
) -> Option<MirKirCfgObservationV2> {
    let mut remaining = fuel;
    let mut state = KirMachineStateV2 {
        pc: KirPcV2::RootCall,
        root_parameter: input,
        helper_parameter: 0,
        fallback_value: fallback,
        selected_edge_value: 0,
        join_parameter: 0,
        helper_return_operand: 0,
        call_result: 0,
        arm: None,
    };
    for step in MIR_KIR_CFG_MACRO_STEPS_V2 {
        consume_macro_step_v2(&mut remaining)?;
        match (step, state.pc) {
            (MirKirCfgMacroStepV2::EnterHelper, KirPcV2::RootCall) => {
                if !shape.callee_is_helper {
                    return None;
                }
                state.helper_parameter = state.root_parameter;
                state.pc = KirPcV2::HelperEntry;
            }
            (MirKirCfgMacroStepV2::SelectArm, KirPcV2::HelperEntry) => {
                state.pc = if (state.helper_parameter == 0) != shape.reverse_branch {
                    KirPcV2::ZeroBlock
                } else {
                    KirPcV2::NonzeroBlock
                };
            }
            (MirKirCfgMacroStepV2::DefineArmValue, KirPcV2::ZeroBlock) => {
                state.selected_edge_value = if shape.swap_edge_arguments {
                    state.fallback_value
                } else {
                    state.helper_parameter
                };
                state.arm = Some(MirKirCfgEventV2::ZeroArm);
                state.pc = KirPcV2::JoinBlock;
            }
            (MirKirCfgMacroStepV2::DefineArmValue, KirPcV2::NonzeroBlock) => {
                state.selected_edge_value = if shape.swap_edge_arguments {
                    state.helper_parameter
                } else {
                    state.fallback_value
                };
                state.arm = Some(MirKirCfgEventV2::NonzeroArm);
                state.pc = KirPcV2::JoinBlock;
            }
            (MirKirCfgMacroStepV2::EnterJoin, KirPcV2::JoinBlock) => {
                state.join_parameter = state.selected_edge_value;
                state.pc = KirPcV2::HelperReturn;
            }
            (MirKirCfgMacroStepV2::ReturnFromHelper, KirPcV2::HelperReturn) => {
                state.helper_return_operand = if shape.return_join_parameter {
                    state.join_parameter
                } else {
                    state.join_parameter.wrapping_add(1)
                };
                state.pc = KirPcV2::RootContinuation;
            }
            (MirKirCfgMacroStepV2::ObserveCallResult, KirPcV2::RootContinuation) => {
                state.call_result = if shape.call_result_is_helper_return {
                    state.helper_return_operand
                } else {
                    state.helper_return_operand.wrapping_add(1)
                };
                state.pc = KirPcV2::Done;
            }
            _ => return None,
        }
    }
    let result = state.call_result;
    Some(MirKirCfgObservationV2 {
        result,
        trace: [
            MirKirCfgEventV2::Call,
            state.arm?,
            MirKirCfgEventV2::Join,
            MirKirCfgEventV2::Return(result),
        ],
    })
}

/// Checks equality of the two executable observations.
pub fn mir_kir_u32_diamond_call_refines_v2(input: u32, fallback: u32, fuel: u8) -> bool {
    match (
        execute_mir_u32_diamond_call_v2(input, fallback, fuel),
        execute_kir_u32_diamond_call_v2(input, fallback, fuel),
    ) {
        (Some(mir), Some(kir)) => mir == kir,
        _ => false,
    }
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
    /// Semantic caller call block.
    pub semantic_root_entry: u32,
    /// Semantic caller continuation where the observation boundary ends.
    pub semantic_root_continuation: u32,
    /// Semantic helper switch block.
    pub semantic_helper_entry: u32,
    /// Semantic helper zero arm.
    pub semantic_zero_arm: u32,
    /// Semantic helper default arm.
    pub semantic_nonzero_arm: u32,
    /// Semantic helper join/return block.
    pub semantic_join: u32,
    /// KIR caller parameter.
    pub kir_root_parameter: ValueId,
    /// KIR direct-call result.
    pub kir_call_result: ValueId,
    /// KIR caller entry block.
    pub kir_root_entry: u32,
    /// KIR caller continuation where the observation boundary ends.
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

/// Explicit outcome of checking the optional bounded CFG language.
///
/// `NotEligible` makes no V2 refinement claim. An exact eligible semantic shape
/// whose KIR relation is missing returns an error instead of this status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirKirCfgRefinementStatusV2 {
    /// The semantic program is outside the exact two-function V2 language.
    NotEligible,
    /// The eligible program has complete replayed production evidence.
    Verified(InertMirKirCfgRefinementEvidenceV2),
}

impl MirKirCfgRefinementStatusV2 {
    /// Classifies optional coverage without conflating absence with verification.
    pub fn from_live_owner(
        owner: &ProductionSemanticKirOwnerV1,
    ) -> Result<Self, MirKirCfgRefinementErrorV2> {
        classify_cfg_derivation_v2(InertMirKirCfgRefinementEvidenceV2::from_live_owner(owner))
    }

    /// Replays a live owner and checks that its coverage status did not change.
    pub fn revalidate_against(
        &self,
        owner: &ProductionSemanticKirOwnerV1,
    ) -> Result<(), MirKirCfgRefinementErrorV2> {
        let replayed = Self::from_live_owner(owner)?;
        if &replayed != self {
            return Err(MirKirCfgRefinementErrorV2::NonCanonicalEvidence);
        }
        Ok(())
    }

    /// Returns evidence only when the exact eligible shape was verified.
    pub const fn evidence(&self) -> Option<&InertMirKirCfgRefinementEvidenceV2> {
        match self {
            Self::NotEligible => None,
            Self::Verified(evidence) => Some(evidence),
        }
    }

    /// Optional status and evidence grant no artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl InertMirKirCfgRefinementEvidenceV2 {
    /// Replays and validates the exact helper/call-result semantic and KIR slice.
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
        let roots = semantic
            .functions()
            .iter()
            .enumerate()
            .filter(|(_, function)| function.role() == SemanticFunctionRoleV1::KernelRoot)
            .collect::<Vec<_>>();
        let helpers = semantic
            .functions()
            .iter()
            .enumerate()
            .filter(|(_, function)| function.role() == SemanticFunctionRoleV1::InternalHelper)
            .collect::<Vec<_>>();
        let ([(root_index, root)], [(helper_index, helper)]) =
            (roots.as_slice(), helpers.as_slice())
        else {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        };
        let (root_id, helper_id) = (
            u32::try_from(*root_index).map_err(|_| MirKirCfgRefinementErrorV2::UnsupportedShape)?,
            u32::try_from(*helper_index)
                .map_err(|_| MirKirCfgRefinementErrorV2::UnsupportedShape)?,
        );
        if helper.blocks().len() != 4 || helper.locals().len() != 2 {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }
        if semantic
            .roots()
            .iter()
            .map(|root| root.index())
            .collect::<Vec<_>>()
            != [root_id]
            || owner.module().kernels.len() != 1
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

        let helper_entry = helper.entry().index();
        let helper_entry_block = helper
            .blocks()
            .get(helper_entry as usize)
            .ok_or(MirKirCfgRefinementErrorV2::UnsupportedShape)?;
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = helper_entry_block.terminator().kind()
        else {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        };
        let zero_target = exact_zero_switch_target_v2(targets)?;
        let zero_arm = zero_target.edge().target().index();
        let nonzero_arm = targets.otherwise().target().index();
        if !helper_entry_block.statements().is_empty()
            || copied_local(discriminant) != Some(helper_argument)
            || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
            || zero_arm == helper_entry
            || nonzero_arm == helper_entry
            || zero_arm == nonzero_arm
            || zero_arm as usize >= helper.blocks().len()
            || nonzero_arm as usize >= helper.blocks().len()
        {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }
        let zero_join = goto_target(helper, zero_arm as usize)?;
        let nonzero_join = goto_target(helper, nonzero_arm as usize)?;
        if zero_join != nonzero_join
            || zero_join == helper_entry
            || zero_join == zero_arm
            || zero_join == nonzero_arm
            || zero_join as usize >= helper.blocks().len()
        {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }
        let join = zero_join;
        require_arm_copy(
            helper,
            zero_arm as usize,
            helper_return,
            helper_argument,
            join,
        )?;
        let fallback = require_arm_constant(
            semantic.types(),
            helper,
            nonzero_arm as usize,
            helper_return,
            join,
        )?;
        if !helper.blocks()[join as usize].statements().is_empty()
            || !matches!(
                helper.blocks()[join as usize].terminator().kind(),
                SemanticTerminatorKindV1::Return
            )
        {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }

        let root_entry_semantic = root.entry().index();
        let SemanticTerminatorKindV1::Call(call) = root.blocks()[root_entry_semantic as usize]
            .terminator()
            .kind()
        else {
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
        let root_continuation_semantic = call_destination.edge().target().index();
        require_u32_local(semantic.types(), root, root_call_result)?;
        if root.locals()[root_call_result as usize].role() != SemanticLocalRoleV1::Temporary {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }
        if root_continuation_semantic == root_entry_semantic
            || root_continuation_semantic as usize >= root.blocks().len()
            || !root.blocks()[root_entry_semantic as usize]
                .statements()
                .is_empty()
            || call.callee().index() != helper_id
            || call.arguments().len() != 1
            || copied_local(&call.arguments()[0]) != Some(root_argument)
            || call.unwind() != SemanticUnwindActionV1::Unreachable
            || !call_destination.place().projections().is_empty()
            || call_destination.edge().role() != SemanticEdgeRoleV1::CallReturn
        {
            return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
        }

        let correspondence = owner.correspondence();
        if correspondence.lowered_functions().len() != 2
            || correspondence
                .statement_operation_spans()
                .iter()
                .filter(|span| span.semantic_function().index() == helper_id)
                .count()
                != 2
            || correspondence
                .terminator_operation_spans()
                .iter()
                .filter(|span| span.semantic_function().index() == helper_id)
                .count()
                != 4
            || correspondence
                .terminator_operation_spans()
                .iter()
                .filter(|span| {
                    span.semantic_function().index() == root_id
                        && span.semantic_block().index() == root_entry_semantic
                })
                .count()
                != 1
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }
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
            || kir_root.signature.parameters.first() != Some(&Type::Scalar(ScalarType::U32))
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
        let root_entry = block_for(correspondence, root_id, root_entry_semantic)?;
        let root_continuation = block_for(correspondence, root_id, root_continuation_semantic)?;
        let root_block = root_body
            .blocks
            .iter()
            .find(|block| block.id == root_entry)
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        let call_span = correspondence
            .terminator_operation_spans()
            .iter()
            .find(|span| {
                span.semantic_function().index() == root_id
                    && span.semantic_block().index() == root_entry_semantic
            })
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
        if call_span.kernel_ir_block() != root_entry
            || call_span.first_operation_ordinal() != 0
            || call_span.operation_count() != 1
            || root_block.operations.len() != 1
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }
        let call_operation = root_block
            .operations
            .get(call_span.first_operation_ordinal() as usize)
            .ok_or(MirKirCfgRefinementErrorV2::Correspondence)?;
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
        if root_body.parameters.first() != Some(&kir_root_parameter)
            || !matches!(root_block.terminator.as_ref(), Some(Terminator::Branch { target, arguments }) if *target == root_continuation && arguments.is_empty())
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }
        if root_body
            .blocks
            .iter()
            .all(|block| block.id != root_continuation)
        {
            return Err(MirKirCfgRefinementErrorV2::Correspondence);
        }

        let entry = block_for(correspondence, helper_id, helper_entry)?;
        let zero = block_for(correspondence, helper_id, zero_arm)?;
        let nonzero = block_for(correspondence, helper_id, nonzero_arm)?;
        let kir_join = block_for(correspondence, helper_id, join)?;
        // Bind EnterHelper/ObserveCallResult to the call operation, result,
        // destination, and continuation edge; SelectArm to the switch;
        // DefineArmValue/EnterJoin to the selected assignment and edge
        // argument; and ReturnFromHelper to the join return operand.
        // Zero-operation spans are meaningful because copy and CFG edges do
        // not fabricate KIR operations.
        require_terminator_span(
            correspondence,
            root_id,
            root_entry_semantic,
            root_entry,
            0,
            1,
        )?;
        require_terminator_span(correspondence, helper_id, helper_entry, entry, 0, 0)?;
        require_statement_span(correspondence, helper_id, zero_arm, 0, zero, 0, 0)?;
        require_terminator_span(correspondence, helper_id, zero_arm, zero, 0, 0)?;
        require_statement_span(correspondence, helper_id, nonzero_arm, 0, nonzero, 0, 1)?;
        require_terminator_span(correspondence, helper_id, nonzero_arm, nonzero, 1, 0)?;
        require_terminator_span(correspondence, helper_id, join, kir_join, 0, 0)?;
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
            .find(|block| block.id == kir_join)
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
        require_branch(zero_block, kir_join, kir_helper_parameter)?;
        require_branch(nonzero_block, kir_join, fallback_value)?;
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
            semantic_root_entry: root_entry_semantic,
            semantic_root_continuation: root_continuation_semantic,
            semantic_helper_entry: helper_entry,
            semantic_zero_arm: zero_arm,
            semantic_nonzero_arm: nonzero_arm,
            semantic_join: join,
            kir_root_parameter,
            kir_call_result: call_result.id,
            kir_root_entry: root_entry.0,
            kir_root_continuation: root_continuation.0,
            kir_helper_parameter,
            kir_entry: entry.0,
            kir_zero_arm: zero.0,
            kir_nonzero_arm: nonzero.0,
            kir_join: kir_join.0,
            kir_fallback_value: fallback_value,
            kir_join_parameter: join_parameter.id,
        };
        validate_exact_production_relation_v2(ExactProductionCfgRelationV2 {
            expected_callee: helper_binding.kernel_ir_function().clone(),
            actual_callee: callee.clone(),
            expected_call_result: call_result.id,
            retained_call_result: bindings.kir_call_result,
            expected_root_continuation: root_continuation.0,
            actual_root_continuation: branch_target(root_block)?.0,
            expected_zero_edge_value: kir_helper_parameter,
            actual_zero_edge_value: branch_argument(zero_block)?,
            expected_nonzero_edge_value: fallback_value,
            actual_nonzero_edge_value: branch_argument(nonzero_block)?,
            expected_join_parameter: join_parameter.id,
            retained_join_parameter: bindings.kir_join_parameter,
            actual_helper_return_operand: return_operand(join_block)?,
            expected_semantic_call_destination: root_call_result,
            retained_semantic_call_destination: bindings.semantic_call_destination,
            expected_semantic_helper_return: helper_return,
            retained_semantic_helper_return: bindings.semantic_helper_return,
        })?;
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
    /// Returns the exact semantic macro-step charges proved by this evidence.
    pub const fn macro_steps(&self) -> &[MirKirCfgMacroStepV2; 6] {
        &MIR_KIR_CFG_MACRO_STEPS_V2
    }
    /// This evidence never grants artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn exact_zero_switch_target_v2(
    targets: &SemanticSwitchTargetsV1,
) -> Result<&SemanticSwitchTargetV1, MirKirCfgRefinementErrorV2> {
    let [target] = targets.values() else {
        return Err(MirKirCfgRefinementErrorV2::UnsupportedShape);
    };
    (target.value() == 0 && target.edge().role() == SemanticEdgeRoleV1::SwitchValue)
        .then_some(target)
        .ok_or(MirKirCfgRefinementErrorV2::UnsupportedShape)
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

fn classify_cfg_derivation_v2(
    result: Result<InertMirKirCfgRefinementEvidenceV2, MirKirCfgRefinementErrorV2>,
) -> Result<MirKirCfgRefinementStatusV2, MirKirCfgRefinementErrorV2> {
    match result {
        Ok(evidence) => Ok(MirKirCfgRefinementStatusV2::Verified(evidence)),
        Err(MirKirCfgRefinementErrorV2::UnsupportedShape) => {
            Ok(MirKirCfgRefinementStatusV2::NotEligible)
        }
        Err(error) => Err(error),
    }
}

#[derive(Clone)]
struct ExactProductionCfgRelationV2 {
    expected_callee: fe2o3_kernel_ir::FunctionId,
    actual_callee: fe2o3_kernel_ir::FunctionId,
    expected_call_result: ValueId,
    retained_call_result: ValueId,
    expected_root_continuation: u32,
    actual_root_continuation: u32,
    expected_zero_edge_value: ValueId,
    actual_zero_edge_value: ValueId,
    expected_nonzero_edge_value: ValueId,
    actual_nonzero_edge_value: ValueId,
    expected_join_parameter: ValueId,
    retained_join_parameter: ValueId,
    actual_helper_return_operand: ValueId,
    expected_semantic_call_destination: u32,
    retained_semantic_call_destination: u32,
    expected_semantic_helper_return: u32,
    retained_semantic_helper_return: u32,
}

/// Final relation check used by live-owner evidence construction. Keeping the
/// producer reads and retained evidence locators separate makes hostile
/// substitution tests exercise the same fail-closed validation path.
fn validate_exact_production_relation_v2(
    relation: ExactProductionCfgRelationV2,
) -> Result<(), MirKirCfgRefinementErrorV2> {
    (relation.actual_callee == relation.expected_callee
        && relation.retained_call_result == relation.expected_call_result
        && relation.actual_root_continuation == relation.expected_root_continuation
        && relation.actual_zero_edge_value == relation.expected_zero_edge_value
        && relation.actual_nonzero_edge_value == relation.expected_nonzero_edge_value
        && relation.retained_join_parameter == relation.expected_join_parameter
        && relation.actual_helper_return_operand == relation.expected_join_parameter
        && relation.retained_semantic_call_destination
            == relation.expected_semantic_call_destination
        && relation.retained_semantic_helper_return == relation.expected_semantic_helper_return)
        .then_some(())
        .ok_or(MirKirCfgRefinementErrorV2::Correspondence)
}

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
fn goto_target(
    function: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1,
    block: usize,
) -> Result<u32, MirKirCfgRefinementErrorV2> {
    match function
        .blocks()
        .get(block)
        .map(|block| block.terminator().kind())
    {
        Some(SemanticTerminatorKindV1::Goto(edge)) if edge.role() == SemanticEdgeRoleV1::Goto => {
            Ok(edge.target().index())
        }
        _ => Err(MirKirCfgRefinementErrorV2::UnsupportedShape),
    }
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
fn branch_target(
    block: &fe2o3_kernel_ir::BasicBlock,
) -> Result<fe2o3_kernel_ir::BlockId, MirKirCfgRefinementErrorV2> {
    match block.terminator.as_ref() {
        Some(Terminator::Branch { target, .. }) => Ok(*target),
        _ => Err(MirKirCfgRefinementErrorV2::Correspondence),
    }
}
fn branch_argument(
    block: &fe2o3_kernel_ir::BasicBlock,
) -> Result<ValueId, MirKirCfgRefinementErrorV2> {
    match block.terminator.as_ref() {
        Some(Terminator::Branch { arguments, .. }) if arguments.len() == 1 => Ok(arguments[0]),
        _ => Err(MirKirCfgRefinementErrorV2::Correspondence),
    }
}
fn return_operand(
    block: &fe2o3_kernel_ir::BasicBlock,
) -> Result<ValueId, MirKirCfgRefinementErrorV2> {
    match block.terminator.as_ref() {
        Some(Terminator::Return { values }) if values.len() == 1 => Ok(values[0]),
        _ => Err(MirKirCfgRefinementErrorV2::Correspondence),
    }
}
#[allow(clippy::too_many_arguments)]
fn require_statement_span(
    correspondence: &crate::SemanticKirCorrespondenceV1,
    function: u32,
    block: u32,
    statement: u32,
    kir_block: fe2o3_kernel_ir::BlockId,
    first_operation: u32,
    operation_count: u32,
) -> Result<(), MirKirCfgRefinementErrorV2> {
    let matches = correspondence
        .statement_operation_spans()
        .iter()
        .filter(|span| {
            span.semantic_function().index() == function
                && span.semantic_block().index() == block
                && span.statement_ordinal() == statement
                && span.kernel_ir_block() == kir_block
                && span.first_operation_ordinal() == first_operation
                && span.operation_count() == operation_count
        })
        .count();
    (matches == 1)
        .then_some(())
        .ok_or(MirKirCfgRefinementErrorV2::Correspondence)
}
fn require_terminator_span(
    correspondence: &crate::SemanticKirCorrespondenceV1,
    function: u32,
    block: u32,
    kir_block: fe2o3_kernel_ir::BlockId,
    first_operation: u32,
    operation_count: u32,
) -> Result<(), MirKirCfgRefinementErrorV2> {
    let matches = correspondence
        .terminator_operation_spans()
        .iter()
        .filter(|span| {
            span.semantic_function().index() == function
                && span.semantic_block().index() == block
                && span.kernel_ir_block() == kir_block
                && span.first_operation_ordinal() == first_operation
                && span.operation_count() == operation_count
        })
        .count();
    (matches == 1)
        .then_some(())
        .ok_or(MirKirCfgRefinementErrorV2::Correspondence)
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
        bindings.semantic_root_entry,
        bindings.semantic_root_continuation,
        bindings.semantic_helper_entry,
        bindings.semantic_zero_arm,
        bindings.semantic_nonzero_arm,
        bindings.semantic_join,
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
            assert!(mir_kir_u32_diamond_call_refines_v2(
                input,
                17,
                MIR_KIR_CFG_REQUIRED_FUEL_V2
            ));
        }
        assert_eq!(
            execute_mir_u32_diamond_call_v2(1, 17, MIR_KIR_CFG_REQUIRED_FUEL_V2 - 1),
            None
        );
        assert!(!mir_kir_u32_diamond_call_refines_v2(
            1,
            17,
            MIR_KIR_CFG_REQUIRED_FUEL_V2 - 1
        ));
        assert_eq!(
            execute_mir_u32_diamond_call_v2(0, 17, MIR_KIR_CFG_REQUIRED_FUEL_V2)
                .unwrap()
                .result,
            0
        );
        assert_eq!(
            execute_mir_u32_diamond_call_v2(1, 17, MIR_KIR_CFG_REQUIRED_FUEL_V2)
                .unwrap()
                .result,
            17
        );
    }
    #[test]
    fn empty_or_nonzero_switch_rosters_are_not_eligible_without_panicking() {
        let otherwise = fe2o3_mir_model::semantic_mir_v1::SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::SwitchOtherwise,
            fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1::from_index(1),
        );
        let empty = SemanticSwitchTargetsV1::new(Vec::new(), otherwise).unwrap();
        assert_eq!(
            exact_zero_switch_target_v2(&empty),
            Err(MirKirCfgRefinementErrorV2::UnsupportedShape)
        );

        let nonzero = SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                fe2o3_mir_model::semantic_mir_v1::SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1::from_index(2),
                ),
            )],
            otherwise,
        )
        .unwrap();
        assert_eq!(
            exact_zero_switch_target_v2(&nonzero),
            Err(MirKirCfgRefinementErrorV2::UnsupportedShape)
        );
    }
    #[test]
    fn branch_direction_phi_and_return_mutations_change_observation() {
        let expected =
            execute_mir_u32_diamond_call_v2(1, 17, MIR_KIR_CFG_REQUIRED_FUEL_V2).unwrap();
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
                execute_kir_with_shape_v2(1, 17, MIR_KIR_CFG_REQUIRED_FUEL_V2, hostile).unwrap()
            );
        }
        assert_eq!(
            execute_kir_with_shape_v2(
                1,
                17,
                MIR_KIR_CFG_REQUIRED_FUEL_V2,
                KirCfgShapeV2 {
                    callee_is_helper: false,
                    ..KirCfgShapeV2::EXACT
                }
            ),
            None
        );
        assert_ne!(
            expected,
            execute_kir_with_shape_v2(
                1,
                17,
                MIR_KIR_CFG_REQUIRED_FUEL_V2,
                KirCfgShapeV2 {
                    call_result_is_helper_return: false,
                    ..KirCfgShapeV2::EXACT
                }
            )
            .unwrap()
        );
    }
    #[test]
    fn live_validator_rejects_each_retained_cfg_relation_mutation() {
        let exact = ExactProductionCfgRelationV2 {
            expected_callee: fe2o3_kernel_ir::FunctionId::new("helper"),
            actual_callee: fe2o3_kernel_ir::FunctionId::new("helper"),
            expected_call_result: ValueId(2),
            retained_call_result: ValueId(2),
            expected_root_continuation: 3,
            actual_root_continuation: 3,
            expected_zero_edge_value: ValueId(4),
            actual_zero_edge_value: ValueId(4),
            expected_nonzero_edge_value: ValueId(5),
            actual_nonzero_edge_value: ValueId(5),
            expected_join_parameter: ValueId(6),
            retained_join_parameter: ValueId(6),
            actual_helper_return_operand: ValueId(6),
            expected_semantic_call_destination: 7,
            retained_semantic_call_destination: 7,
            expected_semantic_helper_return: 8,
            retained_semantic_helper_return: 8,
        };
        assert_eq!(validate_exact_production_relation_v2(exact.clone()), Ok(()));
        let mutations = [
            ExactProductionCfgRelationV2 {
                actual_callee: fe2o3_kernel_ir::FunctionId::new("other"),
                ..exact.clone()
            },
            ExactProductionCfgRelationV2 {
                retained_call_result: ValueId(9),
                ..exact.clone()
            },
            ExactProductionCfgRelationV2 {
                actual_root_continuation: 9,
                ..exact.clone()
            },
            ExactProductionCfgRelationV2 {
                actual_zero_edge_value: ValueId(9),
                ..exact.clone()
            },
            ExactProductionCfgRelationV2 {
                actual_nonzero_edge_value: ValueId(9),
                ..exact.clone()
            },
            ExactProductionCfgRelationV2 {
                retained_join_parameter: ValueId(9),
                ..exact.clone()
            },
            ExactProductionCfgRelationV2 {
                actual_helper_return_operand: ValueId(9),
                ..exact.clone()
            },
            ExactProductionCfgRelationV2 {
                retained_semantic_call_destination: 9,
                ..exact.clone()
            },
            ExactProductionCfgRelationV2 {
                retained_semantic_helper_return: 9,
                ..exact.clone()
            },
        ];
        for mutation in mutations {
            assert_eq!(
                validate_exact_production_relation_v2(mutation),
                Err(MirKirCfgRefinementErrorV2::Correspondence)
            );
        }
    }
    #[test]
    fn exact_eligible_shape_cannot_be_classified_as_missing_optional_evidence() {
        assert_eq!(
            classify_cfg_derivation_v2(Err(MirKirCfgRefinementErrorV2::UnsupportedShape)),
            Ok(MirKirCfgRefinementStatusV2::NotEligible)
        );
        assert_eq!(
            classify_cfg_derivation_v2(Err(MirKirCfgRefinementErrorV2::Correspondence)),
            Err(MirKirCfgRefinementErrorV2::Correspondence)
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
