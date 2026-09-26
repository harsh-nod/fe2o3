thread_local! {
    static PENDING_READ_MODE_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static PENDING_READ_JOINED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PENDING_READ_FINAL_GATE_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PENDING_READ_MUTATED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PENDING_READ_CONTROL_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

const PENDING_READ_STOP_V29: &str = "pending pointer read claim control completed";

fn pending_read_error_v29(error: ProductionSemanticKirErrorV1, expected: &'static str) {
    assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported {
        function: 0, block: None, statement: None, detail,
    } if detail == expected), "{error:?}");
}

fn observe_pending_read_final_gate_v29(
    _: &mut PendingScopedRootEmissionV29,
    _: &ExecutionInstancesV29<'_>,
    _: &SourceReferencePlanV29<'_, '_>,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert!(PENDING_READ_JOINED_V29.get());
    PENDING_READ_FINAL_GATE_V29.set(true);
    Ok(())
}

fn observe_pending_read_claims_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let references = references.unwrap();
    let floor = budget.storage();
    let mode = PENDING_READ_MODE_V29.get();
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        check_pending_source_reference_claims_v29(references, emitted, slots, budget)?;
        PENDING_READ_JOINED_V29.set(true);
        if mode < 2 { return Ok(()); }

        // This is an inert test copy of query state after the genuine complete
        // pending join. No altered index or seed is returned as checked state.
        let mut proof = source_reference_backing_pointer_claims_v29(
            references, emitted, slots, SourceReferencePointerClaimsV29::SelectedBacking, budget,
        )?;
        let root = instances.root();
        let lowered = emitted[root.index()].as_ref().unwrap();
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        let (anchor, row, payload) = anchors.rows.iter().enumerate().find_map(|(i, row)| {
            let ScopedMemoryAnchorKindV29::Object(object) = row.kind else { return None; };
            let payload = anchors.objects[object];
            let ScopedObjectRoleV29::ReadValue { source, read: ScopedObjectReadOriginV29::Original(_) } = payload.role else { return None; };
            if !matches!(source.object, ScopedObjectIdentityV29::Local { local, .. } if local.index() == 3) { return None; }
            let key = (root.index(), payload.result?);
            let definition = proof.index.definitions.binary_search_by_key(&key, |row| row.0).ok()?;
            let (Type::Pointer(_), _) = proof.index.definitions[definition].1 else { return None; };
            Some((i, row, payload))
        }).expect("a genuine retained pointer-holder read");
        let key = (root.index(), payload.result.unwrap());
        let original = instances.instance(root).unwrap().declaration();
        let occurrences = instances.occurrences(root).unwrap();
        match mode {
            2 | 3 => {
                let mut changed = payload;
                let ScopedObjectRoleV29::ReadValue { read: ScopedObjectReadOriginV29::Original(ref mut read), .. } = changed.role else { unreachable!() };
                if mode == 2 { read.role = ExecutionOperandV29::Destination; }
                else { read.ty = SemanticTypeIdV1::from_index(1); }
                pending_read_error_v29(anchors.check_object_source(original, &occurrences,
                    anchor, row, &changed, budget).unwrap_err(),
                    "typed object source payload differs from its actual operation");
            }
            4 => {
                // Duplicate the already-authenticated pointer-result seed. The
                // producer must not overwrite a prior claim, even an equal one.
                let error = source_reference_pending_read_seeds_v29(references.plan, emitted,
                    &proof.index, &proof.ordinals, &proof.cell_slots, &mut proof.origins, budget).unwrap_err();
                pending_read_error_v29(error, "typed object source payload differs from its actual operation");
            }
            5 | 8 => {
                let replacement = proof.index.definitions.iter().find_map(|&(other, definition)| {
                    (if mode == 5 {
                        other.0 == key.0 && other != key && matches!(definition.1,
                            SourceReferencePointerDefinitionV29::Operation(_, 0))
                    } else { other.0 != key.0 }).then_some(definition)
                }).unwrap();
                let definition = proof.index.definitions.binary_search_by_key(&key, |row| row.0).unwrap();
                proof.index.definitions[definition].1 = replacement;
                let mut seeds = source_reference_owned_vec_v29(references.plan, proof.origins.len(), budget)?;
                budget.charge_work(proof.origins.len())?;
                seeds.resize(proof.origins.len(), origin_worklist_v1::OriginStateV1::Unknown);
                let error = source_reference_pending_read_seeds_v29(references.plan, emitted,
                    &proof.index, &proof.ordinals, &proof.cell_slots, &mut seeds, budget).unwrap_err();
                pending_read_error_v29(error, "typed object source payload differs from its actual operation");
            }
            6 => {
                let definition = proof.index.definitions.binary_search_by_key(&key, |row| row.0).unwrap();
                proof.index.definitions[definition].1.0 = &Type::INDEX;
                let mut seeds = source_reference_owned_vec_v29(references.plan, proof.origins.len(), budget)?;
                budget.charge_work(proof.origins.len())?;
                seeds.resize(proof.origins.len(), origin_worklist_v1::OriginStateV1::Unknown);
                let error = source_reference_pending_read_seeds_v29(references.plan, emitted,
                    &proof.index, &proof.ordinals, &proof.cell_slots, &mut seeds, budget).unwrap_err();
                pending_read_error_v29(error, "typed object source payload differs from its actual operation");
            }
            7 => {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
                let owner_before = (budget.work(), budget.storage());
                let foreign_before = (foreign.work(), foreign.storage());
                let error = source_reference_pending_read_seeds_v29(references.plan, emitted,
                    &proof.index, &proof.ordinals, &proof.cell_slots, &mut proof.origins, &mut foreign).unwrap_err();
                assert!(matches!(error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting)), "{error:?}");
                assert_eq!((budget.work(), budget.storage()), owner_before);
                assert_eq!((foreign.work(), foreign.storage()), foreign_before);
                assert!(matches!(references.check(budget), Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))));
                assert_eq!((budget.work(), budget.storage()), owner_before);
            }
            _ => unreachable!(),
        }
        PENDING_READ_CONTROL_V29.set(true);
        Err(source_reference_error_v29(PENDING_READ_STOP_V29))
    });
    assert_eq!(budget.storage(), floor);
    result
}

struct PendingReadObserversV29(
    Option<ScopedSlotCustodyObserverV29>,
    Option<ScopedSlotObserverV29>,
    Option<RootExecutionArchiveObserverV29>,
);
impl PendingReadObserversV29 {
    fn install(mutate: Option<ScopedSlotObserverV29>) -> Self {
        Self(SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(observe_pending_read_claims_v29)),
            SCOPED_SLOT_OBSERVER_V29.replace(mutate),
            ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.replace(Some(observe_pending_read_final_gate_v29)))
    }
}
impl Drop for PendingReadObserversV29 {
    fn drop(&mut self) {
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
        SCOPED_SLOT_OBSERVER_V29.set(self.1);
        ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(self.2);
    }
}

fn pending_read_capture_v29(factory: fn() -> ProductionSemanticSsaOwnerV1) -> SourceOwnedResultV18<()> {
    PENDING_READ_JOINED_V29.set(false);
    PENDING_READ_FINAL_GATE_V29.set(false);
    PENDING_READ_MUTATED_V29.set(false);
    PENDING_READ_CONTROL_V29.set(false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
    let mut completed = false;
    let result = prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
        assert_eq!(PENDING_READ_MODE_V29.get(), 0, "hostile hypothesis cannot grant final admission");
        assert!(PENDING_READ_JOINED_V29.get());
        assert!(PENDING_READ_FINAL_GATE_V29.get());
        completed = true;
        Ok(())
    });
    assert_eq!(completed, result.is_ok(), "{result:?}");
    assert_eq!(budget.storage(), MODULE_FLOOR);
    result
}

#[test]
fn pending_pointer_read_claims_keep_repeated_diamond_loop_source_positives() {
    let _restore = PendingReadObserversV29::install(None);
    for immutable in [false, true] {
        for graph in 0..3 {
            SELECTED_POINTER_FIXTURE.set((immutable, graph));
            PENDING_READ_MODE_V29.set(0);
            pending_read_capture_v29(selected_pointer_emission_owner_v29)
                .unwrap_or_else(|error| panic!("immutable={immutable}, graph={graph}: {error:?}"));
        }
    }
}

#[test]
fn pending_pointer_read_claims_reject_source_result_type_duplicate_and_foreign_ledger() {
    let _restore = PendingReadObserversV29::install(None);
    for immutable in [false, true] {
        SELECTED_POINTER_FIXTURE.set((immutable, 0));
        for mode in 2..9 {
            PENDING_READ_MODE_V29.set(mode);
            let error = pending_read_capture_v29(selected_pointer_emission_owner_v29).unwrap_err();
            assert!(PENDING_READ_JOINED_V29.get(), "mode={mode}: {error:?}");
            assert!(PENDING_READ_CONTROL_V29.get(), "mode={mode}: {error:?}");
            assert!(!PENDING_READ_FINAL_GATE_V29.get());
            if mode == 7 {
                assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting)))), "{error:?}");
            } else {
                assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported {
                        detail: PENDING_READ_STOP_V29, .. }))), "{error:?}");
            }
        }
    }
}

fn pending_read_distinct_holder_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let (immutable, _) = SELECTED_POINTER_FIXTURE.get();
    let base = selected_pointer_test_owner_v29(entrance_control_owner(false), immutable, 0);
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let span = original.source();
    let word = SemanticTypeIdV1::from_index(1);
    let raw = SemanticTypeIdV1::from_index(2);
    let indirect = SemanticTypeIdV1::from_index(3);
    let plain = |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |local, ty, value| SemanticStatementV1::new(span, SemanticStatementKindV1::Assign(
        SemanticAssignmentV1::new(plain(local, ty), SemanticRvalueV1::new(ty, value))));
    let copy = |local, ty| SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(plain(local, ty)));
    let mut locals = original.locals().to_vec();
    for (tag, ty) in [(239, raw), (240, indirect), (241, word), (242, raw)] {
        locals.push(SemanticLocalDeclV1::new(SemanticLocalIdentityV1::from_sha256([tag; 32]), ty,
            SemanticLocalRoleV1::Temporary, span));
    }
    let mut statements = original.blocks()[0].statements()[..2].to_vec();
    statements.extend([
        assign(11, word, copy(1, word)),
        assign(12, raw, SemanticRvalueKindV1::AddressOf { place: plain(11, word),
            mutability: if immutable { SemanticMutabilityV1::Immutable } else { SemanticMutabilityV1::Mutable } }),
        assign(9, raw, copy(12, raw)),
        assign(10, indirect, SemanticRvalueKindV1::AddressOf { place: plain(9, raw),
            mutability: SemanticMutabilityV1::Mutable }),
    ]);
    statements.extend_from_slice(&original.blocks()[0].statements()[2..]);
    let mut blocks = original.blocks().to_vec();
    blocks[0] = SemanticBasicBlockV1::new(blocks[0].identity(), blocks[0].source(), statements,
        blocks[0].terminator().clone()).unwrap();
    let root = SemanticFunctionDeclV1::new(original.identity(), original.role(), original.item_definition_identity(),
        original.monomorphization_identity(), original.generic_type_arguments_identity(), original.const_generic_arguments_identity(),
        span, original.abi().clone(), locals, original.entry(), blocks).unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(source.target(), source.types().to_vec(),
        vec![], vec![], vec![], vec![root, source.functions()[1].clone()],
        vec![SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1))],
        vec![SemanticFunctionIdV1::from_index(0)]).unwrap()
        .admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(ProductionSemanticMirOwnerV1::try_new(admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default()).unwrap(), ProductionSemanticSsaLimitsV1::default()).unwrap()
}

fn substitute_pending_read_holder_v29(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if PENDING_READ_MODE_V29.get() == 0 { return Ok(()); }
    let root = instances.root();
    let slot = |local| slots.slots.iter().find(|slot| slot.instance == root &&
        matches!(slot.origin.identity, ScopedAllocationIdentityV29::OriginalObject { local: found, generation: 0 } if found == local)).unwrap();
    let (original, wrong) = (slot(3), slot(9));
    assert_eq!(original.representation, wrong.representation);
    assert_ne!(original.origin.pointer, wrong.origin.pointer);
    let lowered = emitted[root.index()].as_mut().unwrap();
    let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
    let (object, block, position) = anchors.rows.iter().find_map(|row| {
        let ScopedMemoryAnchorKindV29::Object(object) = row.kind else { return None; };
        let payload = anchors.objects[object];
        let ScopedObjectRoleV29::ReadValue { source, read: ScopedObjectReadOriginV29::Original(_) } = payload.role else { return None; };
        (matches!(source.object, ScopedObjectIdentityV29::Local { local, .. } if local.index() == 3)
            && matches!(payload.operation, ScopedObjectOperationV29::ReadValue { address, .. } if address == original.origin.pointer))
            .then_some((object, row.block, row.position))
    }).expect("actual whole pointer holder read");
    let operation = &mut lowered.function.body.as_mut().unwrap().blocks.iter_mut()
        .find(|candidate| candidate.id == block).unwrap().operations[position];
    let OperationKind::Storage(ScopedObjectOperationV29::ReadValue { ref mut address, .. }) = operation.kind else { unreachable!() };
    assert_eq!(*address, original.origin.pointer);
    *address = wrong.origin.pointer;
    let ScopedObjectOperationV29::ReadValue { ref mut address, .. } = anchors.objects[object].operation else { unreachable!() };
    *address = wrong.origin.pointer;
    anchors.objects[object].check_operation(operation, budget)?;
    PENDING_READ_MUTATED_V29.set(true);
    Ok(())
}

#[test]
fn pending_pointer_read_hypothesis_cannot_replace_final_original_memory_check() {
    let _restore = PendingReadObserversV29::install(Some(substitute_pending_read_holder_v29));
    for immutable in [false, true] {
        SELECTED_POINTER_FIXTURE.set((immutable, 0));
        PENDING_READ_MODE_V29.set(0);
        pending_read_capture_v29(pending_read_distinct_holder_owner_v29).unwrap();
        PENDING_READ_MODE_V29.set(1);
        let error = pending_read_capture_v29(pending_read_distinct_holder_owner_v29).unwrap_err();
        assert!(PENDING_READ_MUTATED_V29.get());
        assert!(PENDING_READ_JOINED_V29.get(), "pending source-result claim must accept before final graph check: {error:?}");
        assert!(PENDING_READ_FINAL_GATE_V29.get(), "earlier correspondence refusal is not final memory coverage: {error:?}");
        assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source raw address differs from its actual formation or memory history", .. }))), "{error:?}");
    }
}
