//! Synthetic provider labels test custody, not rustc authentication or execution.
use super::*;
use crate::collector::workgroup_scope_custody_v29::*;

fn declarations(semantic: &AdmittedInertSemanticMirV1) -> SemanticDeclarationTablesCommitmentV1 {
    canonical_declaration_tables_commitment_v1(
        semantic.types(),
        semantic.callables(),
        SemanticMirWireVersionV1::V29,
        SemanticMirLimitsV1::default(),
        &mut |_| Ok(()),
    )
    .unwrap()
}

fn classes(semantic: &AdmittedInertSemanticMirV1) -> Vec<ScopeCallableV29> {
    let mut classes = vec![ScopeCallableV29::Ordinary; semantic.callables().len()];
    let helper = semantic.roots().len();
    classes[helper] = ScopeCallableV29::Provider {
        function: SemanticFunctionIdV1::from_index(helper as u32),
        identity: semantic.functions()[helper].identity(),
    };
    classes
}

fn pending(semantic: &AdmittedInertSemanticMirV1) -> PendingWorkgroupScopesV29 {
    PendingWorkgroupScopesV29::new(
        classes(semantic),
        declarations(semantic),
        semantic.target(),
        semantic.functions().len(),
    )
    .unwrap()
}

fn capture(
    semantic: &AdmittedInertSemanticMirV1,
    change: impl FnMut(usize, &mut Vec<ScopeEventV29>),
) -> PendingWorkgroupScopesV29 {
    capture_with_pending(semantic, pending(semantic), change)
}

fn capture_with_pending(
    semantic: &AdmittedInertSemanticMirV1,
    mut pending: PendingWorkgroupScopesV29,
    mut change: impl FnMut(usize, &mut Vec<ScopeEventV29>),
) -> PendingWorkgroupScopesV29 {
    for (index, function) in semantic.functions().iter().enumerate() {
        let function_id = SemanticFunctionIdV1::from_index(index as u32);
        let mut events = Vec::new();
        for (block, data) in function.blocks().iter().enumerate() {
            pending
                .capture(
                    function_id,
                    SemanticBlockIdV1::from_index(block as u32),
                    data.statements().len(),
                    data.terminator().kind(),
                    &mut events,
                    |_| Ok(()),
                )
                .unwrap();
        }
        change(index, &mut events);
        pending
            .prepare(function_id, events, |_| Ok(()))
            .unwrap()
            .publish();
    }
    pending
}

fn entries(semantic: &AdmittedInertSemanticMirV1) -> Vec<RetainedContextEntryV29> {
    let roots = semantic.roots().len() as u32;
    (0..roots)
        .map(|root| {
            let mut entry = completed();
            entry.function = SemanticFunctionIdV1::from_index(root);
            entry.root_identity = semantic.functions()[root as usize].identity();
            entry.helper = SemanticFunctionIdV1::from_index(roots);
            entry.issuer = SemanticCallableIdV1::from_index(roots + 1);
            entry
                .bind_function(&semantic.functions()[root as usize], |_| Ok(()))
                .unwrap()
        })
        .collect()
}

fn seal(
    pending: PendingWorkgroupScopesV29,
    semantic: &AdmittedInertSemanticMirV1,
) -> Result<RetainedContextEntriesV29, Error> {
    RetainedContextEntriesV29::seal_with_scopes(
        entries(semantic),
        Some((pending, declarations(semantic))),
        semantic,
        |_| Ok(()),
    )
}

pub(super) fn complete(semantic: &AdmittedInertSemanticMirV1) -> RetainedContextEntriesV29 {
    seal(capture(semantic, |_, _| {}), semantic).unwrap()
}

pub(super) fn complete_ordinary(
    semantic: &AdmittedInertSemanticMirV1,
) -> RetainedContextEntriesV29 {
    let pending = PendingWorkgroupScopesV29::new(
        vec![ScopeCallableV29::Ordinary; semantic.callables().len()],
        declarations(semantic),
        semantic.target(),
        semantic.functions().len(),
    )
    .unwrap();
    seal(capture_with_pending(semantic, pending, |_, _| {}), semantic).unwrap()
}

fn ordinary_root(root: &SemanticFunctionDeclV1) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        root.locals()[..3].to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![block(
            10,
            vec![unit_return()],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone())
}

#[test]
fn materialization_source_keeps_ordinary_roots_without_context_callbacks() {
    use crate::production_pipeline::check_context_handoff_v29;
    let original = fixture(Mutation::None);
    let semantic = InertSemanticMirRequestV1::new(
        original.target(),
        original.types()[..2].to_vec(),
        vec![],
        vec![],
        vec![],
        vec![ordinary_root(&original.functions()[0])],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let receipt = RetainedContextEntriesV29::seal(vec![], &semantic, |_| Ok(())).unwrap();
    let launch = launch_roster(&semantic);
    let owner = ssa_owner(semantic);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(2);
    let mut budget = VisitBudget::new(&mut work, 7);
    budget.reserve_storage(7).unwrap();
    assert!(
        receipt
            .materialization_source_v29(owner.source_semantic(), &mut budget)
            .unwrap()
            .is_none()
    );
    check_context_handoff_v29(&receipt, &owner, &launch, &mut budget, |_, _| {
        panic!("ordinary root entered the context consumer")
    })
    .unwrap();
    assert_eq!(budget.work(), 2);
    assert_eq!(budget.storage(), 7);
    assert_eq!(launch.roots().len(), 1);
}

#[test]
fn materialization_source_keeps_context_subset_of_the_physical_root_roster() {
    use crate::production_pipeline::check_context_handoff_v29;
    let original = fixture_roots(Mutation::None, 4);
    let mut functions = original.functions().to_vec();
    for index in [0, 2] {
        functions[index] = ordinary_root(&functions[index]);
    }
    let semantic = InertSemanticMirRequestV1::new_with_callables(
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
    .unwrap();
    let retained = entries(&original)
        .into_iter()
        .filter(|entry| matches!(entry.function().index(), 1 | 3))
        .collect();
    let receipt = RetainedContextEntriesV29::seal_with_scopes(
        retained,
        Some((capture(&semantic, |_, _| {}), declarations(&semantic))),
        &semantic,
        |_| Ok(()),
    )
    .unwrap();
    let launch = launch_roster(&semantic);
    let owner = ssa_owner(semantic);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = VisitBudget::new(&mut work, projection_storage(&receipt) + 7);
    budget.reserve_storage(7).unwrap();
    let view = receipt
        .materialization_source_v29(owner.source_semantic(), &mut budget)
        .unwrap()
        .unwrap();
    assert_eq!(
        view.roots()
            .iter()
            .map(|root| root.function().index())
            .collect::<Vec<_>>(),
        [1, 3]
    );
    let mut observed = Vec::new();
    check_context_handoff_v29(&receipt, &owner, &launch, &mut budget, |root, _| {
        observed.push(root.root_id().index());
        assert_eq!(root.semantic_ssa().source_semantic().roots().len(), 4);
        Ok(())
    })
    .unwrap();
    assert_eq!(observed, [1, 3]);
    assert_eq!(launch.roots().len(), 4);
    assert_eq!(budget.storage(), 7);
}

#[test]
fn materialization_source_borrows_only_receipt_storage() {
    for count in [1, 4] {
        let semantic = fixture_roots(Mutation::None, count);
        let receipt = complete(&semantic);
        let owner = ssa_owner(semantic);
        let view = {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000);
            let mut budget = VisitBudget::new(&mut work, 7);
            budget.reserve_storage(7).unwrap();
            let view = receipt
                .materialization_source_v29(owner.source_semantic(), &mut budget)
                .unwrap()
                .unwrap();
            budget.charge_work(1).unwrap();
            assert_eq!(budget.storage(), 7);
            assert_eq!(
                budget.work(),
                2 + view.roots().len() + view.classes().len() + view.events().len()
            );
            view
        };
        drop(owner);
        let scopes = receipt.scopes.as_ref().unwrap();
        assert!(std::ptr::eq(
            view.semantic_sha256(),
            &receipt.semantic_sha256
        ));
        assert!(std::ptr::eq(view.roots(), receipt.entries.as_slice()));
        assert!(std::ptr::eq(view.classes(), scopes.classes()));
        assert!(std::ptr::eq(view.events(), scopes.events()));
        assert_eq!(view.roots().len(), usize::from(count));
        assert_eq!(view.events().len(), usize::from(count) + 1);
    }
}

#[test]
fn materialization_source_rejects_changed_source_and_incomplete_custody() {
    let semantic = fixture(Mutation::None);
    let receipt = complete(&semantic);
    for mutation in [
        Mutation::Arguments,
        Mutation::HelperIdentity,
        Mutation::ContextIdentity,
    ] {
        let changed = fixture(mutation);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = VisitBudget::new(&mut work, 7);
        budget.reserve_storage(7).unwrap();
        assert!(matches!(
            receipt.materialization_source_v29(&changed, &mut budget),
            Err(ContextRootVisitErrorV29::Source(_))
        ));
        assert_eq!(budget.work(), 1);
        assert_eq!(budget.storage(), 7);
    }
    let without_scopes = sealed_roots(&semantic);
    let mut without_roots = complete(&semantic);
    without_roots.entries.clear();
    for incomplete in [without_scopes, without_roots] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = VisitBudget::new(&mut work, 7);
        budget.reserve_storage(7).unwrap();
        assert!(matches!(
            incomplete.materialization_source_v29(&semantic, &mut budget),
            Err(ContextRootVisitErrorV29::Source(_))
        ));
        assert_eq!(budget.work(), 1);
        assert_eq!(budget.storage(), 7);
    }
}

#[test]
fn materialization_source_preserves_exact_shared_work_and_storage_boundaries() {
    let semantic = fixture_roots(Mutation::None, 4);
    let receipt = complete(&semantic);
    let scopes = receipt.scopes.as_ref().unwrap();
    let exact = 1 + receipt.entries.len() + scopes.classes().len() + scopes.events().len();
    for prior in [0, 11] {
        for remaining in [0, 1, exact - 1, exact] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(prior + remaining);
            let mut budget = VisitBudget::new(&mut work, 7);
            budget.charge_work(prior).unwrap();
            budget.reserve_storage(7).unwrap();
            let result = receipt.materialization_source_v29(&semantic, &mut budget);
            if remaining == exact {
                assert!(result.unwrap().is_some());
                assert_eq!(budget.work(), prior + exact);
            } else {
                assert!(matches!(result, Err(ContextRootVisitErrorV29::Resource(_))));
                assert!((prior..=prior + remaining).contains(&budget.work()));
            }
            assert_eq!(budget.storage(), 7);
        }
    }
}

#[test]
fn context_handoff_rejects_incomplete_custody_before_its_consumer() {
    use crate::production_pipeline::{ProductionPipelineError, check_context_handoff_v29};
    let semantic = fixture(Mutation::None);
    let incomplete = sealed_roots(&semantic);
    let launch = launch_roster(&semantic);
    let owner = ssa_owner(semantic);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = VisitBudget::new(&mut work, 7);
    budget.reserve_storage(7).unwrap();
    let result = check_context_handoff_v29(&incomplete, &owner, &launch, &mut budget, |_, _| {
        panic!("incomplete scope custody reached the context consumer")
    });
    assert!(matches!(
        result,
        Err(ProductionPipelineError::SemanticImport(
            crate::collector::ProductionSemanticImportErrorV1::BodyConstruction(_)
        ))
    ));
    assert_eq!(budget.work(), 1);
    assert_eq!(budget.storage(), 7);
}

#[test]
fn scope_capture_seals_complete_canonical_events_for_shared_provider() {
    for count in [1, 4] {
        let semantic = fixture_roots(Mutation::None, count);
        let sealed = seal(capture(&semantic, |_, _| {}), &semantic).unwrap();
        let scopes = sealed.scopes.as_ref().unwrap();
        assert_eq!(scopes.classes(), classes(&semantic));
        assert_eq!(scopes.events().len(), usize::from(count) + 1);
        for (root, event) in scopes.events()[..usize::from(count)].iter().enumerate() {
            assert_eq!(event.function.index(), root as u32);
            assert_eq!(event.block.index(), 1);
            assert_eq!(event.statement_count, 0);
            assert_eq!(
                event.kind,
                ScopeEventKindV29::Call {
                    callee: SemanticCallableIdV1::from_index(u32::from(count)),
                    kind: ScopeCallKindV29::Provider,
                }
            );
        }
        let exit = scopes.events().last().unwrap();
        assert_eq!(exit.function.index(), u32::from(count));
        assert_eq!(exit.kind, ScopeEventKindV29::Return);
        assert_eq!(exit.statement_count, 1);
    }
}

#[test]
fn scope_seal_rejects_missing_extra_and_changed_events() {
    let semantic = fixture(Mutation::None);
    for mutation in 0..5 {
        let pending = capture(&semantic, |index, events| {
            if index != 0 {
                return;
            }
            match mutation {
                0 => events.clear(),
                1 => events.push(events[0]),
                2 => events[0].block = SemanticBlockIdV1::from_index(2),
                3 => events[0].statement_count += 1,
                4 => events[0].kind = ScopeEventKindV29::Return,
                _ => unreachable!(),
            }
        });
        assert!(seal(pending, &semantic).is_err(), "mutation {mutation}");
    }
    assert!(seal(pending(&semantic), &semantic).is_err());
}

#[test]
fn scope_seal_rejects_fresh_admitted_declaration_and_provider_substitutions() {
    let original = fixture(Mutation::None);
    for mutation in [
        Mutation::LayoutIdentity,
        Mutation::ContextIdentity,
        Mutation::Issuer,
        Mutation::HelperIdentity,
    ] {
        let changed = fixture(mutation);
        assert!(seal(capture(&original, |_, _| {}), &changed).is_err());
    }
}

fn readmit(
    original: &AdmittedInertSemanticMirV1,
    target: SemanticTargetDataLayoutV1,
    allocations: Vec<SemanticAllocationDeclV1>,
    callables: Vec<SemanticCallableDeclV1>,
) -> AdmittedInertSemanticMirV1 {
    InertSemanticMirRequestV1::new_with_callables(
        target,
        original.types().to_vec(),
        allocations,
        vec![],
        vec![],
        original.functions().to_vec(),
        callables,
        original.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn scope_seal_binds_target_after_fresh_admission() {
    let original = fixture(Mutation::None);
    let changed = readmit(
        &original,
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([249; 32])),
        vec![],
        original.callables().to_vec(),
    );
    assert_eq!(declarations(&original), declarations(&changed));
    assert!(seal(capture(&original, |_, _| {}), &changed).is_err());
}

#[test]
fn scope_seal_rejects_fresh_admitted_reachable_external_tables() {
    let original = fixture(Mutation::None);
    let allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256([1; 32]),
        vec![0; 4],
        vec![0x0f],
        4,
        false,
        vec![],
    )
    .unwrap();
    let exported = SemanticStaticDeclV1::new(
        SemanticStaticIdentityV1::from_sha256([1; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        U32,
        false,
        0,
        SemanticStaticDefinitionV1::Defined {
            initializer: SemanticAllocationIdV1::from_index(0),
        },
    )
    .with_export_symbol(SemanticLinkSymbolV1::new(b"scope_custody_external".to_vec()).unwrap());
    let changed = InertSemanticMirRequestV1::new_with_callables(
        original.target(),
        original.types().to_vec(),
        vec![allocation],
        vec![exported],
        vec![],
        original.functions().to_vec(),
        original.callables().to_vec(),
        original.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(declarations(&original), declarations(&changed));
    assert_eq!(original.functions(), changed.functions());
    RetainedContextEntriesV29::seal(entries(&original), &changed, |_| Ok(())).unwrap();
    assert!(seal(capture(&original, |_, _| {}), &changed).is_err());
}

#[test]
fn scope_seal_binds_non_primary_callable_metadata_after_fresh_admission() {
    let original = fixture(Mutation::None);
    for axis in 0..4 {
        let mut callables = original.callables().to_vec();
        let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &mut callables[2] else {
            panic!()
        };
        let abi = binding.abi().clone();
        *binding = SemanticNonBodyCallableBindingV1::new(
            binding.identity(),
            SemanticItemDefinitionIdentityV1::from_sha256([if axis == 0 { 90 } else { 70 }; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([if axis == 1 { 90 } else { 70 }; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(
                [if axis == 2 { 90 } else { 70 }; 32],
            ),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(
                [if axis == 3 { 90 } else { 70 }; 32],
            ),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        );
        let changed = readmit(&original, original.target(), vec![], callables);
        // The old primary issuer/entry checks alone accept this substitution.
        RetainedContextEntriesV29::seal(entries(&original), &changed, |_| Ok(())).unwrap();
        assert!(
            seal(capture(&original, |_, _| {}), &changed).is_err(),
            "axis {axis}"
        );
    }
}

#[test]
fn scope_capture_distinguishes_returns_abnormal_exits_and_destinationless_calls() {
    let semantic = fixture(Mutation::None);
    let mut pending = pending(&semantic);
    pending
        .prepare(SemanticFunctionIdV1::from_index(0), vec![], |_| Ok(()))
        .unwrap()
        .publish();
    let cases = [
        (SemanticTerminatorKindV1::Return, ScopeEventKindV29::Return),
        (
            SemanticTerminatorKindV1::Unreachable,
            ScopeEventKindV29::Unreachable,
        ),
        (
            SemanticTerminatorKindV1::UnwindResume,
            ScopeEventKindV29::UnwindResume,
        ),
        (
            SemanticTerminatorKindV1::UnwindTerminate,
            ScopeEventKindV29::UnwindTerminate,
        ),
        (SemanticTerminatorKindV1::Abort, ScopeEventKindV29::Abort),
        (
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(2),
                    vec![],
                    None,
                    SemanticUnwindActionV1::Terminate,
                )
                .unwrap(),
            ),
            ScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(2),
                kind: ScopeCallKindV29::Ordinary,
            },
        ),
    ];
    let mut events = Vec::new();
    for (block, (kind, expected)) in cases.iter().enumerate() {
        pending
            .capture(
                SemanticFunctionIdV1::from_index(1),
                SemanticBlockIdV1::from_index(block as u32),
                0,
                kind,
                &mut events,
                |_| Ok(()),
            )
            .unwrap();
        assert_eq!(events.last().unwrap().kind, *expected);
    }
    let last = events.clone();
    assert!(
        pending
            .capture(
                SemanticFunctionIdV1::from_index(1),
                SemanticBlockIdV1::from_index(0),
                0,
                &SemanticTerminatorKindV1::Return,
                &mut events,
                |_| Ok(())
            )
            .is_err()
    );
    assert_eq!(events, last);
    let tail = SemanticTerminatorKindV1::TailCall(
        SemanticDirectTailCallV1::new_callable(
            SemanticCallableIdV1::from_index(2),
            vec![],
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    assert!(
        pending
            .capture(
                SemanticFunctionIdV1::from_index(1),
                SemanticBlockIdV1::from_index(9),
                0,
                &tail,
                &mut events,
                |_| Ok(())
            )
            .is_err()
    );
}

#[test]
fn scope_capture_rejects_derive_outside_provider_and_accepts_inside() {
    let semantic = fixture(Mutation::None);
    let mut classes = classes(&semantic);
    classes[2] = ScopeCallableV29::Derive {
        binding: SemanticFunctionIdentityV1::from_sha256([70; 32]),
        operation: SemanticCompilerIntrinsicIdentityV1::from_sha256([70; 32]),
        context: CONTEXT,
        workgroup: MARKER,
    };
    // Synthetic classifier test only, deliberately not a structurally admitted derive.
    let mut pending =
        PendingWorkgroupScopesV29::new(classes, declarations(&semantic), semantic.target(), 2)
            .unwrap();
    let kind = call(2, vec![], 0, UNIT, 1);
    let mut events = Vec::new();
    assert!(
        pending
            .capture(
                SemanticFunctionIdV1::from_index(0),
                SemanticBlockIdV1::from_index(0),
                0,
                &kind,
                &mut events,
                |_| Ok(())
            )
            .is_err()
    );
    assert!(events.is_empty());
    pending
        .prepare(SemanticFunctionIdV1::from_index(0), vec![], |_| Ok(()))
        .unwrap()
        .publish();
    pending
        .capture(
            SemanticFunctionIdV1::from_index(1),
            SemanticBlockIdV1::from_index(0),
            0,
            &kind,
            &mut events,
            |_| Ok(()),
        )
        .unwrap();
    assert_eq!(
        events[0].kind,
        ScopeEventKindV29::Call {
            callee: SemanticCallableIdV1::from_index(2),
            kind: ScopeCallKindV29::Derive,
        }
    );
}

#[test]
fn scope_prepare_drop_or_charge_failure_does_not_publish() {
    let semantic = fixture(Mutation::None);
    let event = ScopeEventV29 {
        function: SemanticFunctionIdV1::from_index(0),
        block: SemanticBlockIdV1::from_index(1),
        statement_count: 0,
        kind: ScopeEventKindV29::Call {
            callee: SemanticCallableIdV1::from_index(1),
            kind: ScopeCallKindV29::Provider,
        },
    };
    for fail in 0..=2 {
        let mut pending = pending(&semantic);
        let mut charged = 0;
        let mut calls = 0;
        let prepared = pending.prepare(event.function, vec![event], |amount| {
            charged += amount;
            calls += 1;
            if fail == calls {
                Err(Error::IdentityTableMismatch {
                    table: "injected scope charge",
                })
            } else {
                Ok(())
            }
        });
        assert_eq!(prepared.is_ok(), fail == 0);
        drop(prepared);
        assert!(charged > 0);
        // The same function is still next, including when a prepared batch was dropped.
        pending
            .prepare(event.function, vec![event], |_| Ok(()))
            .unwrap()
            .publish();
        let function = SemanticFunctionIdV1::from_index(1);
        let body = &semantic.functions()[1].blocks()[0];
        let mut events = Vec::new();
        pending
            .capture(
                function,
                SemanticBlockIdV1::from_index(0),
                body.statements().len(),
                body.terminator().kind(),
                &mut events,
                |_| Ok(()),
            )
            .unwrap();
        pending
            .prepare(function, events, |_| Ok(()))
            .unwrap()
            .publish();
        assert_eq!(
            seal(pending, &semantic)
                .unwrap()
                .scopes
                .unwrap()
                .events()
                .len(),
            2
        );
    }
}

#[test]
fn scope_census_seal_charges_same_cumulative_ledger_without_refund() {
    let semantic = fixture(Mutation::None);
    let mut required = 7usize;
    RetainedContextEntriesV29::seal_with_scopes(
        entries(&semantic),
        Some((capture(&semantic, |_, _| {}), declarations(&semantic))),
        &semantic,
        |amount| {
            required += amount;
            Ok(())
        },
    )
    .unwrap();
    for max in [0, required - 1, required] {
        let mut used = 7usize;
        let result = RetainedContextEntriesV29::seal_with_scopes(
            entries(&semantic),
            Some((capture(&semantic, |_, _| {}), declarations(&semantic))),
            &semantic,
            |amount| {
                used += amount;
                if used > max {
                    Err(Error::IdentityTableMismatch {
                        table: "scope seal work limit",
                    })
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.is_ok(), max == required);
        assert!(used > 7);
        assert!(used <= required);
    }
}
