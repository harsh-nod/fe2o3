use super::*;

#[test]
fn reusable_lds_lowerer_checks_frame_kills_without_dropping_them() {
    let owner = owner();
    let plan = owner
        .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let row = plan.defined_reusable_lds_results()[0];
    let input = plan
        .plan()
        .resolved_events(fe2o3_mir_model::SsaBlockIdV1::new(
            row.parameter_block().index(),
        ))
        .unwrap();
    let output = plan
        .plan()
        .resolved_events(fe2o3_mir_model::SsaBlockIdV1::new(
            row.return_block().index(),
        ))
        .unwrap();
    assert_eq!(input.len(), 5);
    assert_eq!(output.len(), 8);
    let (allocated, parameter) = input_events(row, input).unwrap();
    for returning in [false, true] {
        for mutation in 0..3 {
            let mut events = if returning {
                output.to_vec()
            } else {
                input.to_vec()
            };
            let index = if returning { 6 } else { 0 };
            match mutation {
                0 => events.swap(index, index + 1),
                1 => {
                    let SsaResolvedEventV1::Kill { variable, .. } = events[index].1 else {
                        panic!("frame kill")
                    };
                    events[index].1 = SsaResolvedEventV1::Kill {
                        variable,
                        previous: Some(allocated),
                    };
                }
                2 => {
                    events[index].1 = SsaResolvedEventV1::Kill {
                        variable: fe2o3_mir_model::SsaVariableIdV1::new(row.allocation().index()),
                        previous: None,
                    }
                }
                _ => unreachable!(),
            }
            let result = if returning {
                output_events(row, parameter, &events).map(|_| ())
            } else {
                input_events(row, &events).map(|_| ())
            };
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ),
                "returning={returning} mutation={mutation}"
            );
        }
    }
}
