#[cfg(test)]
mod progress_graph_release_v2_tests {
    use super::*;
    use crate::production_analysis::pliron_progress::{
        PreparedProgressGraphV2, preflight_progress_graph_resource_upper_bound_v2,
    };
    use dialect_kernel::ReturnOp;
    use pliron::{builtin::types::FunctionType, dialect::DialectName, op::Op};

    #[test]
    fn retained_progress_graph_drops_with_its_exact_reservation_in_both_receipt_modes() {
        for observed in [false, true] {
            let mut context = Context::new();
            dialect_kernel::register_dialect(
                &mut context,
                &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
            )
            .unwrap();
            let signature = FunctionType::get(&context, vec![], vec![]);
            let function = FuncOp::new(
                &mut context,
                "progress_release".try_into().unwrap(),
                signature,
            );
            ReturnOp::new(&mut context)
                .get_operation()
                .insert_at_back(function.get_entry_block(&context), &context);
            let session = begin_production_pliron_pass_contract_session_v1(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
            )
            .unwrap();
            let census = session.input_census_v1();
            // Resource-only manager: no claim that the seven absent caches
            // or a report have been produced. Their reservations are zero.
            let mut analyses = PlironAnalysisManagerV1::new(&function);
            let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
            let bound = preflight_progress_graph_resource_upper_bound_v2(census, hard).unwrap();
            let mut receipt =
                invocation_receipt_v1::InvocationReceiptV1::new(Default::default(), hard).unwrap();
            let graph = with_invocation_phase_v1(
                observed.then_some(&mut receipt),
                ProductionAnalysisResourcePhaseV1::Progress,
                0,
                |observer| {
                    retain_cache_with_observation_v1(
                        &mut analyses,
                        ProductionAnalysisResourcePhaseV1::Progress,
                        bound,
                        observer,
                    )?;
                    Ok((
                        PreparedProgressGraphV2::new(&context, &function, observer),
                        Some(bound),
                    ))
                },
            )
            .unwrap();
            assert_eq!(analyses.resource_upper_bound(), bound);
            let empty = ProductionAnalysisResourceUpperBoundV1::default();
            let reservations = ExclusiveAnalysisCacheReservationsV1 {
                sparse: empty,
                presburger: empty,
                execution_layout: empty,
                invocation_trace: empty,
                provenance: empty,
                simt: empty,
                memory_order: empty,
            };
            let returned = finish_exclusive_analysis_caches_with_progress_v2(
                analyses,
                reservations,
                Some((graph, bound)),
                observed.then_some(&mut receipt),
            )
            .unwrap();
            assert_eq!(returned.retained_storage_upper_bound(), 0);
            assert_eq!(returned.work_upper_bound(), bound.work_upper_bound());
            assert_eq!(
                returned.peak_storage_upper_bound(),
                bound.peak_storage_upper_bound()
            );
            if observed {
                assert_eq!(receipt.complete(), Ok(returned));
            }
        }
    }
}
