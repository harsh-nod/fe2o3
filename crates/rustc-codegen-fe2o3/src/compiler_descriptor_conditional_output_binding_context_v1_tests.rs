//! Reuse private custody fixtures. These test receipt consumption, not rustc
//! authentication, a source/KIR relation, or production logical-body lowering.
use super::*;
use crate::compiler_descriptor::conditional_output_binding_v1::{
    CompilerConditionalOutputDescriptorErrorV1 as FieldError, GeneratedFieldCoordinatesV1,
    checked_generated_field_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

fn query(source: u32) -> GeneratedFieldCoordinatesV1 {
    GeneratedFieldCoordinatesV1 {
        source,
        adjusted: source,
        local: SemanticLocalIdV1::from_index(source + 1),
        ty: if source == 0 { CONTEXT } else { U32 },
    }
}

#[test]
fn materialization_source_only_borrows_existing_receipt_allocations() {
    let semantic = fixture(Mutation::None);
    let receipt = scope_tests::complete(&semantic);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 37);
    budget.reserve_storage(37).unwrap();
    let first = receipt
        .materialization_source_v29(&semantic, &mut budget)
        .unwrap()
        .unwrap();
    let second = receipt
        .materialization_source_v29(&semantic, &mut budget)
        .unwrap()
        .unwrap();
    assert!(std::ptr::eq(first.roots(), second.roots()));
    assert!(std::ptr::eq(first.classes(), second.classes()));
    assert!(std::ptr::eq(first.events(), second.events()));
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.peak_storage(), 37);
}

#[test]
fn authenticated_context_first_elides_only_context_with_multiple_arguments() {
    let semantic = fixture(Mutation::None);
    let receipt = scope_tests::complete(&semantic);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(37).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    for source in 1..=2 {
        assert_eq!(
            checked_generated_field_v1(
                &semantic,
                (
                    SemanticFunctionIdV1::from_index(0),
                    SemanticFunctionIdV1::from_index(1)
                ),
                Some(&receipt),
                query(source),
                2,
                &mut budget,
            )
            .unwrap(),
            (source - 1) as usize,
        );
        assert_eq!(budget.storage(), 37);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    // The physical root has no context input. Its coordinates must not acquire
    // the logical helper's shift just because a context receipt is available.
    for source in 0..2 {
        assert_eq!(
            checked_generated_field_v1(
                &semantic,
                (
                    SemanticFunctionIdV1::from_index(0),
                    SemanticFunctionIdV1::from_index(0)
                ),
                Some(&receipt),
                GeneratedFieldCoordinatesV1 {
                    source,
                    adjusted: source,
                    local: SemanticLocalIdV1::from_index(source + 1),
                    ty: U32,
                },
                2,
                &mut budget,
            )
            .unwrap(),
            source as usize,
        );
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn context_tag_without_original_complete_custody_cannot_admit_elision() {
    let semantic = fixture(Mutation::None);
    let incomplete = sealed_roots(&semantic);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    for receipt in [None, Some(&incomplete)] {
        assert!(
            checked_generated_field_v1(
                &semantic,
                (
                    SemanticFunctionIdV1::from_index(0),
                    SemanticFunctionIdV1::from_index(1)
                ),
                receipt,
                query(1),
                2,
                &mut budget,
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 0);
    }
    assert!(RetainedContextEntriesV29::seal(vec![], &semantic, |_| Ok(())).is_err());
}

#[test]
fn elided_context_and_cross_wired_source_coordinates_never_select_a_field() {
    let semantic = fixture(Mutation::None);
    let receipt = scope_tests::complete(&semantic);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    for case in 0..7 {
        let mut candidate = query(1);
        let mut count = 2;
        match case {
            0 => candidate = query(0),
            1 => candidate.source = 2,
            2 => candidate.adjusted = 0,
            3 => candidate.local = SemanticLocalIdV1::from_index(3),
            4 => candidate.ty = CONTEXT,
            5 => candidate.source = u32::MAX,
            6 => count = 3,
            _ => unreachable!(),
        }
        assert!(
            checked_generated_field_v1(
                &semantic,
                (
                    SemanticFunctionIdV1::from_index(0),
                    SemanticFunctionIdV1::from_index(1)
                ),
                Some(&receipt),
                candidate,
                count,
                &mut budget,
            )
            .is_err(),
            "coordinate {case}"
        );
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn reordered_parameters_and_substituted_nominal_source_require_new_admission() {
    let original = fixture(Mutation::None);
    let receipt = scope_tests::complete(&original);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    for mutation in [
        Mutation::Arguments,
        Mutation::HelperIdentity,
        Mutation::ContextIdentity,
        Mutation::Issuer,
    ] {
        let changed = fixture(mutation);
        assert!(matches!(
            checked_generated_field_v1(
                &changed,
                (
                    SemanticFunctionIdV1::from_index(0),
                    SemanticFunctionIdV1::from_index(1)
                ),
                Some(&receipt),
                query(1),
                2,
                &mut budget,
            ),
            Err(FieldError::ContextAdmission)
        ));
        assert_eq!(budget.storage(), 0);
    }
}

// Rebuild synthetic custody through the real bind/seal checks, including a fresh
// scope census. Never patch an already sealed receipt or its source digest.
fn matching_receipt(
    semantic: &AdmittedInertSemanticMirV1,
    candidate: CompletedContextEntryV29,
) -> RetainedContextEntriesV29 {
    let declarations = canonical_declaration_tables_commitment_v1(
        semantic.types(),
        semantic.callables(),
        SemanticMirWireVersionV1::V29,
        SemanticMirLimitsV1::default(),
        &mut |_| Ok(()),
    )
    .unwrap();
    let mut classes = vec![ScopeCallableV29::Ordinary; semantic.callables().len()];
    classes[candidate.helper.index() as usize] = ScopeCallableV29::Provider {
        function: candidate.helper,
        identity: candidate.helper_identity,
    };
    let entry = candidate
        .bind_function(&semantic.functions()[0], |_| Ok(()))
        .unwrap();
    let mut pending = PendingWorkgroupScopesV29::new(
        classes,
        declarations,
        semantic.target(),
        semantic.functions().len(),
    )
    .unwrap();
    for (index, function) in semantic.functions().iter().enumerate() {
        let id = SemanticFunctionIdV1::from_index(index as u32);
        let mut events = Vec::new();
        for (index, block) in function.blocks().iter().enumerate() {
            pending
                .capture(
                    id,
                    SemanticBlockIdV1::from_index(index as u32),
                    block.statements().len(),
                    block.terminator().kind(),
                    &mut events,
                    |_| Ok(()),
                )
                .unwrap();
        }
        pending.prepare(id, events, |_| Ok(())).unwrap().publish();
    }
    RetainedContextEntriesV29::seal_with_scopes(
        vec![entry],
        Some((pending, declarations)),
        semantic,
        |_| Ok(()),
    )
    .unwrap()
}

fn map_with_matching_receipt(
    semantic: &AdmittedInertSemanticMirV1,
    receipt: &RetainedContextEntriesV29,
) -> (Result<usize, FieldError>, usize) {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(37).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let source = receipt
        .materialization_source_v29(semantic, &mut budget)
        .unwrap()
        .unwrap();
    assert_eq!(source.roots().len(), 1);
    assert_eq!(budget.peak_storage(), 37);
    let before = budget.work();
    let result = checked_generated_field_v1(
        semantic,
        (
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ),
        Some(receipt),
        query(2),
        2,
        &mut budget,
    );
    assert_eq!(budget.storage(), 37);
    assert!(budget.work_ledger_identity_v1() == ledger);
    // Reaching both prepaid maps rules out stale custody and early ABI gates.
    // These are logical scratch charges, not whole-process memory accounting.
    assert_eq!(
        budget.peak_storage(),
        37 + 2 * std::mem::size_of::<fe2o3_mir_model::SemanticLogicalArgumentMapV1<'_>>()
            + 5 * std::mem::size_of::<Option<SemanticLocalIdV1>>()
    );
    (result, budget.work() - before)
}

#[test]
fn fresh_receipt_for_reordered_same_type_operands_reaches_forwarding_check() {
    let original = fixture(Mutation::None);
    let receipt = matching_receipt(&original, completed());
    let (result, success_work) = map_with_matching_receipt(&original, &receipt);
    assert_eq!(result.unwrap(), 1);

    let changed = fixture(Mutation::Arguments);
    assert!(receipt.validate_source(&changed).is_err());
    let mut candidate = completed();
    candidate.arguments.swap(1, 2);
    let receipt = matching_receipt(&changed, candidate);
    assert!(receipt.validate_source(&original).is_err());
    let (result, rejected_work) = map_with_matching_receipt(&changed, &receipt);
    assert!(matches!(result, Err(FieldError::ContextAdmission)));
    // Context was consumed, then the first scalar row failed forwarding; the
    // otherwise identical successful path charges one more 128-work row.
    assert_eq!(rejected_work + 128, success_work);
}

fn with_helper_argument(
    signature: &SemanticFunctionAbiV1,
    value: SemanticAbiValueV1,
) -> SemanticFunctionAbiV1 {
    let mut arguments = signature.arguments().to_vec();
    arguments[1] = SemanticAbiArgumentV1::source(value);
    SemanticFunctionAbiV1::from_rustc(
        signature.identity(),
        signature.layout_identity(),
        signature.canon_abi(),
        signature.extern_abi(),
        signature.can_unwind(),
        signature.c_variadic(),
        signature.fixed_count(),
        arguments,
        signature.return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(signature.source_argument_ownership().to_vec())
    .unwrap()
}

fn with_body(
    function: &SemanticFunctionDeclV1,
    signature: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let mut replacement = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        signature,
        locals,
        function.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = function.kernel_entry() {
        replacement = replacement.with_kernel_entry(entry.clone());
    }
    replacement
}

fn request_with_functions(
    original: &AdmittedInertSemanticMirV1,
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
) -> InertSemanticMirRequestV1 {
    InertSemanticMirRequestV1::new_with_callables(
        original.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        original.callables().to_vec(),
        original.roots().to_vec(),
    )
    .unwrap()
}

fn nonflat_helper_argument(tuple: bool) -> (AdmittedInertSemanticMirV1, CompletedContextEntryV29) {
    let original = fixture(Mutation::None);
    let mut types = original.types().to_vec();
    let (ty, mode, value) = if tuple {
        let ty = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([5; 32]),
            SemanticLayoutIdentityV1::from_sha256([5; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(4),
                4,
                types[U32.index() as usize].layout().backend_repr().clone(),
                false,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32]).unwrap()),
        ));
        (
            ty,
            original.functions()[1].abi().arguments()[1].mode().clone(),
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Tuple,
                vec![SemanticOperandV1::Copy(place(1, U32))],
            )
            .unwrap(),
        )
    } else {
        (
            MARKER,
            SemanticAbiPassModeV1::Ignore,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                MARKER,
                SemanticConstantValueV1::ZeroSized,
            ))),
        )
    };
    let mut candidate = completed();
    candidate.arguments[1] = SemanticOperandV1::Move(place(5, ty));
    candidate.helper_call.statements = 1;
    let mut functions = original.functions().to_vec();
    let root = &functions[0];
    let mut locals = root.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([6; 32]),
        ty,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let mut blocks = root.blocks().to_vec();
    blocks[1] = block(
        11,
        vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(5, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )],
        call(1, candidate.arguments.clone(), 4, UNIT, 2),
    );
    functions[0] = with_body(root, root.abi().clone(), locals, blocks);
    let helper = &functions[1];
    let mut locals = helper.locals().to_vec();
    locals[2] = SemanticLocalDeclV1::new(
        locals[2].identity(),
        ty,
        locals[2].role(),
        locals[2].source(),
    );
    functions[1] = with_body(
        helper,
        with_helper_argument(helper.abi(), SemanticAbiValueV1::new(ty, mode)),
        locals,
        helper.blocks().to_vec(),
    );
    let semantic = request_with_functions(&original, types, functions)
        .admit_exact_v29(SemanticMirLimitsV1::default())
        .unwrap();
    (semantic, candidate)
}

#[test]
fn matching_context_receipts_do_not_admit_tuple_or_noncontext_ignore_arguments() {
    for tuple in [true, false] {
        let (semantic, candidate) = nonflat_helper_argument(tuple);
        // The root stays flat; only the helper's first non-context row changes.
        assert_eq!(
            semantic.functions()[0].abi(),
            fixture(Mutation::None).functions()[0].abi()
        );
        let argument = &semantic.functions()[1].abi().arguments()[1];
        if tuple {
            assert!(matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_)));
            assert!(matches!(
                semantic.types()[argument.ty().index() as usize].shape(),
                SemanticTypeShapeV1::Tuple(_)
            ));
        } else {
            assert_eq!(argument.ty(), MARKER);
            assert!(matches!(argument.mode(), SemanticAbiPassModeV1::Ignore));
        }
        let receipt = matching_receipt(&semantic, candidate);
        let (result, _) = map_with_matching_receipt(&semantic, &receipt);
        assert!(
            matches!(result, Err(FieldError::UnsupportedAbi)),
            "tuple={tuple}"
        );
    }
}

#[test]
fn helper_adjustments_and_pointee_overrides_fail_before_context_admission() {
    let original = fixture(Mutation::None);
    let receipt = matching_receipt(&original, completed());
    assert_eq!(map_with_matching_receipt(&original, &receipt).0.unwrap(), 1);
    let helper = &original.functions()[1];
    let value = helper.abi().arguments()[1].value();
    let scalar = &original.types()[U32.index() as usize];
    let adjusted = SemanticAbiValueV1::new_with_adjusted_type(
        U32,
        SemanticAbiAdjustedTypeV1::new(U32, scalar.layout_identity(), scalar.layout().clone()),
        value.mode().clone(),
    );
    let overridden = value.clone().with_pointee_override(
        SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
    );
    for value in [adjusted, overridden] {
        let mut functions = original.functions().to_vec();
        functions[1] = with_body(
            helper,
            with_helper_argument(helper.abi(), value),
            helper.locals().to_vec(),
            helper.blocks().to_vec(),
        );
        // Defined helpers cannot admit these FnAbi facts. Do not bypass that
        // boundary to manufacture a retained receipt or reach the field mapper.
        assert!(matches!(
            request_with_functions(&original, original.types().to_vec(), functions)
                .admit_exact_v29(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

fn move_context_to(position: usize) -> AdmittedInertSemanticMirV1 {
    let original = fixture(Mutation::None);
    let mut functions = original.functions().to_vec();
    for (index, function) in functions.iter_mut().enumerate() {
        let mut signature = function.abi().clone();
        let mut locals = function.locals().to_vec();
        let mut blocks = function.blocks().to_vec();
        if index == 0 {
            let mut operands = completed().arguments;
            let context = operands.remove(0);
            operands.insert(position, context);
            blocks[1] = block(11, vec![], call(1, operands, 4, UNIT, 2));
        } else {
            let mut inputs = vec![U32, U32];
            inputs.insert(position, CONTEXT);
            signature = abi(50, false, &inputs, UNIT);
            // Preserve local identity and type; only entry argument roles move.
            let mut ordered_locals = vec![2usize, 3];
            ordered_locals.insert(position, 1);
            for (ordinal, local) in ordered_locals.into_iter().enumerate() {
                let old = &locals[local];
                locals[local] = SemanticLocalDeclV1::new(
                    old.identity(),
                    old.ty(),
                    SemanticLocalRoleV1::Argument(ordinal as u32),
                    old.source(),
                );
            }
        }
        let mut replacement = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            signature,
            locals,
            function.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = function.kernel_entry() {
            replacement = replacement.with_kernel_entry(entry.clone());
        }
        *function = replacement;
    }
    InertSemanticMirRequestV1::new_with_callables(
        original.target(),
        original.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        original.callables().to_vec(),
        original.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn nonzero_context_positions_require_importer_admission_not_index_compaction() {
    let original = fixture(Mutation::None);
    let receipt = scope_tests::complete(&original);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    for position in [1, 2] {
        let semantic = move_context_to(position);
        // Current compiler custody admits context only at source position zero.
        // Even a matching candidate operand roster cannot change that policy.
        let mut candidate = completed();
        let context = candidate.arguments.remove(0);
        candidate.arguments.insert(position, context);
        assert!(
            candidate
                .bind_function(&semantic.functions()[0], |_| Ok(()))
                .is_err()
        );
        for contexts in [None, Some(&receipt)] {
            assert!(
                checked_generated_field_v1(
                    &semantic,
                    (
                        SemanticFunctionIdV1::from_index(0),
                        SemanticFunctionIdV1::from_index(1)
                    ),
                    contexts,
                    GeneratedFieldCoordinatesV1 {
                        source: 0,
                        adjusted: 0,
                        local: SemanticLocalIdV1::from_index(2),
                        ty: U32,
                    },
                    2,
                    &mut budget,
                )
                .is_err()
            );
        }
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn context_mapping_work_is_cumulative_and_scratch_is_restored_on_denial() {
    let semantic = fixture(Mutation::None);
    let receipt = scope_tests::complete(&semantic);
    let run = |limit, storage| {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, storage);
        budget.reserve_storage(37).unwrap();
        budget.charge_work(11).unwrap();
        let result = checked_generated_field_v1(
            &semantic,
            (
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(1),
            ),
            Some(&receipt),
            query(2),
            2,
            &mut budget,
        );
        assert_eq!(budget.storage(), 37);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, exact, peak) = run(1_000_000, 1_000_000);
    assert_eq!(result.unwrap(), 1);
    assert_eq!(
        peak,
        37 + 2 * std::mem::size_of::<fe2o3_mir_model::SemanticLogicalArgumentMapV1<'_>>()
            + 5 * std::mem::size_of::<Option<SemanticLocalIdV1>>()
    );
    assert_eq!(run(exact, peak).0.unwrap(), 1);
    assert!(matches!(
        run(exact - 1, peak).0,
        Err(FieldError::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(exact, peak - 1).0,
        Err(FieldError::Resource(_))
    ));
}
