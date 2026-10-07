//! Re-admitted semantic MIR coverage, not a rustc-generated source matrix.
use super::super::super::{
    expanded_generation::ExpandedGenerationV221, paired::ExpandedScalarBindingsV196,
    source_frame_plan::FramePlan, tile_target::TileMicroCutsV180,
};
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, EndiannessV2, FormalIndexWidth};
use fe2o3_mir_model::{SsaBlockIdV1, SsaVariableIdV1};

const LIMIT: usize = 512 * 1024 * 1024;

fn product_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = owner();
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let arguments = root
        .locals()
        .iter()
        .enumerate()
        .filter(|(_, local)| matches!(local.role(), SemanticLocalRoleV1::Argument(_)))
        .collect::<Vec<_>>();
    let (slice_local, slice) = arguments.iter().copied().find(|(_, local)| {
        matches!(types[local.ty().index() as usize].shape(), SemanticTypeShapeV1::Pointer(pointer)
            if matches!(types[pointer.pointee().index() as usize].shape(), SemanticTypeShapeV1::Slice { .. }))
    }).unwrap();
    let (word_local, word) = arguments
        .iter()
        .copied()
        .find(|(_, local)| {
            matches!(
                types[local.ty().index() as usize].shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32
                })
            )
        })
        .unwrap();
    let (slice, word) = (slice.ty(), word.ty());
    let slice_layout = types[slice.index() as usize].layout();
    let word_layout = types[word.index() as usize].layout();
    let align = slice_layout
        .alignment_bytes()
        .max(word_layout.alignment_bytes());
    let offset = (slice_layout.size_bytes().unwrap() + word_layout.alignment_bytes() - 1)
        & !(word_layout.alignment_bytes() - 1);
    let size = (offset + word_layout.size_bytes().unwrap() + align - 1) & !(align - 1);
    let product = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([245; 32]),
        SemanticLayoutIdentityV1::from_sha256([245; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(size),
            align,
            SemanticAggregateLayoutV1::new(vec![0, offset], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![slice, word]).unwrap()),
    ));
    let mut locals = root.locals().to_vec();
    let held = locals.len();
    for tag in [246, 247] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            product,
            SemanticLocalRoleV1::Temporary,
            root.source(),
        ));
    }
    let place = |local, ty| {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local as u32), vec![], ty).unwrap()
    };
    let assignment = |local, value| {
        SemanticStatementV1::new(
            root.source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, product),
                SemanticRvalueV1::new(product, value),
            )),
        )
    };
    let mut blocks = root.blocks().to_vec();
    let SemanticTerminatorKindV1::Call(call) = blocks[1].terminator().kind() else {
        panic!("retained root helper call");
    };
    let continuation = call.destination().unwrap().edge().target().index() as usize;
    assert_ne!(continuation, 1);
    let mut statements = blocks[1].statements().to_vec();
    statements.push(assignment(
        held,
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Tuple,
                vec![
                    SemanticOperandV1::Copy(place(slice_local, slice)),
                    SemanticOperandV1::Copy(place(word_local, word)),
                ],
            )
            .unwrap(),
        ),
    ));
    blocks[1] = SemanticBasicBlockV1::new(
        blocks[1].identity(),
        blocks[1].source(),
        statements,
        blocks[1].terminator().clone(),
    )
    .unwrap();
    let mut statements = vec![assignment(
        held + 1,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(held, product))),
    )];
    statements.extend_from_slice(blocks[continuation].statements());
    blocks[continuation] = SemanticBasicBlockV1::new(
        blocks[continuation].identity(),
        blocks[continuation].source(),
        statements,
        blocks[continuation].terminator().clone(),
    )
    .unwrap();
    functions[0] = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        locals,
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let semantic = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn fixture(
    layout: Layout,
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &TileExpansion<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_fixture_with_preparation(
        layout,
        work,
        storage,
        |budget| prepared_with_owner(budget, product_owner),
        examine,
    )
}

fn emit(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    _: &TileExpansion<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let model = ExpandedGenerationV221::derive(
        plan,
        slots,
        FormalIndexWidth::Bits64,
        EndiannessV2::Little,
        out,
    )?;
    let cuts = TileMicroCutsV180::derive(model.target(out)?, plan, out)?;
    let frames = FramePlan::derive(plan, slots, out)?;
    let source = plan.source(out)?;
    let semantic = source.source_semantic(out.budget)?;
    let function = &semantic.functions()[0];
    let held = function.locals().len() - 2;
    let product = function.locals()[held].ty();
    let archive = source.source_ssa(out.budget)?;
    let ssa = archive
        .plan_for_function(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    assert!(
        ssa.plan()
            .promoted_variables()
            .contains(&SsaVariableIdV1::new(held as u32))
    );
    assert!(
        !ssa.retained_cross_edge_variables()
            .contains(&SsaVariableIdV1::new(held as u32))
    );
    assert!(!slots.has_original_object(0, 0, held as u32, out)?);
    assert!(slots.is_product_v282(product, out)?);
    assert_eq!(slots.product_component_count_v282(product, out)?, Some(2));
    let frame_index = frames
        .frames
        .iter()
        .position(|frame| frame.root == 0 && frame.instance == 0)
        .unwrap();
    let frame = &frames.frames[frame_index];
    let call = frames.calls[frame.calls.clone()]
        .iter()
        .find(|call| call.block == 1)
        .unwrap();
    assert!(call.reachable && call.child_active);
    let demand = call
        .demands
        .clone()
        .find(|at| frames.demands[*at].source.local == held)
        .unwrap();
    let SemanticTerminatorKindV1::Call(original_call) = function.blocks()[1].terminator().kind()
    else {
        panic!("original call");
    };
    let continuation = original_call.destination().unwrap().edge().target().index();
    assert!(
        ssa.plan()
            .live_in(SsaBlockIdV1::new(continuation))
            .is_some()
    );
    let cut = frames.cuts[frame.cuts.clone()]
        .iter()
        .find(|cut| cut.block == continuation as usize)
        .unwrap();
    let current = cut
        .demands
        .clone()
        .find(|at| frames.demands[*at].source.local == held)
        .unwrap();
    for at in [demand, current] {
        for ordinal in 0..2 {
            assert!(frames.leaf_required(frame_index, at, ordinal, out)?);
        }
    }
    let start = out.text.len();
    model.emit_frame_contracts_v281(&frames, &cuts, out)?;
    model.finish(out)?;
    let text = &out.text[start..];
    assert!(text.contains("InvocationSourceProductAtomV282::Carrier(original)"));
    assert!(text.contains("invocation_source_product_atom_current_v282(source,"));
    assert!(text.contains("invocation_runtime_little_endian_v36()"));
    assert!(text.contains("let path = seq![0int,"));
    assert!(text.contains("let path = seq![1int,"));
    assert!(text.contains(
        "invocation_value_related_v36(original, actual, map, source.machine.memory, target.memory)"
    ));
    assert!(text.contains("micro: MemoryMicroStateV30"));
    assert!(text.contains("micro.observations == target_prefix"));
    assert!(text.contains("invocation_source_byte_state_well_formed_v36(source)"));
    assert!(!text.contains("proof fn"));
    Ok(())
}

#[test]
fn expanded_product_carriers_bind_readmitted_current_and_suspended_source_atoms() {
    for layout in [Layout::Blocked, Layout::Striped] {
        fixture(layout, LIMIT, LIMIT, emit).0.unwrap();
    }
}

#[test]
fn expanded_product_carrier_complete_emission_has_exact_and_short_budgets() {
    let baseline = fixture(Layout::Blocked, LIMIT, LIMIT, emit);
    baseline.0.unwrap();
    let exact = fixture(Layout::Blocked, baseline.1, baseline.3, emit);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (baseline.1, baseline.2, baseline.3)
    );
    for (work, storage) in [(baseline.1 - 1, baseline.3), (baseline.1, baseline.3 - 1)] {
        let result = fixture(Layout::Blocked, work, storage, emit);
        assert!(matches!(
            result.0,
            Err(Error::Resource(Resource::Work(_) | Resource::Storage(_)))
                | Err(Error::Source(
                    fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                        Resource::Work(_) | Resource::Storage(_)
                    )
                ))
        ));
    }
}

#[test]
fn expanded_product_carrier_query_refuses_scalar_source_and_invalid_component() {
    fixture(Layout::Blocked, LIMIT, LIMIT, |plan, slots, _, out| {
        let model = ExpandedGenerationV221::derive(
            plan,
            slots,
            FormalIndexWidth::Bits64,
            EndiannessV2::Little,
            out,
        )?;
        let frames = FramePlan::derive(plan, slots, out)?;
        let bindings = ExpandedScalarBindingsV196::derive(slots, model.target(out)?, out)?;
        let source = plan.source(out)?;
        let semantic = source.source_semantic(out.budget)?;
        let held = semantic.functions()[0].locals().len() - 2;
        let frame = frames
            .frames
            .iter()
            .find(|frame| frame.root == 0 && frame.instance == 0)
            .unwrap();
        let call = frames.calls[frame.calls.clone()]
            .iter()
            .find(|call| call.block == 1)
            .unwrap();
        let demand = &frames.demands[call
            .demands
            .clone()
            .find(|at| frames.demands[*at].source.local == held)
            .unwrap()]
        .source;
        assert!(matches!(
            bindings.emit_source_product_conjunct_v283(
                plan,
                0,
                0,
                demand.value,
                2,
                FormalIndexWidth::Bits64,
                out
            ),
            Err(Error::Statement(_))
        ));
        let scalar_type = semantic.functions()[0]
            .locals()
            .iter()
            .find(|local| {
                matches!(
                    semantic.types()[local.ty().index() as usize].shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32
                    })
                )
            })
            .unwrap()
            .ty();
        let scalar = &frames.demands[call
            .demands
            .clone()
            .find(|at| {
                semantic.functions()[0].locals()[frames.demands[*at].source.local].ty()
                    == scalar_type
            })
            .unwrap()];
        assert!(matches!(
            bindings.emit_source_product_conjunct_v283(
                plan,
                0,
                0,
                scalar.source.value,
                0,
                FormalIndexWidth::Bits64,
                out
            ),
            Err(Error::Statement(_))
        ));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn expanded_product_carrier_queries_retain_foreign_and_refunded_account_refusals() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for foreign in [true, false] {
        let mut reached = false;
        let result = fixture(Layout::Blocked, LIMIT, LIMIT, |plan, slots, _, out| {
            let model = ExpandedGenerationV221::derive(
                plan,
                slots,
                FormalIndexWidth::Bits64,
                EndiannessV2::Little,
                out,
            )?;
            let frames = FramePlan::derive(plan, slots, out)?;
            let bindings = ExpandedScalarBindingsV196::derive(slots, model.target(out)?, out)?;
            let source = plan.source(out)?;
            let semantic = source.source_semantic(out.budget)?;
            let held = semantic.functions()[0].locals().len() - 2;
            let frame = frames
                .frames
                .iter()
                .find(|frame| frame.root == 0 && frame.instance == 0)
                .unwrap();
            let call = frames.calls[frame.calls.clone()]
                .iter()
                .find(|call| call.block == 1)
                .unwrap();
            let value = frames.demands[call
                .demands
                .clone()
                .find(|at| frames.demands[*at].source.local == held)
                .unwrap()]
            .source
            .value;
            reached = true;
            let error = if foreign {
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(out.budget.storage())?;
                let mut writer = Writer::new(&mut budget)?;
                bindings
                    .emit_source_product_conjunct_v283(
                        plan,
                        0,
                        0,
                        value,
                        0,
                        FormalIndexWidth::Bits64,
                        &mut writer,
                    )
                    .unwrap_err()
            } else {
                out.budget.release_storage(1)?;
                bindings
                    .emit_source_product_conjunct_v283(
                        plan,
                        0,
                        0,
                        value,
                        0,
                        FormalIndexWidth::Bits64,
                        out,
                    )
                    .unwrap_err()
            };
            assert!(matches!(
                error,
                Error::Resource(Resource::Accounting)
                    | Error::Source(SourceError::Resource(Resource::Accounting))
            ));
            assert!(matches!(
                bindings.emit_source_product_conjunct_v283(
                    plan,
                    0,
                    0,
                    value,
                    1,
                    FormalIndexWidth::Bits64,
                    out
                ),
                Err(Error::Resource(Resource::Accounting)
                    | Error::Source(SourceError::Resource(Resource::Accounting)))
            ));
            Err(error)
        });
        assert!(reached);
        assert!(matches!(
            result.0,
            Err(Error::Resource(Resource::Accounting)
                | Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
    }
}
