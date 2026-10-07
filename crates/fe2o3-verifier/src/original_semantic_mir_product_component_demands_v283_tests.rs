//! Unused admitted local schemas exercise demand-domain custody, not live values.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use fe2o3_mir_model::semantic_mir_v1::*;
use std::{cell::Cell, panic::AssertUnwindSafe};

const LIMIT: usize = 256 * 1024 * 1024;
const FLOOR: usize = super::super::super::invocations::tests::FLOOR;

#[derive(Clone, Copy)]
struct Rows {
    function: FunctionId,
    scalar_local: usize,
    product_local: usize,
    scalar_type: TypeId,
    product_type: TypeId,
    pointer: TypeId,
    word: TypeId,
}

fn extend(types: &mut Vec<SemanticTypeDeclV1>, functions: &mut [Function]) -> Rows {
    let word = TypeId::from_index(0);
    let pointer = TypeId::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([230; 32]),
        SemanticLayoutIdentityV1::from_sha256([230; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        Shape::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                word,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let scalar_type = TypeId::from_index(types.len() as u32);
    let product_type = TypeId::from_index(types.len() as u32 + 1);
    for (tag, fields, bytes, alignment, offsets) in [
        (231, vec![word, word], 8, 4, vec![0, 4]),
        (232, vec![pointer, word], 16, 8, vec![0, 8]),
    ] {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(bytes),
                alignment,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            Shape::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
        ));
    }
    let function_id = FunctionId::from_index(functions.len() as u32 - 1);
    let function = functions.last_mut().unwrap();
    let scalar_local = function.locals().len();
    let mut locals = function.locals().to_vec();
    for (tag, ty) in [(240, scalar_type), (241, product_type)] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            function.source(),
        ));
    }
    *function = Function::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        locals,
        function.entry(),
        function.blocks().to_vec(),
    )
    .unwrap();
    Rows {
        function: function_id,
        scalar_local,
        product_local: scalar_local + 1,
        scalar_type,
        product_type,
        pointer,
        word,
    }
}

fn fixture(
    work: usize,
    storage: usize,
    inspect: impl FnOnce(Rows, &SourceSlots<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    let rows = Cell::new(None);
    super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| rows.set(Some(extend(types, functions))),
        |plan, out| {
            super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                inspect(rows.get().unwrap(), slots, out)
            })
        },
    )
}

fn field(local: usize, field: u32, ty: TypeId) -> Place {
    Place::new(
        SemanticLocalIdV1::from_index(local as u32),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()],
        ty,
    )
    .unwrap()
}

fn inspect(rows: Rows, slots: &SourceSlots<'_, '_>, out: &mut Writer<'_, '_>) -> Result<()> {
    let demands = ComponentDemandsV42::derive(slots, rows.function, out)?;
    assert_eq!(
        demands.local_domain_v283(slots, rows.function, rows.scalar_local, out)?,
        (rows.scalar_type, ComponentDomainV283::ScalarV42, 2)
    );
    assert_eq!(
        demands.local_domain_v283(slots, rows.function, rows.product_local, out)?,
        (rows.product_type, ComponentDomainV283::ProductV282, 2)
    );
    for (local, ty) in [
        (rows.scalar_local, rows.word),
        (rows.product_local, rows.pointer),
    ] {
        assert_eq!(
            demands.projected_range_v283(slots, rows.function, &field(local, 0, ty), out)?,
            Some(0..1)
        );
        assert_eq!(
            demands.projected_range_v283(slots, rows.function, &field(local, 1, rows.word), out,)?,
            Some(1..2)
        );
        for leaf in 0..2 {
            assert!(!demands.leaf_required(rows.function, 0, local, leaf, out)?);
        }
    }
    Ok(())
}

#[test]
fn product_demand_cache_keeps_distinct_owner_derived_domains_and_exact_projection_types() {
    let result = fixture(LIMIT, LIMIT, |rows, slots, out| {
        inspect(rows, slots, out)?;
        let demands = ComponentDemandsV42::derive(slots, rows.function, out)?;
        assert!(matches!(
            demands.projected_range_v283(
                slots,
                rows.function,
                &field(rows.product_local, 0, rows.word),
                out,
            ),
            Err(Error::Statement(_))
        ));
        Ok(())
    });
    result.0.unwrap();
    assert_eq!(result.2, FLOOR);
}

#[test]
fn product_demand_cache_complete_derivation_has_exact_and_one_short_budgets() {
    let baseline = fixture(LIMIT, LIMIT, inspect);
    baseline.0.unwrap();
    let exact = fixture(baseline.1, baseline.3, inspect);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (baseline.1, FLOOR, baseline.3));
    assert!(matches!(fixture(baseline.1 - 1, baseline.3, inspect).0,
        Err(Error::Resource(Resource::Work(error))) | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.actual() == baseline.1 && error.limit() == baseline.1 - 1));
    assert!(matches!(fixture(baseline.1, baseline.3 - 1, inspect).0,
        Err(Error::Resource(Resource::Storage(error))) | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.actual() == baseline.3 && error.limit() == baseline.3 - 1));
}

#[test]
fn product_demand_cache_rejects_foreign_function_and_out_of_range_components() {
    fixture(LIMIT, LIMIT, |rows, slots, out| {
        let demands = ComponentDemandsV42::derive(slots, rows.function, out)?;
        assert!(matches!(
            demands.local_domain_v283(slots, FunctionId::from_index(0), rows.product_local, out),
            Err(Error::Statement(_))
        ));
        assert!(matches!(
            demands.leaf_required(rows.function, 0, rows.product_local, 2, out),
            Err(Error::Statement(_))
        ));
        assert!(matches!(
            demands.local_domain_v283(slots, rows.function, usize::MAX, out),
            Err(Error::Statement(_))
        ));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn product_demand_cache_rejects_equal_source_distinct_slots_owner() {
    fixture(LIMIT, LIMIT, |rows, slots, out| {
        let demands = ComponentDemandsV42::derive(slots, rows.function, out)?;
        let relation = slots.correspondence(out)?;
        let plan = super::super::super::invocations::InvocationPlan::derive(
            relation.source(out.budget)?,
            out,
        )?;
        let foreign = SourceSlots::derive(&plan, relation, out)?;
        assert_eq!(
            foreign.product_component_count_v282(rows.product_type, out)?,
            Some(2)
        );
        assert!(matches!(
            demands.local_domain_v283(&foreign, rows.function, rows.product_local, out),
            Err(Error::Statement(
                "original aggregate component demand differs from its source CFG"
            ))
        ));
        assert!(matches!(
            demands.projected_range_v283(
                &foreign,
                rows.function,
                &field(rows.product_local, 0, rows.pointer),
                out
            ),
            Err(Error::Statement(
                "original aggregate component demand differs from its source CFG"
            ))
        ));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn product_demand_synthetic_projected_transfer_keeps_other_atom_and_move_demand() {
    fixture(LIMIT, LIMIT, |rows, slots, out| {
        let demands = ComponentDemandsV42::derive(slots, rows.function, out)?;
        let semantic = slots
            .correspondence(out)?
            .source(out.budget)?
            .source_semantic(out.budget)?;
        let function = &semantic.functions()[rows.function.index() as usize];
        let mut generated = vec![0; demands.words];
        let mut killed = vec![0; demands.words];
        let base = demands.locals[rows.product_local].range.start;
        let mut facts = Facts {
            slots,
            function,
            locals: &demands.locals,
            generated: &mut generated,
            killed: &mut killed,
        };
        // These are synthetic transfer events over admitted unused schemas,
        // not an assertion that this original function executes a Product.
        facts.place(&field(rows.product_local, 0, rows.pointer), false, out)?;
        facts.operand(&Operand::Move(field(rows.product_local, 1, rows.word)), out)?;
        drop(facts);
        assert_eq!(generated[base / 64] & (1 << (base % 64)), 0);
        assert_ne!(generated[(base + 1) / 64] & (1 << ((base + 1) % 64)), 0);
        assert_ne!(killed[base / 64] & (1 << (base % 64)), 0);
        assert_ne!(killed[(base + 1) / 64] & (1 << ((base + 1) % 64)), 0);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn product_demand_cache_rejects_foreign_account_and_restores_original_account() {
    let original_floor = Cell::new(None);
    let result = fixture(LIMIT, LIMIT, |rows, slots, out| {
        let demands = ComponentDemandsV42::derive(slots, rows.function, out)?;
        let floor = out.budget.storage();
        original_floor.set(Some(floor));
        let slot = std::ptr::from_ref(&*out.budget);
        let ledger = out.budget.work_ledger_identity_v1();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT + floor)?;
        let mut foreign = Writer::new(&mut budget)?;
        assert!(matches!(
            demands.local_domain_v283(slots, rows.function, rows.product_local, &mut foreign),
            Err(Error::Resource(Resource::Accounting))
                | Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        assert_eq!(std::ptr::from_ref(&*out.budget), slot);
        assert!(ledger == out.budget.work_ledger_identity_v1());
        assert_eq!(out.budget.storage(), floor);
        assert!(matches!(
            demands.local_domain_v283(slots, rows.function, rows.product_local, out),
            Err(Error::Resource(Resource::Accounting))
                | Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        assert_eq!(out.budget.storage(), floor);
        Ok(())
    });
    assert!(matches!(
        result.0,
        Err(Error::Resource(Resource::Accounting))
            | Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
    // The owner latches lost custody; containing source credits must not be
    // refunded even though the original account itself was never replaced.
    assert!(result.2 > FLOOR);
    assert!(result.2 <= original_floor.get().expect("foreign query reached"));
}

#[test]
fn product_demand_cache_unwind_releases_owned_domain_storage() {
    let result = fixture(LIMIT, LIMIT, |rows, slots, out| {
        let floor = out.budget.storage();
        let ledger = out.budget.work_ledger_identity_v1();
        let slot = std::ptr::from_ref(&*out.budget);
        let panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let demands = ComponentDemandsV42::derive(slots, rows.function, out).unwrap();
            assert_eq!(
                demands.locals[rows.product_local].domain,
                ComponentDomainV283::ProductV282
            );
            std::panic::panic_any(283_u32);
        }))
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<u32>(), Some(&283));
        assert_eq!(std::ptr::from_ref(&*out.budget), slot);
        assert!(ledger == out.budget.work_ledger_identity_v1());
        assert!(out.budget.storage() >= floor);
        Ok(())
    });
    result.0.unwrap();
    assert_eq!(result.2, FLOOR);
}

fn live_transform(types: &mut Vec<SemanticTypeDeclV1>, functions: &mut [Function]) -> Rows {
    // This re-admitted semantic-MIR fixture is not rustc-generated source.
    // Only scalar local 4 is address-observed; Product local 6 is constructed
    // and used whole, so the production storage classifier leaves it promotable.
    let rows = extend(types, functions);
    for function in &mut functions[..2] {
        assert_eq!(function.locals().len(), 4);
        let source = function.source();
        let mut locals = function.locals().to_vec();
        for (index, ty) in [
            rows.word,
            rows.pointer,
            rows.product_type,
            rows.product_type,
        ]
        .into_iter()
        .enumerate()
        {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([240 + index as u8; 32]),
                ty,
                LocalRole::Temporary,
                source,
            ));
        }
        let place =
            |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
        let assignment = |local, ty, value| {
            SemanticStatementV1::new(
                source,
                Statement::Assign(SemanticAssignmentV1::new(
                    place(local, ty),
                    SemanticRvalueV1::new(ty, value),
                )),
            )
        };
        let mut blocks = function.blocks().to_vec();
        let mut entry = vec![
            assignment(
                4,
                rows.word,
                Rvalue::Use(Operand::Copy(place(1, rows.word))),
            ),
            assignment(
                5,
                rows.pointer,
                Rvalue::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: place(4, rows.word),
                },
            ),
            assignment(
                6,
                rows.product_type,
                Rvalue::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![
                            Operand::Copy(place(5, rows.pointer)),
                            Operand::Copy(place(2, rows.word)),
                        ],
                    )
                    .unwrap(),
                ),
            ),
        ];
        entry.extend_from_slice(blocks[0].statements());
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            blocks[0].source(),
            entry,
            blocks[0].terminator().clone(),
        )
        .unwrap();
        let mut continuation = vec![assignment(
            7,
            rows.product_type,
            Rvalue::Use(Operand::Copy(place(6, rows.product_type))),
        )];
        continuation.extend_from_slice(blocks[1].statements());
        blocks[1] = SemanticBasicBlockV1::new(
            blocks[1].identity(),
            blocks[1].source(),
            continuation,
            blocks[1].terminator().clone(),
        )
        .unwrap();
        *function = Function::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            source,
            function.abi().clone(),
            locals,
            function.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(function.kernel_entry().unwrap().clone());
    }
    rows
}

#[test]
fn product_demand_readmitted_source_ssa_keeps_both_atoms_across_an_actual_call() {
    use super::super::slots::ProductAtomV282;
    use fe2o3_mir_model::SsaVariableIdV1 as Variable;
    let rows = Cell::new(None);
    let result = super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |types, functions| rows.set(Some(live_transform(types, functions))),
        |plan, out| {
            super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let rows = rows.get().unwrap();
                let source = plan.source(out)?;
                let archive = source.source_ssa(out.budget)?;
                let semantic = source.source_semantic(out.budget)?;
                let pointer = slots.product_component_v282(rows.product_type, 0, out)?;
                assert_eq!(pointer.path(out)?, &[0_u32]);
                assert_eq!(pointer.source_type(out)?, rows.pointer);
                assert_eq!(
                    pointer.atom(out)?,
                    ProductAtomV282::Pointer {
                        mutable: true,
                        reference: false,
                    }
                );
                let scalar = slots.product_component_v282(rows.product_type, 1, out)?;
                assert_eq!(scalar.path(out)?, &[1_u32]);
                assert_eq!(scalar.source_type(out)?, rows.word);
                assert_eq!(
                    scalar.atom(out)?,
                    ProductAtomV282::Scalar(super::super::super::ScalarV30::from_source(
                        semantic.types(),
                        rows.word
                    )?)
                );
                let frames = super::super::source_frame_plan::FramePlan::derive(plan, slots, out)?;
                let mut observed = 0;
                for (frame_index, frame) in frames.frames.iter().enumerate() {
                    if frame.instance != 0 {
                        continue;
                    }
                    assert!(frame.active);
                    let original = archive.plan_for_function(frame.function).unwrap();
                    let ssa = original.plan();
                    assert!(ssa.promoted_variables().contains(&Variable::new(6)));
                    assert!(
                        !original
                            .retained_cross_edge_variables()
                            .contains(&Variable::new(6))
                    );
                    assert!(!slots.has_original_object(frame.root, 0, 6, out)?);
                    let function = &semantic.functions()[frame.function.index() as usize];
                    let Statement::Assign(constructed) =
                        function.blocks()[0].statements()[2].kind()
                    else {
                        panic!("actual original Product construction");
                    };
                    assert!(matches!(constructed.value().kind(), Rvalue::Aggregate(_)));
                    assert_eq!(constructed.destination().local().index(), 6);
                    assert_eq!(constructed.value().result_type(), rows.product_type);
                    let call = frames.calls[frame.calls.clone()]
                        .iter()
                        .find(|call| call.block == 0)
                        .unwrap();
                    assert!(call.reachable && call.child_active && call.child.is_some());
                    let carry = call
                        .demands
                        .clone()
                        .find(|at| frames.demands[*at].source.local == 6)
                        .unwrap();
                    let current = frames.cuts[frame.cuts.clone()]
                        .iter()
                        .find(|cut| cut.block == 1)
                        .unwrap();
                    assert!(current.reachable);
                    let current = current
                        .demands
                        .clone()
                        .find(|at| frames.demands[*at].source.local == 6)
                        .unwrap();
                    for index in [carry, current] {
                        let demand = &frames.demands[index].source;
                        let endpoint = slots.correspondence(out)?.ssa_typed_endpoint_v36(
                            frame.root,
                            frame.instance,
                            demand.value,
                            out.budget,
                        )?;
                        assert_eq!(endpoint.source_type(out.budget)?, rows.product_type);
                        assert_eq!(endpoint.source_local(out.budget)?.index(), 6);
                        for atom in 0..2 {
                            assert!(frames.leaf_required(frame_index, index, atom, out)?);
                        }
                    }
                    observed += 1;
                }
                assert_eq!(observed, 2);
                Ok(())
            })
        },
    );
    result.0.unwrap();
    assert_eq!(result.2, FLOOR);
}
