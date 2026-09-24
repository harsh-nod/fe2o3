//! Inert MIR34 fixture through existing pre-ranked materialization; not rustc custody.
use super::*;
use fe2o3_kernel_ir::{AssemblyEffect, AssemblyOption};
use fe2o3_mir_model::semantic_mir_v1::*;

mod inline_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/helper_inline_singleton_semantic_fixture_v1.rs"
    ));
}

fn inline_owner(kind: SemanticGfx942InlineInstructionV30) -> ProductionPreRankedKirOwnerV1 {
    let semantic = inline_fixture::Fixture::new(kind).admit();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        &semantic,
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "helper_value_source",
            [31; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 512 * 1024 * 1024);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}
fn helper(module: &Module) -> &Function {
    module
        .functions
        .iter()
        .find(|f| f.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
        .unwrap()
}
fn join(
    owner: &ProductionPreRankedKirOwnerV1,
    function: &Function,
    correspondence: &SemanticKirCorrespondenceV1,
    meter: &mut dyn Meter,
) -> Result<(), Error> {
    native_helper_inline_source_v30(
        owner.semantic_ssa.source_semantic(),
        correspondence,
        SemanticFunctionIdV1::from_index(0),
        1,
        function,
        meter,
    )
}
#[test]
fn native_singleton_return_requires_exact_ordinary_u32_layout_without_general_aggregate_admission()
{
    let fixture = inline_fixture::Fixture::new(SemanticGfx942InlineInstructionV30::VOrB32);
    assert_eq!(
        singleton_u32_return_type_v30(&fixture.types, inline_fixture::SINGLETON),
        Ok(Type::Scalar(ScalarType::U32))
    );
    for mutation in 0..7 {
        let mut types = fixture.types.clone();
        let current = &types[inline_fixture::SINGLETON.index() as usize];
        let mut shape = current.shape().clone();
        let mut layout = current.layout().clone();
        match mutation {
            0 => {
                shape = SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![inline_fixture::WORD]).unwrap(),
                )
            }
            1 => {
                shape = SemanticTypeShapeV1::Tuple(
                    SemanticAggregateTypeV1::new(vec![inline_fixture::WORD, inline_fixture::WORD])
                        .unwrap(),
                )
            }
            2 => {
                shape = SemanticTypeShapeV1::Tuple(
                    SemanticAggregateTypeV1::new(vec![inline_fixture::SINGLETON]).unwrap(),
                )
            }
            3 => {
                layout = SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(4),
                    4,
                    *layout.backend_repr(),
                    false,
                )
                .unwrap()
            }
            4 => {
                layout = inline_fixture::singleton_type(SemanticBackendReprV1::memory(true))
                    .layout()
                    .clone()
            }
            5 => {
                let word = &types[inline_fixture::WORD.index() as usize];
                types[inline_fixture::WORD.index() as usize] = SemanticTypeDeclV1::new(
                    word.identity(),
                    word.layout_identity(),
                    word.layout().clone(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: true,
                        bits: 32,
                    }),
                );
            }
            6 => {
                shape = SemanticTypeShapeV1::Tuple(
                    SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(u32::MAX)])
                        .unwrap(),
                )
            }
            _ => unreachable!(),
        }
        let current = &types[inline_fixture::SINGLETON.index() as usize];
        types[inline_fixture::SINGLETON.index() as usize] =
            SemanticTypeDeclV1::new(current.identity(), current.layout_identity(), layout, shape);
        assert!(
            singleton_u32_return_type_v30(&types, inline_fixture::SINGLETON).is_err(),
            "mutation {mutation}"
        );
    }
    assert!(singleton_u32_return_type_v30(&fixture.types, inline_fixture::WORD).is_err());
}
#[test]
fn actual_preowner_singleton_helpers_join_all_six_inline_values_to_exact_source_occurrences() {
    for kind in inline_fixture::KINDS {
        let owner = inline_owner(kind);
        let module = owner.executable.module();
        let entry = entry(module);
        let (location, operation) = call(entry);
        let (result, storage, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
            join(&owner, helper(module), &owner.correspondence, meter)?;
            with_native_helper_values(
                owner.semantic_ssa.source_semantic(),
                module,
                &owner.correspondence,
                SemanticFunctionIdV1::from_index(0),
                entry,
                meter,
                |context, meter| {
                    let template = context.root_call(entry, location, operation, meter)?;
                    let (expression, bytes) =
                        template.instantiate(&[constant(7), constant(11)], meter)?;
                    match (&expression, kind) {
                        (
                            NormalizedScalarExpressionV1::Constant { scalar, bits: 7 },
                            SemanticGfx942InlineInstructionV30::VMovB32,
                        ) => {
                            assert_eq!(
                                *scalar,
                                ProductionSemanticScalarTypeV2::Integer {
                                    signed: false,
                                    bits: 32
                                }
                            );
                        }
                        (
                            NormalizedScalarExpressionV1::Binary {
                                overflow, lhs, rhs, ..
                            },
                            _,
                        ) => {
                            assert_eq!(*overflow, ProductionOverflowContractV2::Wrapping);
                            assert_eq!(**lhs, constant(7));
                            assert_eq!(**rhs, constant(11));
                        }
                        _ => panic!("wrong helper value for {kind:?}"),
                    }
                    drop(expression);
                    meter.release(bytes)?;
                    Ok(())
                },
            )
        });
        result.unwrap();
        assert_eq!(storage, 4096);
    }
}
#[test]
fn native_helper_occurrence_join_rejects_foreign_ids_options_hidden_effects_missing_and_extra_isa()
{
    let owner = inline_owner(SemanticGfx942InlineInstructionV30::VOrB32);
    let original = helper(owner.executable.module());
    for mutation in 0..10 {
        let mut function = original.clone();
        let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
        let index = operations
            .iter()
            .position(|o| matches!(o.kind, OperationKind::InlineAssembly(_)))
            .unwrap();
        if mutation == 8 {
            let mut extra = operations[index].clone();
            extra.results[0].id = ValueId(u32::MAX - 1);
            operations.push(extra);
        } else if mutation == 9 {
            operations.remove(index);
        } else {
            let OperationKind::InlineAssembly(assembly) = &mut operations[index].kind else {
                unreachable!()
            };
            match mutation {
                0 => assembly.source.frontend_unit = [99; 32],
                1 => assembly.source.function = [99; 32],
                2 => assembly.source.contract = [99; 32],
                3 => assembly.source.statement = [99; 32],
                4 => assembly.mnemonic = "v_xor_b32".into(),
                5 => {
                    assembly.options.insert(AssemblyOption::Pure);
                }
                6 => {
                    assembly.options.clear();
                }
                7 => {
                    assembly
                        .declared_effects
                        .insert(AssemblyEffect::WriteGlobal);
                }
                _ => unreachable!(),
            }
        }
        let (result, storage, _, _, _) = run(1_000_000, 1 << 24, |meter, _| {
            join(&owner, &function, &owner.correspondence, meter)
        });
        assert!(result.is_err(), "mutation {mutation}");
        assert_eq!(storage, 4096);
    }
}
#[test]
fn native_helper_occurrence_join_rejects_foreign_root_missing_duplicate_and_truncated_spans() {
    let owner = inline_owner(SemanticGfx942InlineInstructionV30::VSubU32);
    let function = helper(owner.executable.module());
    for mutation in 0..4 {
        let mut correspondence = owner.correspondence.clone();
        let mut spans = correspondence.terminator_operation_spans.to_vec();
        let index = spans
            .iter()
            .position(|s| {
                s.semantic_function() == SemanticFunctionIdV1::from_index(1)
                    && s.semantic_block().index() == 0
            })
            .unwrap();
        match mutation {
            0 => spans[index].correspondence_owner = SemanticFunctionIdV1::from_index(1),
            1 => {
                spans.remove(index);
            }
            2 => spans.push(spans[index]),
            3 => spans[index].operation_count = 0,
            _ => unreachable!(),
        }
        correspondence.terminator_operation_spans = spans.into_boxed_slice();
        let (result, storage, _, _, _) = run(1_000_000, 1 << 24, |meter, _| {
            join(&owner, function, &correspondence, meter)
        });
        assert!(result.is_err(), "mutation {mutation}");
        assert_eq!(storage, 4096);
    }
}
#[test]
fn native_inline_helper_cache_exact_cumulative_work_and_storage_refuse_one_short() {
    let owner = inline_owner(SemanticGfx942InlineInstructionV30::VSubU32);
    let module = owner.executable.module();
    let entry = entry(module);
    let (location, operation) = call(entry);
    let probe = |meter: &mut TestMeter<'_, '_>, _: &Cell<bool>| {
        with_native_helper_values(
            owner.semantic_ssa.source_semantic(),
            module,
            &owner.correspondence,
            SemanticFunctionIdV1::from_index(0),
            entry,
            meter,
            |context, meter| {
                context
                    .root_call(entry, location, operation, meter)
                    .map(|_| ())
            },
        )
    };
    let (result, storage, work, peak, _) = run(1_000_000, 1 << 24, probe);
    result.unwrap();
    assert_eq!(storage, 4096);
    for (work, peak, succeeds) in [
        (work, peak, true),
        (work - 1, peak, false),
        (work, peak - 1, false),
    ] {
        let (result, storage, _, _, _) = run(work, peak, probe);
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(storage, 4096);
    }
}

#[test]
fn native_inline_marker_type_is_exact_ordinary_u32_not_a_structurally_similar_nominal_type() {
    let fixture = inline_fixture::Fixture::new(SemanticGfx942InlineInstructionV30::VOrB32);
    assert!(ordinary_inline_u32_type_v30(
        &fixture.types,
        inline_fixture::WORD
    ));
    assert!(!ordinary_inline_u32_type_v30(
        &fixture.types,
        inline_fixture::SINGLETON
    ));
    assert!(!ordinary_inline_u32_type_v30(
        &fixture.types,
        SemanticTypeIdV1::from_index(u32::MAX)
    ));
    for kind in [
        SemanticRustTypeKindV1::Str,
        SemanticRustTypeKindV1::Usize,
        SemanticRustTypeKindV1::Isize,
    ] {
        let mut types = fixture.types.clone();
        types[inline_fixture::WORD.index() as usize] = types[inline_fixture::WORD.index() as usize]
            .clone()
            .with_rust_type_kind(kind);
        assert!(
            !ordinary_inline_u32_type_v30(&types, inline_fixture::WORD),
            "{kind:?}"
        );
    }
}
