use super::*;

const STOP: &str = "original scalar-reference payload control completed";
thread_local! {
    static CASE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static COMPLETED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

struct Restore(Option<ScopedSlotCustodyObserverV29>);
impl Drop for Restore {
    fn drop(&mut self) {
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
    }
}

fn observe(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let references = references.unwrap();
    let mut samples = [None, None];
    let mut count = 0;
    'outer: for lowered in emitted.iter().flatten() {
        let instance = lowered.source_call_instance.unwrap();
        let original = instances.instance(instance).unwrap().declaration();
        let archive = lowered.execution_observation.as_ref().unwrap();
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        for anchor in &anchors.rows {
            let ScopedMemoryAnchorKindV29::Access {
                pointer,
                payload: Some(ScopedMemoryPayloadV29::Load { read, .. }),
            } = anchor.kind
            else {
                continue;
            };
            let ScopedMemoryOccurrenceV29::Promoted { definition, .. } = read.occurrence else {
                continue;
            };
            let place = scoped_payload_place_v29(original, read.site, read.role).unwrap();
            if read.prefix != 1
                || !matches!(
                    place.projections().first().map(|p| p.kind()),
                    Some(SemanticProjectionKindV1::Dereference)
                )
            {
                continue;
            }
            let binding = archive.lookup_original_v29(instances, instance, definition, budget)?;
            let SemanticValueBindingV1::SourceReference(_) = binding else {
                continue;
            };
            let (block, statement) = scoped_memory_site_key_v29(read.site);
            let site = SourceReferenceSiteV29 {
                instance,
                block: SemanticBlockIdV1::from_index(block),
                statement: statement.map(|value| value as usize),
            };
            samples[count] = Some((site, place, binding, pointer));
            count += 1;
            if count == 2 {
                break 'outer;
            }
        }
    }
    assert_eq!(
        count, 2,
        "real emission must contain both same-typed reference reads"
    );
    let (site, place, binding, pointer) = samples[0].unwrap();
    let (_, _, peer, peer_pointer) = samples[1].unwrap();
    let SemanticValueBindingV1::SourceReference(original) = binding else {
        unreachable!()
    };
    let SemanticValueBindingV1::SourceReference(peer) = peer else {
        unreachable!()
    };
    assert_eq!(original.source_type, peer.source_type);
    assert_ne!(original.origin, peer.origin);
    assert_ne!(pointer, peer_pointer);

    let floor = budget.storage();
    let mut required = (0, 0);
    with_canonical_call_scratch_v1(budget, |budget| {
        let before = (budget.work(), budget.storage());
        check_source_cell_dereference_payload_v29(
            references, site, place, original, pointer, budget,
        )?;
        required = (budget.work() - before.0, budget.storage() - before.1);
        Ok(())
    })?;
    assert_eq!(budget.storage(), floor);
    assert!(required.0 > 0 && required.1 > 0);
    let mode = CASE.get();
    if mode == 0 {
        COMPLETED.set(true);
        return Ok(());
    }
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        let candidate = clone_execution_cfg_binding_v29(binding, &mut 0, budget)?;
        let SemanticValueBindingV1::SourceReference(mut candidate) = candidate else {
            unreachable!()
        };
        let copied_place = place.clone();
        match mode {
            2 => candidate.origin = peer.origin,
            3 => candidate.owner ^= 1,
            4 => candidate.source[0] ^= 1,
            5 => candidate.source_type = U32,
            6 => {
                candidate.values[0].ty = Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                )
            }
            13 => candidate.values.clear(),
            _ => {}
        }
        if matches!(mode, 8 | 9) {
            budget.charge_work(usize::MAX - budget.work() - required.0 + usize::from(mode == 9))?;
        }
        let filler = if matches!(mode, 10 | 11) {
            usize::MAX - budget.storage() - required.1 + usize::from(mode == 11)
        } else {
            0
        };
        budget.reserve_storage(filler)?;
        let before = (budget.work(), budget.storage());
        let result = if mode == 12 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
            foreign.reserve_storage(budget.storage())?;
            let query = check_source_cell_dereference_payload_v29(
                references,
                site,
                place,
                &candidate,
                pointer,
                &mut foreign,
            );
            assert_eq!((foreign.work(), foreign.storage()), (0, before.1));
            assert_eq!((budget.work(), budget.storage()), before);
            query
        } else {
            check_source_cell_dereference_payload_v29(
                references,
                site,
                if mode == 7 { &copied_place } else { place },
                &candidate,
                if mode == 1 { peer_pointer } else { pointer },
                budget,
            )
        };
        if matches!(mode, 8 | 10) {
            result.as_ref().unwrap();
            assert_eq!(budget.work() - before.0, required.0);
            assert_eq!(budget.storage() - before.1, required.1);
        } else if matches!(mode, 9 | 11 | 12) {
            let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) = &result
            else {
                panic!("expected exact resource refusal: {result:?}");
            };
            assert!(matches!(
                (mode, error),
                (9, ArgumentResourceV1::Work(_))
                    | (11, ArgumentResourceV1::Storage(_))
                    | (12, ArgumentResourceV1::Accounting)
            ));
            let failed = (budget.work(), budget.storage());
            assert!(matches!(check_source_cell_dereference_payload_v29(
                references, site, place, original, pointer, budget),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(next)) if next == *error));
            assert_eq!((budget.work(), budget.storage()), failed);
        } else {
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                ),
                "same-typed or copied source substitutions must fail semantically: {result:?}"
            );
        }
        budget.release_storage(filler)?;
        COMPLETED.set(true);
        if matches!(mode, 9 | 11 | 12) {
            result
        } else {
            Err(source_reference_error_v29(STOP))
        }
    });
    assert_eq!(budget.storage(), floor);
    result
}

#[test]
fn genuine_cell_payloads_complete_and_reject_source_loan_and_pointer_substitution() {
    let _restore = Restore(SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(observe)));
    for mode in 0..14 {
        CASE.set(mode);
        COMPLETED.set(false);
        let result = run_suffix(SuffixCase::Cells, capture, usize::MAX, usize::MAX).0;
        assert!(
            COMPLETED.get(),
            "mode={mode}: every inner assertion must complete: {result:?}"
        );
        match mode {
            0 => result.unwrap(),
            9 => assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            )),
            11 => assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            )),
            12 => assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            )),
            _ => assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported { detail: STOP, .. })
            )),
        }
    }
}
