//! Exact production custody for a two-loaded-`u32` XOR/diamond call slice.
//!
//! The observed root call takes two direct `u32` locals, each defined by one
//! semantic load and retained as the unique `u32` KIR load result in that
//! statement span. The helper computes `left ^ right`, branches on that SSA
//! value being zero, transfers either that value or one `u32` constant through
//! a join block argument, and returns it to the root call destination. Load and
//! root continuation memory behavior are outside this theorem.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    BinaryOp, FunctionRole, OperationKind, ScalarType, Terminator, Type, ValueId,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBinaryOpV1, SemanticConstantValueV1, SemanticDirectCallV1, SemanticEdgeRoleV1,
    SemanticFunctionDeclV1, SemanticFunctionRoleV1, SemanticLocalRoleV1, SemanticOperandV1,
    SemanticRvalueKindV1, SemanticScalarTypeV1, SemanticStatementKindV1, SemanticTerminatorKindV1,
    SemanticTypeDeclV1, SemanticTypeShapeV1, SemanticUnwindActionV1,
};
use sha2::{Digest, Sha256};

use crate::{
    FORMAL_COMPILER_V3_BRANCH_ARMS, FORMAL_COMPILER_V3_BYTE_WIDTH, FORMAL_COMPILER_V3_CLAIM_NAME,
    FORMAL_COMPILER_V3_GUARDED_ALLOCATIONS, FORMAL_COMPILER_V3_HELPER_PARAMETERS,
    FORMAL_COMPILER_V3_PRODUCTION_LOOP_TRIP_COUNT, FORMAL_COMPILER_V3_PRODUCTION_SCALAR_OPERATIONS,
    FORMAL_COMPILER_V3_PRODUCTION_STACK_FRAMES, FORMAL_COMPILER_V3_WORD_BITS,
    MIR_KIR_CFG_REFINEMENT_PROOF_SHA256_V2, MIR_KIR_SCALAR_REFINEMENT_PROOF_SHA256_V1,
    MIR_KIR_STRUCTURED_CFG_PROOF_SHA256_V3, ProductionCanonicalKernelIrIdentityV1,
    ProductionSemanticKirOwnerV1, SemanticKirCorrespondenceV1,
};

/// Exact production policy version.
pub const MIR_KIR_XOR_CFG_POLICY_VERSION_V3: u16 = 3;
/// Stable theorem relation composed by this custody object.
pub const MIR_KIR_XOR_CFG_THEOREM_V3: &str = "fe2o3_mir_kir_xor_diamond_call_refines_v3";

const MODEL_DOMAIN_V3: &[u8] = b"FE2O3/MIR-KIR/U32-XOR-DIAMOND/MODEL/V3\0";
const EVIDENCE_DOMAIN_V3: &[u8] = b"FE2O3/MIR-KIR/U32-XOR-DIAMOND/EVIDENCE/V3\0";

/// Exact source/KIR locators retained by the live-owner classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirKirXorCfgBindingsV3 {
    /// Semantic root and helper function IDs.
    pub semantic_functions: [u32; 2],
    /// Root left/right load destinations and call destination.
    pub semantic_root_values: [u32; 3],
    /// Semantic `(block, statement)` locators for the two defining loads.
    pub semantic_root_load_sites: [[u32; 2]; 2],
    /// Helper left/right arguments, XOR destination, and return local.
    pub semantic_helper_values: [u32; 4],
    /// Root entry and observation continuation blocks.
    pub semantic_root_blocks: [u32; 2],
    /// Helper entry, zero arm, nonzero arm, and join blocks.
    pub semantic_helper_blocks: [u32; 4],
    /// KIR root left/right load results and call result.
    pub kir_root_values: [ValueId; 3],
    /// KIR `(block, operation)` locators for the two defining loads.
    pub kir_root_load_sites: [[u32; 2]; 2],
    /// KIR `(block, operation)` locator for the direct helper call.
    pub kir_root_call_site: [u32; 2],
    /// KIR helper left/right parameters, XOR result, fallback, and join parameter.
    pub kir_helper_values: [ValueId; 5],
    /// KIR root entry and observation continuation blocks.
    pub kir_root_blocks: [u32; 2],
    /// KIR helper entry, zero arm, nonzero arm, and join blocks.
    pub kir_helper_blocks: [u32; 4],
}

/// Authority-free evidence for one exact live production owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertMirKirXorCfgEvidenceV3 {
    identity: [u8; 32],
    model_identity: [u8; 32],
    semantic_sha256: [u8; 32],
    canonical_kernel_ir: ProductionCanonicalKernelIrIdentityV1,
    fallback: u32,
    bindings: MirKirXorCfgBindingsV3,
}

/// Explicit optional-coverage status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirKirXorCfgStatusV3 {
    /// The semantic owner is outside the exact V3 production language.
    NotEligible,
    /// The complete source/KIR relation was checked and retained.
    Verified(InertMirKirXorCfgEvidenceV3),
}

impl MirKirXorCfgStatusV3 {
    /// Derives status from a replayed live owner.
    pub fn from_live_owner(
        owner: &ProductionSemanticKirOwnerV1,
    ) -> Result<Self, MirKirXorCfgErrorV3> {
        match InertMirKirXorCfgEvidenceV3::from_live_owner(owner) {
            Ok(evidence) => Ok(Self::Verified(evidence)),
            Err(MirKirXorCfgErrorV3::UnsupportedShape) => Ok(Self::NotEligible),
            Err(error) => Err(error),
        }
    }

    /// Replays the owner and rejects changed status or evidence.
    pub fn revalidate_against(
        &self,
        owner: &ProductionSemanticKirOwnerV1,
    ) -> Result<(), MirKirXorCfgErrorV3> {
        (Self::from_live_owner(owner)? == *self)
            .then_some(())
            .ok_or(MirKirXorCfgErrorV3::NonCanonicalEvidence)
    }

    /// Returns exact evidence only for verified owners.
    pub const fn evidence(&self) -> Option<&InertMirKirXorCfgEvidenceV3> {
        match self {
            Self::NotEligible => None,
            Self::Verified(evidence) => Some(evidence),
        }
    }

    /// Optional custody never grants artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl InertMirKirXorCfgEvidenceV3 {
    /// Replays and validates the exact XOR/diamond helper and root call prefix.
    pub fn from_live_owner(
        owner: &ProductionSemanticKirOwnerV1,
    ) -> Result<Self, MirKirXorCfgErrorV3> {
        owner
            .verify_equivalence()
            .map_err(|error| MirKirXorCfgErrorV3::LiveOwner(error.to_string()))?;
        let semantic = owner.semantic().semantic();
        if semantic.functions().len() != 2 || owner.module().functions.len() != 2 {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
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
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        };
        let root_id =
            u32::try_from(*root_index).map_err(|_| MirKirXorCfgErrorV3::UnsupportedShape)?;
        let helper_id =
            u32::try_from(*helper_index).map_err(|_| MirKirXorCfgErrorV3::UnsupportedShape)?;
        if helper.blocks().len() != 4
            || helper.locals().len() != 4
            || semantic
                .roots()
                .iter()
                .map(|root| root.index())
                .collect::<Vec<_>>()
                != [root_id]
            || owner.module().kernels.len() != 1
        {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        }

        let helper_left = unique_local(helper, SemanticLocalRoleV1::Argument(0))?;
        let helper_right = unique_local(helper, SemanticLocalRoleV1::Argument(1))?;
        let helper_return = unique_local(helper, SemanticLocalRoleV1::Return)?;
        let helper_xor = unique_local(helper, SemanticLocalRoleV1::Temporary)?;
        for local in [helper_left, helper_right, helper_return, helper_xor] {
            require_u32(semantic.types(), helper, local)?;
        }

        let helper_entry = helper.entry().index();
        let [xor_statement] = helper.blocks()[helper_entry as usize].statements() else {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        };
        let SemanticStatementKindV1::Assign(xor_assignment) = xor_statement.kind() else {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        };
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::BitXor,
            left,
            right,
        } = xor_assignment.value().kind()
        else {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        };
        if !xor_assignment.destination().projections().is_empty()
            || xor_assignment.destination().local().index() != helper_xor
            || copied_local(left) != Some(helper_left)
            || copied_local(right) != Some(helper_right)
        {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        }
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = helper.blocks()[helper_entry as usize].terminator().kind()
        else {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        };
        if copied_local(discriminant) != Some(helper_xor)
            || targets.values().len() != 1
            || targets.values()[0].value() != 0
            || targets.values()[0].edge().role() != SemanticEdgeRoleV1::SwitchValue
            || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
        {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        }
        let zero_arm = targets.values()[0].edge().target().index();
        let nonzero_arm = targets.otherwise().target().index();
        if zero_arm == nonzero_arm
            || [zero_arm, nonzero_arm]
                .iter()
                .any(|block| *block as usize >= helper.blocks().len() || *block == helper_entry)
        {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        }
        let join = goto_target(helper, zero_arm)?;
        if goto_target(helper, nonzero_arm)? != join
            || [helper_entry, zero_arm, nonzero_arm].contains(&join)
            || join as usize >= helper.blocks().len()
        {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        }
        require_arm_copy(helper, zero_arm, helper_return, helper_xor, join)?;
        let fallback =
            require_arm_constant(semantic.types(), helper, nonzero_arm, helper_return, join)?;
        if !helper.blocks()[join as usize].statements().is_empty()
            || !matches!(
                helper.blocks()[join as usize].terminator().kind(),
                SemanticTerminatorKindV1::Return
            )
        {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        }

        let (root_call_block, call) = unique_helper_call(root, helper_id)?;
        let destination = call
            .destination()
            .ok_or(MirKirXorCfgErrorV3::UnsupportedShape)?;
        let root_result = destination.place().local().index();
        let root_continuation = destination.edge().target().index();
        let [left_operand, right_operand] = call.arguments() else {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        };
        let root_left = direct_local(left_operand).ok_or(MirKirXorCfgErrorV3::UnsupportedShape)?;
        let root_right =
            direct_local(right_operand).ok_or(MirKirXorCfgErrorV3::UnsupportedShape)?;
        let left_load = unique_load_definition(root, root_left)?;
        let right_load = unique_load_definition(root, root_right)?;
        require_u32(semantic.types(), root, root_left)?;
        require_u32(semantic.types(), root, root_right)?;
        require_u32(semantic.types(), root, root_result)?;
        if root_left == root_right
            || [root_left, root_right].contains(&root_result)
            || call.unwind() != SemanticUnwindActionV1::Unreachable
            || !destination.place().projections().is_empty()
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
            || root.locals()[root_result as usize].role() != SemanticLocalRoleV1::Temporary
            || root_continuation == root_call_block
            || root_continuation as usize >= root.blocks().len()
        {
            return Err(MirKirXorCfgErrorV3::UnsupportedShape);
        }

        let correspondence = owner.correspondence();
        let root_binding = function_binding(correspondence, root_id)?;
        let helper_binding = function_binding(correspondence, helper_id)?;
        let kir_root = owner
            .module()
            .function(root_binding.kernel_ir_function())
            .ok_or(MirKirXorCfgErrorV3::Correspondence)?;
        let kir_helper = owner
            .module()
            .function(helper_binding.kernel_ir_function())
            .ok_or(MirKirXorCfgErrorV3::Correspondence)?;
        if kir_root.role != FunctionRole::KernelEntry
            || kir_helper.role != FunctionRole::InternalHelper
            || !kir_root.signature.results.is_empty()
            || kir_helper.signature.parameters.as_slice()
                != [Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)]
            || kir_helper.signature.results.as_slice() != [Type::Scalar(ScalarType::U32)]
        {
            return Err(MirKirXorCfgErrorV3::Correspondence);
        }
        let kir_helper_left = parameter_value(correspondence, helper_id, helper_left)?;
        let kir_helper_right = parameter_value(correspondence, helper_id, helper_right)?;
        let kir_root_call_block = block_for(correspondence, root_id, root_call_block)?;
        let kir_root_continuation = block_for(correspondence, root_id, root_continuation)?;
        let kir_entry = block_for(correspondence, helper_id, helper_entry)?;
        let kir_zero = block_for(correspondence, helper_id, zero_arm)?;
        let kir_nonzero = block_for(correspondence, helper_id, nonzero_arm)?;
        let kir_join = block_for(correspondence, helper_id, join)?;
        let root_body = kir_root
            .body
            .as_ref()
            .ok_or(MirKirXorCfgErrorV3::Correspondence)?;
        let helper_body = kir_helper
            .body
            .as_ref()
            .ok_or(MirKirXorCfgErrorV3::Correspondence)?;
        let root_block = find_block(root_body, kir_root_call_block)?;
        let entry_block = find_block(helper_body, kir_entry)?;
        let zero_block = find_block(helper_body, kir_zero)?;
        let nonzero_block = find_block(helper_body, kir_nonzero)?;
        let join_block = find_block(helper_body, kir_join)?;

        require_statement_span(correspondence, helper_id, helper_entry, 0, kir_entry, 0, 1)?;
        require_terminator_span(correspondence, helper_id, helper_entry, kir_entry, 1, 0)?;
        require_statement_span(correspondence, helper_id, zero_arm, 0, kir_zero, 0, 0)?;
        require_terminator_span(correspondence, helper_id, zero_arm, kir_zero, 0, 0)?;
        require_statement_span(correspondence, helper_id, nonzero_arm, 0, kir_nonzero, 0, 1)?;
        require_terminator_span(correspondence, helper_id, nonzero_arm, kir_nonzero, 1, 0)?;
        require_terminator_span(correspondence, helper_id, join, kir_join, 0, 0)?;
        let kir_left_load = unique_kir_load_for_statement(
            correspondence,
            root_body,
            root_id,
            left_load.block,
            left_load.statement,
        )?;
        let kir_right_load = unique_kir_load_for_statement(
            correspondence,
            root_body,
            root_id,
            right_load.block,
            right_load.statement,
        )?;
        let root_call_operation = unique_terminator_operation(
            correspondence,
            root_body,
            root_id,
            root_call_block,
            kir_root_call_block,
        )?;

        let [xor_operation] = entry_block.operations.as_slice() else {
            return Err(MirKirXorCfgErrorV3::Correspondence);
        };
        let ([xor_result], OperationKind::Binary { op, lhs, rhs }) =
            (xor_operation.results.as_slice(), &xor_operation.kind)
        else {
            return Err(MirKirXorCfgErrorV3::Correspondence);
        };
        if *op != BinaryOp::BitXor
            || *lhs != kir_helper_left
            || *rhs != kir_helper_right
            || xor_result.ty != Type::Scalar(ScalarType::U32)
            || !xor_operation.memory_effects().is_empty()
            || !matches!(entry_block.terminator.as_ref(), Some(Terminator::Switch { selector, cases, default_target, default_arguments })
                if *selector == xor_result.id && cases.len() == 1 && cases[0].value == 0
                    && cases[0].target == kir_zero && cases[0].arguments.is_empty()
                    && *default_target == kir_nonzero && default_arguments.is_empty())
            || !entry_block.parameters.is_empty()
        {
            return Err(MirKirXorCfgErrorV3::Correspondence);
        }
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
            _ => return Err(MirKirXorCfgErrorV3::Correspondence),
        };
        if !zero_block.operations.is_empty()
            || !zero_block.parameters.is_empty()
            || !nonzero_block.parameters.is_empty()
            || helper_body.blocks.len() != 4
            || helper_body.parameters.as_slice() != [kir_helper_left, kir_helper_right]
        {
            return Err(MirKirXorCfgErrorV3::Correspondence);
        }
        require_branch(zero_block, kir_join, xor_result.id)?;
        require_branch(nonzero_block, kir_join, fallback_value)?;
        let [join_parameter] = join_block.parameters.as_slice() else {
            return Err(MirKirXorCfgErrorV3::Correspondence);
        };
        if join_parameter.ty != Type::Scalar(ScalarType::U32)
            || !join_block.operations.is_empty()
            || !matches!(join_block.terminator.as_ref(), Some(Terminator::Return { values }) if values.as_slice() == [join_parameter.id])
        {
            return Err(MirKirXorCfgErrorV3::Correspondence);
        }
        let ([call_result], OperationKind::Call { callee, arguments }) = (
            root_call_operation.operation.results.as_slice(),
            &root_call_operation.operation.kind,
        ) else {
            return Err(MirKirXorCfgErrorV3::Correspondence);
        };
        if *callee != *helper_binding.kernel_ir_function()
            || arguments.as_slice() != [kir_left_load.value, kir_right_load.value]
            || call_result.ty != Type::Scalar(ScalarType::U32)
            || !root_call_operation.operation.memory_effects().is_empty()
            || !matches!(root_block.terminator.as_ref(), Some(Terminator::Branch { target, arguments })
                if *target == kir_root_continuation && arguments.is_empty())
        {
            return Err(MirKirXorCfgErrorV3::Correspondence);
        }

        let bindings = MirKirXorCfgBindingsV3 {
            semantic_functions: [root_id, helper_id],
            semantic_root_values: [root_left, root_right, root_result],
            semantic_root_load_sites: [
                [left_load.block, left_load.statement],
                [right_load.block, right_load.statement],
            ],
            semantic_helper_values: [helper_left, helper_right, helper_xor, helper_return],
            semantic_root_blocks: [root_call_block, root_continuation],
            semantic_helper_blocks: [helper_entry, zero_arm, nonzero_arm, join],
            kir_root_values: [kir_left_load.value, kir_right_load.value, call_result.id],
            kir_root_load_sites: [
                [kir_left_load.block, kir_left_load.operation_ordinal],
                [kir_right_load.block, kir_right_load.operation_ordinal],
            ],
            kir_root_call_site: [
                root_call_operation.block,
                root_call_operation.operation_ordinal,
            ],
            kir_helper_values: [
                kir_helper_left,
                kir_helper_right,
                xor_result.id,
                fallback_value,
                join_parameter.id,
            ],
            kir_root_blocks: [kir_root_call_block.0, kir_root_continuation.0],
            kir_helper_blocks: [kir_entry.0, kir_zero.0, kir_nonzero.0, kir_join.0],
        };
        validate_exact_relation_v3(ExactXorRelationV3 {
            expected_callee: helper_binding.kernel_ir_function().clone(),
            actual_callee: callee.clone(),
            expected_call_arguments: [kir_left_load.value, kir_right_load.value],
            actual_call_arguments: [arguments[0], arguments[1]],
            retained_load_results: [bindings.kir_root_values[0], bindings.kir_root_values[1]],
            expected_expression: xor_result.id,
            switch_selector: switch_selector(entry_block)?,
            zero_edge: branch_argument(zero_block)?,
            expected_fallback: fallback_value,
            nonzero_edge: branch_argument(nonzero_block)?,
            join_parameter: join_parameter.id,
            return_operand: return_operand(join_block)?,
            call_result: call_result.id,
            retained_call_result: bindings.kir_root_values[2],
            semantic_expression: helper_xor,
            retained_semantic_expression: bindings.semantic_helper_values[2],
            semantic_load_destinations: [root_left, root_right],
            retained_semantic_load_destinations: [
                bindings.semantic_root_values[0],
                bindings.semantic_root_values[1],
            ],
            semantic_call_destination: root_result,
            retained_semantic_call_destination: bindings.semantic_root_values[2],
        })?;

        let model_identity = model_identity_v3();
        let semantic_sha256 = *semantic.semantic_sha256().as_bytes();
        let canonical_kernel_ir = owner.canonical_kernel_ir_identity();
        let identity = evidence_identity_v3(
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

    /// Rechecks model/evidence identity.
    pub fn revalidate(&self) -> Result<(), MirKirXorCfgErrorV3> {
        if self.model_identity != model_identity_v3()
            || self.semantic_sha256 == [0; 32]
            || self.canonical_kernel_ir.digest() == &[0; 32]
            || self.identity
                != evidence_identity_v3(
                    self.model_identity,
                    self.semantic_sha256,
                    self.canonical_kernel_ir,
                    self.fallback,
                    self.bindings,
                )
        {
            return Err(MirKirXorCfgErrorV3::NonCanonicalEvidence);
        }
        Ok(())
    }

    /// Returns exact retained bindings.
    pub const fn bindings(&self) -> MirKirXorCfgBindingsV3 {
        self.bindings
    }

    /// Returns the constant selected on the nonzero edge.
    pub const fn fallback(&self) -> u32 {
        self.fallback
    }

    /// Returns the domain-separated evidence identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    /// Evidence grants no artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Fail-closed production classifier error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirKirXorCfgErrorV3 {
    /// Full owner replay failed.
    LiveOwner(String),
    /// Semantic owner is outside the exact two-function shape.
    UnsupportedShape,
    /// KIR spans, control flow, operands, or values do not match.
    Correspondence,
    /// Retained identity or status changed.
    NonCanonicalEvidence,
}

impl fmt::Display for MirKirXorCfgErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LiveOwner(error) => write!(formatter, "live owner failed: {error}"),
            Self::UnsupportedShape => {
                formatter.write_str("owner is outside the u32 XOR/diamond call language")
            }
            Self::Correspondence => {
                formatter.write_str("KIR does not implement the exact XOR/diamond relation")
            }
            Self::NonCanonicalEvidence => {
                formatter.write_str("XOR/diamond evidence is noncanonical")
            }
        }
    }
}

impl Error for MirKirXorCfgErrorV3 {}

#[derive(Clone)]
struct ExactXorRelationV3 {
    expected_callee: fe2o3_kernel_ir::FunctionId,
    actual_callee: fe2o3_kernel_ir::FunctionId,
    expected_call_arguments: [ValueId; 2],
    actual_call_arguments: [ValueId; 2],
    retained_load_results: [ValueId; 2],
    expected_expression: ValueId,
    switch_selector: ValueId,
    zero_edge: ValueId,
    expected_fallback: ValueId,
    nonzero_edge: ValueId,
    join_parameter: ValueId,
    return_operand: ValueId,
    call_result: ValueId,
    retained_call_result: ValueId,
    semantic_expression: u32,
    retained_semantic_expression: u32,
    semantic_load_destinations: [u32; 2],
    retained_semantic_load_destinations: [u32; 2],
    semantic_call_destination: u32,
    retained_semantic_call_destination: u32,
}

fn validate_exact_relation_v3(relation: ExactXorRelationV3) -> Result<(), MirKirXorCfgErrorV3> {
    (relation.actual_callee == relation.expected_callee
        && relation.actual_call_arguments == relation.expected_call_arguments
        && relation.retained_load_results == relation.expected_call_arguments
        && relation.switch_selector == relation.expected_expression
        && relation.zero_edge == relation.expected_expression
        && relation.nonzero_edge == relation.expected_fallback
        && relation.return_operand == relation.join_parameter
        && relation.retained_call_result == relation.call_result
        && relation.retained_semantic_expression == relation.semantic_expression
        && relation.retained_semantic_load_destinations == relation.semantic_load_destinations
        && relation.retained_semantic_call_destination == relation.semantic_call_destination)
        .then_some(())
        .ok_or(MirKirXorCfgErrorV3::Correspondence)
}

fn unique_local(
    function: &SemanticFunctionDeclV1,
    role: SemanticLocalRoleV1,
) -> Result<u32, MirKirXorCfgErrorV3> {
    let found = function
        .locals()
        .iter()
        .enumerate()
        .filter_map(|(index, local)| (local.role() == role).then_some(index as u32))
        .collect::<Vec<_>>();
    match found.as_slice() {
        [local] => Ok(*local),
        _ => Err(MirKirXorCfgErrorV3::UnsupportedShape),
    }
}

fn require_u32(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    local: u32,
) -> Result<(), MirKirXorCfgErrorV3> {
    matches!(
        function
            .locals()
            .get(local as usize)
            .and_then(|local| types.get(local.ty().index() as usize))
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        }))
    )
    .then_some(())
    .ok_or(MirKirXorCfgErrorV3::UnsupportedShape)
}

fn copied_local(operand: &SemanticOperandV1) -> Option<u32> {
    match operand {
        SemanticOperandV1::Copy(place) if place.projections().is_empty() => {
            Some(place.local().index())
        }
        _ => None,
    }
}

fn direct_local(operand: &SemanticOperandV1) -> Option<u32> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            Some(place.local().index())
        }
        _ => None,
    }
}

fn unique_helper_call(
    function: &SemanticFunctionDeclV1,
    helper: u32,
) -> Result<(u32, &SemanticDirectCallV1), MirKirXorCfgErrorV3> {
    let calls = function
        .blocks()
        .iter()
        .enumerate()
        .filter_map(|(block, body)| match body.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) if call.callee().index() == helper => {
                Some((block as u32, call))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    match calls.as_slice() {
        [(block, call)] => Ok((*block, *call)),
        _ => Err(MirKirXorCfgErrorV3::UnsupportedShape),
    }
}

#[derive(Clone, Copy)]
struct SemanticLoadSiteV3 {
    block: u32,
    statement: u32,
}

fn unique_load_definition(
    function: &SemanticFunctionDeclV1,
    destination: u32,
) -> Result<SemanticLoadSiteV3, MirKirXorCfgErrorV3> {
    let loads = function
        .blocks()
        .iter()
        .enumerate()
        .flat_map(|(block, body)| {
            body.statements()
                .iter()
                .enumerate()
                .filter_map(move |(statement, source)| {
                    let SemanticStatementKindV1::Assign(assignment) = source.kind() else {
                        return None;
                    };
                    (assignment.destination().projections().is_empty()
                        && assignment.destination().local().index() == destination
                        && matches!(assignment.value().kind(), SemanticRvalueKindV1::Load(_)))
                    .then_some(SemanticLoadSiteV3 {
                        block: block as u32,
                        statement: statement as u32,
                    })
                })
        })
        .collect::<Vec<_>>();
    match loads.as_slice() {
        [load] => Ok(*load),
        _ => Err(MirKirXorCfgErrorV3::UnsupportedShape),
    }
}

fn goto_target(function: &SemanticFunctionDeclV1, block: u32) -> Result<u32, MirKirXorCfgErrorV3> {
    match function
        .blocks()
        .get(block as usize)
        .map(|block| block.terminator().kind())
    {
        Some(SemanticTerminatorKindV1::Goto(edge)) if edge.role() == SemanticEdgeRoleV1::Goto => {
            Ok(edge.target().index())
        }
        _ => Err(MirKirXorCfgErrorV3::UnsupportedShape),
    }
}

fn require_arm_copy(
    function: &SemanticFunctionDeclV1,
    block: u32,
    destination: u32,
    source: u32,
    join: u32,
) -> Result<(), MirKirXorCfgErrorV3> {
    let [statement] = function.blocks()[block as usize].statements() else {
        return Err(MirKirXorCfgErrorV3::UnsupportedShape);
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Err(MirKirXorCfgErrorV3::UnsupportedShape);
    };
    if !assignment.destination().projections().is_empty()
        || assignment.destination().local().index() != destination
        || !matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(operand) if copied_local(operand) == Some(source))
        || goto_target(function, block)? != join
    {
        return Err(MirKirXorCfgErrorV3::UnsupportedShape);
    }
    Ok(())
}

fn require_arm_constant(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block: u32,
    destination: u32,
    join: u32,
) -> Result<u32, MirKirXorCfgErrorV3> {
    let [statement] = function.blocks()[block as usize].statements() else {
        return Err(MirKirXorCfgErrorV3::UnsupportedShape);
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Err(MirKirXorCfgErrorV3::UnsupportedShape);
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)) =
        assignment.value().kind()
    else {
        return Err(MirKirXorCfgErrorV3::UnsupportedShape);
    };
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return Err(MirKirXorCfgErrorV3::UnsupportedShape);
    };
    if !assignment.destination().projections().is_empty()
        || assignment.destination().local().index() != destination
        || !matches!(
            types
                .get(constant.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32
            }))
        )
        || value.size_bytes() != 4
        || goto_target(function, block)? != join
    {
        return Err(MirKirXorCfgErrorV3::UnsupportedShape);
    }
    u32::try_from(value.bits()).map_err(|_| MirKirXorCfgErrorV3::UnsupportedShape)
}

fn function_binding(
    correspondence: &SemanticKirCorrespondenceV1,
    function: u32,
) -> Result<&crate::SemanticKirFunctionCorrespondenceV1, MirKirXorCfgErrorV3> {
    correspondence
        .lowered_functions()
        .iter()
        .find(|binding| binding.semantic_function().index() == function)
        .ok_or(MirKirXorCfgErrorV3::Correspondence)
}

fn parameter_value(
    correspondence: &SemanticKirCorrespondenceV1,
    function: u32,
    local: u32,
) -> Result<ValueId, MirKirXorCfgErrorV3> {
    correspondence
        .parameter_bindings()
        .iter()
        .find(|binding| {
            binding.semantic_function().index() == function
                && binding.semantic_local().index() == local
        })
        .map(|binding| binding.kernel_ir_value())
        .ok_or(MirKirXorCfgErrorV3::Correspondence)
}

fn block_for(
    correspondence: &SemanticKirCorrespondenceV1,
    function: u32,
    block: u32,
) -> Result<fe2o3_kernel_ir::BlockId, MirKirXorCfgErrorV3> {
    correspondence
        .blocks()
        .iter()
        .find(|binding| {
            binding.semantic_function().index() == function
                && binding.semantic_block().index() == block
        })
        .map(|binding| binding.kernel_ir_block())
        .ok_or(MirKirXorCfgErrorV3::Correspondence)
}

fn find_block(
    body: &fe2o3_kernel_ir::FunctionBody,
    block: fe2o3_kernel_ir::BlockId,
) -> Result<&fe2o3_kernel_ir::BasicBlock, MirKirXorCfgErrorV3> {
    body.blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .ok_or(MirKirXorCfgErrorV3::Correspondence)
}

fn require_branch(
    block: &fe2o3_kernel_ir::BasicBlock,
    target: fe2o3_kernel_ir::BlockId,
    argument: ValueId,
) -> Result<(), MirKirXorCfgErrorV3> {
    matches!(block.terminator.as_ref(), Some(Terminator::Branch { target: actual, arguments })
        if *actual == target && arguments.as_slice() == [argument])
    .then_some(())
    .ok_or(MirKirXorCfgErrorV3::Correspondence)
}

fn branch_argument(block: &fe2o3_kernel_ir::BasicBlock) -> Result<ValueId, MirKirXorCfgErrorV3> {
    match block.terminator.as_ref() {
        Some(Terminator::Branch { arguments, .. }) if arguments.len() == 1 => Ok(arguments[0]),
        _ => Err(MirKirXorCfgErrorV3::Correspondence),
    }
}

fn return_operand(block: &fe2o3_kernel_ir::BasicBlock) -> Result<ValueId, MirKirXorCfgErrorV3> {
    match block.terminator.as_ref() {
        Some(Terminator::Return { values }) if values.len() == 1 => Ok(values[0]),
        _ => Err(MirKirXorCfgErrorV3::Correspondence),
    }
}

fn switch_selector(block: &fe2o3_kernel_ir::BasicBlock) -> Result<ValueId, MirKirXorCfgErrorV3> {
    match block.terminator.as_ref() {
        Some(Terminator::Switch { selector, .. }) => Ok(*selector),
        _ => Err(MirKirXorCfgErrorV3::Correspondence),
    }
}

#[derive(Clone, Copy)]
struct KirOperationSiteV3<'a> {
    operation: &'a fe2o3_kernel_ir::Operation,
    block: u32,
    operation_ordinal: u32,
}

#[derive(Clone, Copy)]
struct KirLoadSiteV3 {
    value: ValueId,
    block: u32,
    operation_ordinal: u32,
}

fn unique_kir_load_for_statement(
    correspondence: &SemanticKirCorrespondenceV1,
    body: &fe2o3_kernel_ir::FunctionBody,
    function: u32,
    block: u32,
    statement: u32,
) -> Result<KirLoadSiteV3, MirKirXorCfgErrorV3> {
    let spans = correspondence
        .statement_operation_spans()
        .iter()
        .filter(|span| {
            span.semantic_function().index() == function
                && span.semantic_block().index() == block
                && span.statement_ordinal() == statement
        })
        .collect::<Vec<_>>();
    let [span] = spans.as_slice() else {
        return Err(MirKirXorCfgErrorV3::Correspondence);
    };
    if span.kernel_ir_block() != block_for(correspondence, function, block)? {
        return Err(MirKirXorCfgErrorV3::Correspondence);
    }
    let kir_block = find_block(body, span.kernel_ir_block())?;
    let first = span.first_operation_ordinal() as usize;
    let end = first
        .checked_add(span.operation_count() as usize)
        .ok_or(MirKirXorCfgErrorV3::Correspondence)?;
    let operations = kir_block
        .operations
        .get(first..end)
        .ok_or(MirKirXorCfgErrorV3::Correspondence)?;
    let loads = operations
        .iter()
        .enumerate()
        .filter_map(
            |(offset, operation)| match (&operation.kind, operation.results.as_slice()) {
                (OperationKind::Load { .. }, [result])
                    if result.ty == Type::Scalar(ScalarType::U32) =>
                {
                    Some((offset, result.id))
                }
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    let [(offset, value)] = loads.as_slice() else {
        return Err(MirKirXorCfgErrorV3::Correspondence);
    };
    let operation_ordinal = first
        .checked_add(*offset)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .ok_or(MirKirXorCfgErrorV3::Correspondence)?;
    Ok(KirLoadSiteV3 {
        value: *value,
        block: span.kernel_ir_block().0,
        operation_ordinal,
    })
}

fn unique_terminator_operation<'a>(
    correspondence: &SemanticKirCorrespondenceV1,
    body: &'a fe2o3_kernel_ir::FunctionBody,
    function: u32,
    block: u32,
    kir_block: fe2o3_kernel_ir::BlockId,
) -> Result<KirOperationSiteV3<'a>, MirKirXorCfgErrorV3> {
    let spans = correspondence
        .terminator_operation_spans()
        .iter()
        .filter(|span| {
            span.semantic_function().index() == function
                && span.semantic_block().index() == block
                && span.kernel_ir_block() == kir_block
        })
        .collect::<Vec<_>>();
    let [span] = spans.as_slice() else {
        return Err(MirKirXorCfgErrorV3::Correspondence);
    };
    if span.operation_count() != 1 {
        return Err(MirKirXorCfgErrorV3::Correspondence);
    }
    let operation_ordinal = span.first_operation_ordinal();
    let operation = find_block(body, kir_block)?
        .operations
        .get(operation_ordinal as usize)
        .ok_or(MirKirXorCfgErrorV3::Correspondence)?;
    Ok(KirOperationSiteV3 {
        operation,
        block: kir_block.0,
        operation_ordinal,
    })
}

#[allow(clippy::too_many_arguments)]
fn require_statement_span(
    correspondence: &SemanticKirCorrespondenceV1,
    function: u32,
    block: u32,
    statement: u32,
    kir_block: fe2o3_kernel_ir::BlockId,
    first_operation: u32,
    operation_count: u32,
) -> Result<(), MirKirXorCfgErrorV3> {
    (correspondence
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
        .count()
        == 1)
        .then_some(())
        .ok_or(MirKirXorCfgErrorV3::Correspondence)
}

fn require_terminator_span(
    correspondence: &SemanticKirCorrespondenceV1,
    function: u32,
    block: u32,
    kir_block: fe2o3_kernel_ir::BlockId,
    first_operation: u32,
    operation_count: u32,
) -> Result<(), MirKirXorCfgErrorV3> {
    (correspondence
        .terminator_operation_spans()
        .iter()
        .filter(|span| {
            span.semantic_function().index() == function
                && span.semantic_block().index() == block
                && span.kernel_ir_block() == kir_block
                && span.first_operation_ordinal() == first_operation
                && span.operation_count() == operation_count
        })
        .count()
        == 1)
        .then_some(())
        .ok_or(MirKirXorCfgErrorV3::Correspondence)
}

fn model_identity_v3() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(MODEL_DOMAIN_V3);
    hash.update(MIR_KIR_XOR_CFG_POLICY_VERSION_V3.to_le_bytes());
    hash.update(MIR_KIR_XOR_CFG_THEOREM_V3.as_bytes());
    hash.update(FORMAL_COMPILER_V3_CLAIM_NAME.as_bytes());
    hash.update(FORMAL_COMPILER_V3_WORD_BITS.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_BYTE_WIDTH.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_HELPER_PARAMETERS.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_BRANCH_ARMS.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_PRODUCTION_STACK_FRAMES.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_PRODUCTION_LOOP_TRIP_COUNT.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_GUARDED_ALLOCATIONS.to_le_bytes());
    for &(operation, tag) in FORMAL_COMPILER_V3_PRODUCTION_SCALAR_OPERATIONS {
        hash.update(operation.as_bytes());
        hash.update(tag.to_le_bytes());
    }
    hash.update(MIR_KIR_STRUCTURED_CFG_PROOF_SHA256_V3);
    hash.update(MIR_KIR_CFG_REFINEMENT_PROOF_SHA256_V2);
    hash.update(MIR_KIR_SCALAR_REFINEMENT_PROOF_SHA256_V1);
    hash.finalize().into()
}

fn evidence_identity_v3(
    model: [u8; 32],
    semantic: [u8; 32],
    kir: ProductionCanonicalKernelIrIdentityV1,
    fallback: u32,
    bindings: MirKirXorCfgBindingsV3,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(EVIDENCE_DOMAIN_V3);
    hash.update(model);
    hash.update(semantic);
    hash.update(kir.digest());
    hash.update(kir.canonical_length().to_le_bytes());
    hash.update(fallback.to_le_bytes());
    for value in bindings
        .semantic_functions
        .into_iter()
        .chain(bindings.semantic_root_values)
        .chain(bindings.semantic_root_load_sites.into_iter().flatten())
        .chain(bindings.semantic_helper_values)
        .chain(bindings.semantic_root_blocks)
        .chain(bindings.semantic_helper_blocks)
        .chain(bindings.kir_root_values.map(|value| value.0))
        .chain(bindings.kir_root_load_sites.into_iter().flatten())
        .chain(bindings.kir_root_call_site)
        .chain(bindings.kir_helper_values.map(|value| value.0))
        .chain(bindings.kir_root_blocks)
        .chain(bindings.kir_helper_blocks)
    {
        hash.update(value.to_le_bytes());
    }
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_relation() -> ExactXorRelationV3 {
        ExactXorRelationV3 {
            expected_callee: fe2o3_kernel_ir::FunctionId::new("helper"),
            actual_callee: fe2o3_kernel_ir::FunctionId::new("helper"),
            expected_call_arguments: [ValueId(10), ValueId(11)],
            actual_call_arguments: [ValueId(10), ValueId(11)],
            retained_load_results: [ValueId(10), ValueId(11)],
            expected_expression: ValueId(1),
            switch_selector: ValueId(1),
            zero_edge: ValueId(1),
            expected_fallback: ValueId(2),
            nonzero_edge: ValueId(2),
            join_parameter: ValueId(3),
            return_operand: ValueId(3),
            call_result: ValueId(4),
            retained_call_result: ValueId(4),
            semantic_expression: 5,
            retained_semantic_expression: 5,
            semantic_load_destinations: [20, 21],
            retained_semantic_load_destinations: [20, 21],
            semantic_call_destination: 6,
            retained_semantic_call_destination: 6,
        }
    }

    #[test]
    fn exact_relation_rejects_every_value_and_control_substitution() {
        validate_exact_relation_v3(exact_relation()).unwrap();
        let exact = exact_relation();
        let mutations = [
            ExactXorRelationV3 {
                actual_callee: fe2o3_kernel_ir::FunctionId::new("other"),
                ..exact.clone()
            },
            ExactXorRelationV3 {
                actual_call_arguments: [ValueId(11), ValueId(10)],
                ..exact.clone()
            },
            ExactXorRelationV3 {
                retained_load_results: [ValueId(10), ValueId(99)],
                ..exact.clone()
            },
            ExactXorRelationV3 {
                switch_selector: ValueId(9),
                ..exact.clone()
            },
            ExactXorRelationV3 {
                zero_edge: ValueId(9),
                ..exact.clone()
            },
            ExactXorRelationV3 {
                nonzero_edge: ValueId(9),
                ..exact.clone()
            },
            ExactXorRelationV3 {
                return_operand: ValueId(9),
                ..exact.clone()
            },
            ExactXorRelationV3 {
                retained_call_result: ValueId(9),
                ..exact.clone()
            },
            ExactXorRelationV3 {
                retained_semantic_expression: 9,
                ..exact.clone()
            },
            ExactXorRelationV3 {
                retained_semantic_load_destinations: [20, 99],
                ..exact.clone()
            },
            ExactXorRelationV3 {
                retained_semantic_call_destination: 9,
                ..exact
            },
        ];
        for mutation in mutations {
            assert_eq!(
                validate_exact_relation_v3(mutation),
                Err(MirKirXorCfgErrorV3::Correspondence)
            );
        }
    }
}
