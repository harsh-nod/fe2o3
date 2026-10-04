use super::*;
use crate::production::semantic_ssa::adapter::emission_v1::emit_terminator_events_with_buffer_v1;

#[test]
fn call_address_output_failure_stops_before_argument_moves() {
    let scalar = SemanticTypeIdV1::from_index(1);
    for indirect in [false, true] {
        let destination = if indirect {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(1),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, scalar)
                        .unwrap(),
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                        scalar,
                    )
                    .unwrap(),
                ],
                scalar,
            )
            .unwrap()
        } else {
            indexed(1, 2)
        };
        let call = test_call(
            0,
            vec![
                SemanticOperandV1::Move(test_scalar_place(1)),
                SemanticOperandV1::Move(test_scalar_place(2)),
            ],
            Some(SemanticCallDestinationV1::new(
                destination,
                test_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
        );
        let mut events = CountEvents(if indirect { usize::MAX - 1 } else { usize::MAX });
        let mut trace = Trace::default();
        assert_eq!(
            emit_terminator_events_with_buffer_v1(
                &call,
                None,
                Site::Terminator { block: 0 },
                &mut events,
                &mut trace,
            ),
            Err(EmissionError::Output(Overflow::Events))
        );
        assert_eq!(events, CountEvents(usize::MAX));
        let site = Site::Terminator { block: 0 };
        let last = row(
            site,
            Operand::CallDestinationAddress,
            EventRole::ProjectionIndexUse(1),
            usize::MAX,
            used(2),
        );
        if indirect {
            assert_eq!(
                trace.events,
                [
                    row(
                        site,
                        Operand::CallDestinationAddress,
                        EventRole::BaseUse,
                        usize::MAX - 1,
                        used(1)
                    ),
                    last,
                ]
            );
        } else {
            assert_eq!(trace.events, [last]);
        }
        assert_eq!(
            trace
                .visits
                .iter()
                .filter(|(visit, _)| *visit == Visit::Projection)
                .count(),
            4
        );
        assert!(
            trace
                .events
                .iter()
                .all(|event| event.operand == Operand::CallDestinationAddress)
        );
        assert!(
            !trace
                .visits
                .iter()
                .any(|(visit, _)| *visit == Visit::Operand)
        );
        assert!(trace.successors.is_empty());
        assert!(trace.edge_definitions.is_empty());
    }
}
