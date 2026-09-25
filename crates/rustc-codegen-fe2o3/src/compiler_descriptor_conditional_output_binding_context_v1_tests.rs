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
