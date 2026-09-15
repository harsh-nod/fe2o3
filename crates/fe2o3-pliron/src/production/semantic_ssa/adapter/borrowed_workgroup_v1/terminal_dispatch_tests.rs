use super::*;

// These fixtures exercise reference transparency, not source/type admission.
// Unregistered calls must never receive authority from this classifier.
fn with_intermediate_calls(arguments: &[Vec<SemanticOperandV1>]) -> SemanticFunctionDeclV1 {
    assert!(arguments.len() <= 64);
    let template = direct_function(direct_statements(), false);
    let mut blocks = Vec::new();
    for (index, arguments) in arguments.iter().enumerate() {
        blocks.push(block(
            index as u8,
            if index == 0 {
                direct_statements()
            } else {
                vec![]
            },
            call(1, arguments.clone(), place(4, 7), index as u32 + 1),
        ));
    }
    let last = arguments.len() as u8;
    blocks.push(block(
        last,
        if last == 0 {
            direct_statements()
        } else {
            vec![]
        },
        call(
            0,
            vec![SemanticOperandV1::Copy(place(3, 5))],
            place(4, 7),
            u32::from(last) + 1,
        ),
    ));
    blocks.push(block(last + 1, vec![], SemanticTerminatorKindV1::Return));
    function(
        175,
        template.abi().clone(),
        template.locals().to_vec(),
        blocks,
    )
}

fn scalar() -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(4, 7))
}

fn projected(local: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(4)).unwrap()],
            ty(4),
        )
        .unwrap(),
    )
}

#[test]
fn lazy_terminal_classification_keeps_scalar_calls_and_exact_work_failure() {
    let callable = borrowed_callable(5, true);
    let expected = BTreeSet::from([SemanticTransparentBorrowSiteV1 {
        block: 0,
        statement: 1,
    }]);
    for count in [0, 1, 16, 64] {
        for arguments in [vec![], vec![scalar(), scalar(), scalar()]] {
            let body = with_intermediate_calls(&vec![arguments; count]);
            let run = |limit| sites(&body, std::slice::from_ref(&callable), &[], limit, None);
            assert_eq!(run(MAX_FLOW_WORK).unwrap(), expected);
            let (mut low, mut high) = (0, MAX_FLOW_WORK);
            while low < high {
                let mid = low + (high - low) / 2;
                if run(mid).is_ok() {
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }
            assert_eq!(run(low).unwrap(), expected);
            assert!(matches!(
                run(low - 1).map_err(flow_work_profile_v1::original_error_for_test),
                Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                    resource: SsaPlannerResourceV1::WorkUnits, required, limit,
                }) if required == low && limit == low - 1
            ));
        }
    }
}

#[test]
fn lazy_terminal_classification_never_hides_a_projected_reference() {
    let callable = borrowed_callable(5, true);
    for local in [2, 3] {
        for position in 0..4 {
            let mut arguments = vec![scalar(); 4];
            arguments[position] = projected(local);
            let body = with_intermediate_calls(&[arguments]);
            assert!(
                direct_sites(&body, std::slice::from_ref(&callable)).is_empty(),
                "projected local {local}, argument {position}"
            );
        }
    }
}

#[test]
fn lazy_terminal_classification_checks_late_copy_move_and_mixed_escapes() {
    let callable = borrowed_callable(5, true);
    for local in [2, 3] {
        for position in 0..4 {
            for moved in [false, true] {
                let mut arguments = vec![scalar(); 4];
                arguments[position] = if moved {
                    SemanticOperandV1::Move(place(local, 5))
                } else {
                    SemanticOperandV1::Copy(place(local, 5))
                };
                for add_projection in [false, true] {
                    let mut arguments = arguments.clone();
                    if add_projection {
                        arguments.push(projected(local));
                    }
                    let body = with_intermediate_calls(&[arguments]);
                    assert!(
                        direct_sites(&body, std::slice::from_ref(&callable)).is_empty(),
                        "local {local}, argument {position}, move {moved}, projected {add_projection}"
                    );
                }
            }
        }
    }
    assert!(direct_sites(&direct_function(direct_statements(), true), &[callable]).is_empty());
}
