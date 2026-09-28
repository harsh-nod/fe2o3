//! Independent frozen ORIGINAL constructor debit roster, test DATA only.
//! Never calls retained/lazy constructors, candidate state, or visit results.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) enum Debit {
    Work(usize),
    Storage(usize),
}
struct Trace {
    events: Vec<Debit>,
}
impl Trace {
    fn is_strict(&self) -> bool {
        true
    }
    fn extra_work(&mut self, n: usize) -> R<()> {
        self.events.push(Debit::Work(n));
        Ok(())
    }
    fn storage(&mut self, n: usize) {
        self.events.push(Debit::Storage(n));
    }
    fn reserve_frame<T>(&mut self, locals: usize) -> R<()> {
        // Exact original reserve_frame arithmetic, Self is the ORIGINAL handle.
        let bytes = 4096usize
            .checked_add(locals)
            .and_then(|n| n.checked_add(size_of::<AssertionResourcesV1<'static>>()))
            .and_then(|n| size_of::<T>().checked_mul(2).and_then(|x| n.checked_add(x)))
            .and_then(|n| {
                size_of::<R<T>>()
                    .checked_mul(2)
                    .and_then(|x| n.checked_add(x))
            })
            .ok_or_else(assertion_resource_overflow_v1)?;
        self.extra_work(16)?;
        self.storage(bytes);
        Ok(())
    }
    fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> R<()> {
        self.extra_work(8)?;
        let requested = values
            .len()
            .checked_add(additional)
            .ok_or_else(assertion_resource_overflow_v1)?;
        if requested <= values.capacity() {
            return Ok(());
        }
        self.extra_work(
            values
                .len()
                .checked_mul(size_of::<T>())
                .ok_or_else(assertion_resource_overflow_v1)?,
        )?;
        self.storage(
            requested
                .checked_mul(size_of::<T>())
                .ok_or_else(assertion_resource_overflow_v1)?,
        );
        values
            .try_reserve_exact(additional)
            .map_err(|_| assertion_resource_accounting_v1())?;
        if size_of::<T>() != 0 && values.capacity() != requested {
            return Err(assertion_resource_accounting_v1());
        }
        Ok(())
    }
    fn nested<T>(&mut self, count: usize) -> R<Vec<Vec<T>>> {
        let work = count
            .checked_mul(size_of::<Vec<T>>())
            .and_then(|n| n.checked_add(count))
            .ok_or_else(assertion_resource_overflow_v1)?;
        self.extra_work(work)?;
        let mut values = Vec::new();
        self.reserve(&mut values, count)?;
        values.resize_with(count, Vec::new);
        Ok(values)
    }
    fn push_vec<T>(&mut self, values: &mut Vec<T>, value: T, _old_error: &'static str) -> R<()> {
        self.reserve_frame::<T>(size_of::<Vec<T>>())?;
        self.extra_work(
            size_of::<T>()
                .checked_add(1)
                .ok_or_else(assertion_resource_overflow_v1)?,
        )?;
        self.reserve(values, 1)?;
        values.push(value);
        Ok(())
    }
}
pub(in crate::production_ranked_projection_v1) fn expected_debits(
    function: &SemanticFunctionDeclV1,
) -> R<Vec<Debit>> {
    let mut trace = Trace { events: Vec::new() };
    // Original strict constructor's work/storage before fixed/evaluator frames.
    let strict_bytes = 4096usize
        .checked_add(size_of::<AssertionResourcesV1<'static>>())
        .and_then(|n| {
            size_of::<R<AssertionResourcesV1<'static>>>()
                .checked_mul(2)
                .and_then(|r| n.checked_add(r))
        })
        .ok_or_else(assertion_resource_overflow_v1)?;
    trace.extra_work(8)?;
    trace.storage(strict_bytes);
    frozen_fixed_frames(&mut trace)?;
    trace.extra_work(1)?;
    frozen_evaluator_frames(&mut trace)?;
    trace.reserve_frame::<SemanticAssertProofsV1<'static>>(0)?;
    trace.extra_work(64)?;
    let _checked = frozen_checked_index(function, &mut trace)?;
    trace.reserve_frame::<AssertionCacheV1<'static>>(0)?;
    trace.reserve_frame::<AssertionCacheV1<'static>>(0)?;
    Ok(trace.events)
}

fn frozen_fixed_frames<'a>(resources: &mut Trace) -> Result<(), FixedGuardErrorV1> {
    use std::mem::size_of;
    // prepare_fixed_guard_session_v1: source-loan factory entry/transfer.
    resources.reserve_frame::<PreparedFixedGuardSessionV1<'a>>(size_of::<(
        &NominalRootCfgSourceV1<'_>,
        &mut PreparationResourcesV1<'_, '_>,
        FixedGuardInputsV1<'_, '_>,
    )>())?;
    // new_fixed_guard_session_v1: own arguments, strict handle, copied identity
    // and the borrowed proof constructor result coexist here.
    resources.reserve_frame::<PreparedFixedGuardSessionV1<'a>>(size_of::<(
        FixedGuardInputsV1<'_, '_>,
        &mut PreparationResourcesV1<'_, '_>,
        Option<FixedGuardLedgerV1>,
        AssertionResourcesV1<'_>,
        FixedGuardLedgerV1,
        Result<SemanticAssertProofsV1<'_>, FixedGuardErrorV1>,
    )>())?;
    // PreparedFixedGuardSession::authenticate.
    resources.reserve_frame::<FixedGuardDataV1>(size_of::<(
        &mut PreparedFixedGuardSessionV1<'_>,
        &NominalRootCfgSourceV1<'_>,
        usize,
        FixedGuardInputsV1<'_, '_>,
    )>())?;
    // query_inputs, including its retained result and closed continuation capture.
    resources.reserve_frame::<FixedGuardDataV1>(size_of::<(
        &mut PreparedFixedGuardSessionV1<'_>,
        FixedGuardInputsV1<'_, '_>,
        usize,
        Result<FixedGuardDataV1, FixedGuardErrorV1>,
        &mut SemanticAssertProofsV1<'_>,
        bool,
    )>())?;
    // require_inputs: every loan stays live through exact slice comparisons.
    resources.reserve_frame::<()>(size_of::<(
        &mut PreparedFixedGuardSessionV1<'_>,
        FixedGuardInputsV1<'_, '_>,
        &crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::RichNominalSourceTablesV1<'_>,
        FixedGuardLedgerV1,
        usize,
        bool,
    )>())?;
    // same_fixed_slice: widest borrowed slice representation is two words;
    // pointer/length scalars are explicit, never row payload copies.
    resources.reserve_frame::<bool>(size_of::<(
        &[Option<ScalarAssignmentSiteV1>],
        &[Option<ScalarAssignmentSiteV1>],
        *const Option<ScalarAssignmentSiteV1>,
        *const Option<ScalarAssignmentSiteV1>,
        usize,
        usize,
        bool,
    )>())?;
    // authenticate_selected_fixed_guard_v1: source Assert selector.
    resources.reserve_frame::<FixedGuardDataV1>(size_of::<(
        &mut SemanticAssertProofsV1<'_>,
        usize,
        &SemanticFunctionDeclV1,
        Option<&fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
        &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
        &SemanticTerminatorKindV1,
        &SemanticAssertMessageV1,
        &SemanticOperandV1,
        &bool,
        &SemanticOperandV1,
        &SemanticOperandV1,
        &fe2o3_mir_model::semantic_mir_v1::SemanticControlFlowEdgeV1,
        &SemanticUnwindActionV1,
        usize,
    )>())?;
    // authenticate_fixed_array_guard_v1: original live scalar/type/assignment
    // and exact-comparison state, plus all six input arguments.
    resources.reserve_frame::<FixedGuardDataV1>(size_of::<(
        &mut SemanticAssertProofsV1<'_>,
        usize,
        usize,
        &SemanticOperandV1,
        &SemanticOperandV1,
        &SemanticOperandV1,
        Option<SemanticLocalIdV1>,
        SemanticLocalIdV1,
        usize,
        Option<SemanticLocalIdV1>,
        SemanticLocalIdV1,
        usize,
        Option<u16>,
        u16,
        Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>,
        &fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1,
        SemanticTypeIdV1,
        SemanticTypeIdV1,
        Option<&bool>,
        Option<&bool>,
        Option<&u8>,
        Option<&SemanticTypeDeclV1>,
        Option<&SemanticTypeShapeV1>,
        &fe2o3_mir_model::semantic_mir_v1::SemanticConstantV1,
        &fe2o3_mir_model::semantic_mir_v1::SemanticScalarValueV1,
        u8,
        u16,
        u128,
        u128,
        u64,
        Result<u64, std::num::TryFromIntError>,
        Option<&Option<ScalarAssignmentSiteV1>>,
        Option<Option<ScalarAssignmentSiteV1>>,
        Option<ScalarAssignmentSiteV1>,
        ScalarAssignmentSiteV1,
        &SemanticStatementKindV1,
        &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        &SemanticRvalueKindV1,
        &SemanticOperandV1,
        &SemanticOperandV1,
        bool,
        Option<&u8>,
        Option<&Option<ScalarAssignmentSiteV1>>,
        Option<Option<ScalarAssignmentSiteV1>>,
        Option<ScalarAssignmentSiteV1>,
        ScalarAssignmentSiteV1,
        Result<bool, FixedGuardErrorV1>,
        Result<bool, FixedGuardErrorV1>,
    )>())?;
    // unsigned_index_bits_v1.
    resources.reserve_frame::<Option<u16>>(size_of::<(
        &[SemanticTypeDeclV1],
        SemanticTypeIdV1,
        usize,
        Option<&SemanticTypeDeclV1>,
        &SemanticTypeDeclV1,
        &SemanticTypeShapeV1,
        &u16,
        bool,
    )>())?;
    // fixed_guard_operand_scan_v1 paid precharge before original operand helper.
    resources.reserve_frame::<()>(size_of::<(
        &mut SemanticAssertProofsV1<'_>,
        &SemanticOperandV1,
        &SemanticPlaceV1,
        usize,
    )>())?;
    // simple_operand_local.
    resources.reserve_frame::<Option<SemanticLocalIdV1>>(size_of::<(
        &SemanticOperandV1,
        Option<&SemanticPlaceV1>,
        &SemanticPlaceV1,
        SemanticLocalIdV1,
        bool,
    )>())?;
    // raw_operand_place.
    resources.reserve_frame::<Option<&SemanticPlaceV1>>(size_of::<(
        &SemanticOperandV1,
        &SemanticPlaceV1,
    )>())?;
    // transparent_operand_place retains its own operand and raw return.
    resources.reserve_frame::<Option<&SemanticPlaceV1>>(size_of::<(
        &SemanticOperandV1,
        Option<&SemanticPlaceV1>,
        &SemanticPlaceV1,
    )>())?;
    // transparent_place's complete scan/predicate iterator state.
    resources.reserve_frame::<Option<&SemanticPlaceV1>>(size_of::<(
        &SemanticPlaceV1,
        &[fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1],
        std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1>,
        &fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1,
        SemanticProjectionKindV1,
        bool,
        Option<&SemanticPlaceV1>,
    )>())?;
    // fixed_source_refusal_v1 and fixed helper's closed refusal construction.
    resources.reserve_frame::<FixedGuardErrorV1>(size_of::<(&'static str, FixedGuardErrorV1)>())?;
    resources.reserve_frame::<FixedGuardErrorV1>(size_of::<(&'static str, FixedGuardErrorV1)>())?;
    // This nonrecursive roster and each reserve_frame Result transfer.
    resources.reserve_frame::<()>(size_of::<(
        &mut AssertionResourcesV1<'_>,
        usize,
        Result<(), FixedGuardErrorV1>,
    )>())?;
    Ok(())
}

fn frozen_evaluator_frames<'a>(
    resources: &mut Trace,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let locals = assertion_evaluator_local_frame_v1()?;
    // checked_assertion_index_v1
    resources
        .reserve_frame::<Result<Vec<Vec<usize>>, ProductionRankedProjectionErrorV1>>(locals)?;
    // new_borrowed_assertion_tables_v1
    resources
        .reserve_frame::<Result<SemanticAssertProofsV1<'a>, ProductionRankedProjectionErrorV1>>(
            locals,
        )?;
    // analyze_existing_assertions_v1
    resources.reserve_frame::<Result<Vec<bool>, ProductionRankedProjectionErrorV1>>(locals)?;
    // proves_literal_shift_assert_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // proves_checked_overflow_assert_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // authenticated_checked_binary_value_v1
    resources.reserve_frame::<Result<Option<AuthenticatedCheckedBinaryValueV1>, ProductionRankedProjectionErrorV1>>(locals)?;
    // authenticated_checked_binary_local_value_v1
    resources.reserve_frame::<Result<Option<AuthenticatedCheckedBinaryValueV1>, ProductionRankedProjectionErrorV1>>(locals)?;
    // charge
    resources.reserve_frame::<Result<(), ProductionRankedProjectionErrorV1>>(locals)?;
    // scalar_unsigned_maximum
    resources.reserve_frame::<Option<u128>>(locals)?;
    // unsigned_integer_bits
    resources.reserve_frame::<Option<u16>>(locals)?;
    // literal_unsigned_subtraction_upper_range_v1
    resources.reserve_frame::<Option<UnsignedRangeProofV1>>(locals)?;
    // assertion_range_operand_task_v1
    resources
        .reserve_frame::<Result<AssertionRangeOperandTaskV1, ProductionRankedProjectionErrorV1>>(
            locals,
        )?;
    // assertion_range_expression_task_v1
    resources
        .reserve_frame::<Result<AssertionRangeExpressionTaskV1, ProductionRankedProjectionErrorV1>>(
            locals,
        )?;
    // push_assertion_reserved_vec_v1
    resources.reserve_frame::<Result<(), ProductionRankedProjectionErrorV1>>(locals)?;
    // push_assertion_range_frame_with_resources_v1
    resources.reserve_frame::<Result<(), ProductionRankedProjectionErrorV1>>(locals)?;
    // push_assertion_range_value_with_resources_v1
    resources.reserve_frame::<Result<(), ProductionRankedProjectionErrorV1>>(locals)?;
    // pop_assertion_range_value_with_resources_v1
    resources
        .reserve_frame::<Result<Option<UnsignedRangeProofV1>, ProductionRankedProjectionErrorV1>>(
            locals,
        )?;
    // schedule_assertion_range_operand_v1
    resources.reserve_frame::<Result<(), ProductionRankedProjectionErrorV1>>(locals)?;
    // schedule_assertion_range_expression_v1
    resources.reserve_frame::<Result<(), ProductionRankedProjectionErrorV1>>(locals)?;
    // schedule_assertion_local_narrowing_v1
    resources.reserve_frame::<Result<(), ProductionRankedProjectionErrorV1>>(locals)?;
    // range_of_binary
    resources
        .reserve_frame::<Result<Option<UnsignedRangeProofV1>, ProductionRankedProjectionErrorV1>>(
            locals,
        )?;
    // authenticated_checked_binary_source_v1
    resources.reserve_frame::<Result<Option<AuthenticatedCheckedBinaryValueV1>, ProductionRankedProjectionErrorV1>>(locals)?;
    // exact_binary_source_v1
    resources.reserve_frame::<Result<
        Option<(ScalarAssignmentSiteV1, SemanticOperandV1, SemanticOperandV1)>,
        ProductionRankedProjectionErrorV1,
    >>(locals)?;
    // authenticated_scaled_quotient_remainder_v1
    resources.reserve_frame::<Result<Option<AuthenticatedScaledQuotientRemainderV1>, ProductionRankedProjectionErrorV1>>(locals)?;
    // proves_scaled_quotient_remainder_nonnegative_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // proves_authenticated_unsigned_lower_bound_subtraction_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // same_edge_stable_unsigned_subtraction_value_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // comparison_edge_authenticates_each_dynamic_use_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // same_exact_unsigned_value_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // exact_unsigned_value_origin_v1
    resources.reserve_frame::<Result<
        Option<(SemanticOperandV1, ScalarAssignmentSiteV1)>,
        ProductionRankedProjectionErrorV1,
    >>(locals)?;
    // explicit_checked_product_maximum_v1
    resources.reserve_frame::<Result<Option<u128>, ProductionRankedProjectionErrorV1>>(locals)?;
    // same_checked_product_operand_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // local_is_stable_from_operand_use_site_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // proves_strictly_bounded_unsigned_product_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // proves_strictly_bounded_unsigned_flat_index_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // proves_strictly_bounded_unsigned_nested_flat_index_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // proves_nested_offset_sum_below_product_extent_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // authenticated_strict_unsigned_bounds_v1
    resources.reserve_frame::<Result<
        Vec<(SemanticOperandV1, ScalarAssignmentSiteV1)>,
        ProductionRankedProjectionErrorV1,
    >>(locals)?;
    // same_edge_stable_unsigned_value_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // local_is_stable_between_sites_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // block_terminator_defines_local_v1
    resources.reserve_frame::<bool>(locals)?;
    // exact_same_type_use_alias_origin_v1
    resources.reserve_frame::<Result<
        Option<(SemanticOperandV1, ScalarAssignmentSiteV1)>,
        ProductionRankedProjectionErrorV1,
    >>(locals)?;
    // unsigned_operand_stable_from_edge_to_use_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // local_stable_after_comparison_and_edge_to_use_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // local_is_stable_between_edge_and_site_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // block_defines_local_in_statement_range_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // exact_checked_overflow_flag_source_local_v1
    resources.reserve_frame::<Result<Option<usize>, ProductionRankedProjectionErrorV1>>(locals)?;
    // operand_has_globally_stable_value_at_v1
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // assignment_dominates_use
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // exact_reaching_assignment_v1
    resources
        .reserve_frame::<Result<Option<ScalarAssignmentSiteV1>, ProductionRankedProjectionErrorV1>>(
            locals,
        )?;
    // zero_excluding_edge_dominates
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // blocks_reaching
    resources.reserve_frame::<Result<Vec<bool>, ProductionRankedProjectionErrorV1>>(locals)?;
    // local_is_stable_between_edge_and_use
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // local_is_stable_from_revalidating_edge_to_use
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // comparison_zero_excluding_target
    resources.reserve_frame::<Result<Option<usize>, ProductionRankedProjectionErrorV1>>(locals)?;
    // operand_is_exact_unsigned_zero
    resources.reserve_frame::<bool>(locals)?;
    // local_has_globally_stable_value_at
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // block_defines_local
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // local_is_value_preserving_alias_of
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // block_dominates
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    // edge_set_dominates
    resources.reserve_frame::<Result<bool, ProductionRankedProjectionErrorV1>>(locals)?;
    Ok(())
}

fn frozen_checked_index(
    function: &SemanticFunctionDeclV1,
    resources: &mut Trace,
) -> Result<Vec<Vec<usize>>, ProductionRankedProjectionErrorV1> {
    let mut checked_assertion_blocks = if resources.is_strict() {
        resources.nested(function.locals().len())?
    } else {
        vec![Vec::new(); function.locals().len()]
    };
    resources.extra_work(function.blocks().len())?;
    for (block_index, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Assert { condition, .. } = block.terminator().kind() else {
            continue;
        };
        let Some(local) = tuple_field_operand_local_v1(condition, 1) else {
            continue;
        };
        let Some(blocks) = checked_assertion_blocks.get_mut(local.index() as usize) else {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a checked arithmetic assertion is outside the semantic local table",
            ));
        };
        resources.push_vec(
            blocks,
            block_index,
            "checked arithmetic assertion storage cannot be reserved",
        )?;
    }
    Ok(checked_assertion_blocks)
}
