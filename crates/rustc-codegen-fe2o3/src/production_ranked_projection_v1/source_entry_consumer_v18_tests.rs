// This inspector runs only through the real rustc Prepared-source parent. Its
// counts describe exact checked original argument bindings, not final helper
// signatures (scoped helpers may be spliced into their root).
pub(crate) fn inspect_actual_source_entry_consumer_v18(
    view: &fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'_>,
    budget: &mut Budget<'_>, hostile: u8,
) -> Result<[usize; 3], fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18> {
    use fe2o3_lower_mir_kernel::{ProductionSourceOwnedViewErrorV18 as Source,
        ProductionSourceOptimizationErrorV18 as Optimize, ProductionSourceScalarInputV18 as Input,
        ProductionSourceScalarArgumentV18 as Argument};
    let semantic = view.source_semantic(budget)?;
    let substituted = (hostile == 1).then(|| {
        fe2o3_mir_model::semantic_mir_v1::InertSemanticMirRequestV1::new_with_callables(
            semantic.target(), semantic.types().to_vec(), semantic.allocations().to_vec(),
            semantic.statics().to_vec(), semantic.vtables().to_vec(), semantic.functions().to_vec(),
            semantic.callables().to_vec(), semantic.roots().to_vec(),
        ).unwrap().admit_exact_v29(
            fe2o3_mir_model::semantic_mir_v1::SemanticMirLimitsV1::default()).unwrap()
    });
    let floor = budget.storage();
    let checked = view.with_checked_optimization_v18(budget, |original, optimized, budget| {
        original.with_optimized_source_scalar_leaves_v18(optimized, 0, budget, |leaves, budget| {
            let source_leaves = leaves.original_leaves(budget)?;
            let counts = std::cell::Cell::new([0usize; 3]);
            leaves.with_checked_entry_writes_v18(budget, |request, budget| {
                let (instance, declaration) = request.original(budget)?;
                let Input::EntryArgument { argument } = request.input_for(declaration, budget)? else {
                    panic!("typed entry request lost its original argument");
                };
                let mut observed = counts.get();
                observed[0] += 1;
                match source_leaves.original_argument(instance, declaration, argument, budget)? {
                    Argument::Root { argument: actual } => {
                        assert_eq!(actual, argument);
                        observed[1] += 1;
                    }
                    Argument::Caller { instance: caller, function, block, operand } => {
                        assert_ne!(caller, instance);
                        let SemanticTerminatorKindV1::Call(call) = function.blocks()[block.index() as usize]
                            .terminator().kind() else { panic!("original caller binding has no call"); };
                        assert!(std::ptr::eq(operand, &call.arguments()[argument as usize]));
                        observed[2] += 1;
                    }
                }
                counts.set(observed);
                source_ranked_consumer_v18::check_source_entry_write_v18(
                    substituted.as_ref().unwrap_or(semantic), source_leaves, request, budget)?;
                if hostile == 2 {
                    let scalar = request.scalar(budget)?;
                    request.check_expression(&ProductionSemanticExpressionV2::Constant {
                        scalar, bits: 0x5a17_c3e9,
                    }, budget)?;
                }
                Ok::<_, ProductionRankedProjectionErrorV1>(())
            }, |_, _| Ok::<_, ProductionRankedProjectionErrorV1>(()))?;
            let counts = counts.get();
            assert!(counts[0] > 0 && counts[2] > 0,
                "actual source must check helper entry bindings, not an empty roster: {counts:?}");
            assert_eq!(counts[0], counts[1] + counts[2]);
            // Exercise the exact production bridge too. The preceding visitor
            // supplies test observations only; it cannot complete this scope.
            optimized_source_consumer_v18::with_checked_source_entry_writes_v18(
                semantic, leaves, budget, |_, _| Ok(()))?;
            Ok::<_, ProductionRankedProjectionErrorV1>((counts, std::mem::size_of::<[usize; 3]>()))
        })
    });
    match checked {
        Ok((output, counts, _receipt)) => {
            assert!(!output.grants_authority());
            drop(output);
            assert_eq!(budget.storage(), floor);
            Ok(counts)
        }
        Err(Optimize::Source(error)) => Err(error),
        Err(Optimize::Adoption(fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
            ProductionRankedProjectionErrorV1::CanonicalAssertions(CanonicalAssertionErrorV1::SourceOwned(error))))) => Err(error),
        Err(Optimize::Adoption(fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
            ProductionRankedProjectionErrorV1::Incomplete(detail)))) if hostile == 1
                && detail == "source scalar original declaration is foreign to semantic owner" => Err(Source::Binding(detail)),
        Err(error) => panic!("actual typed entry source/optimizer/backend refused: {error:?}"),
    }
}
