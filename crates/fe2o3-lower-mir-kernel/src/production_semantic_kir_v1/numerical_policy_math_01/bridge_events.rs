fn math_bridge_events_v1(
    result: ProductionSemanticMathBridgeResultV1,
    events: &[(u32, SsaResolvedEventV1)],
) -> Result<(SsaValueV1, SsaValueV1, SsaValueV1, SsaValueV1), ProductionSemanticKirErrorV1> {
    let mut found = None;
    for window in events.windows(6) {
        let [
            (
                _,
                SsaResolvedEventV1::Use {
                    variable: receiver,
                    value: receiver_value,
                },
            ),
            (
                _,
                SsaResolvedEventV1::Use {
                    variable: current,
                    value: current_value,
                },
            ),
            (
                _,
                SsaResolvedEventV1::Define {
                    variable: returned,
                    value: returned_value,
                },
            ),
            (
                _,
                SsaResolvedEventV1::Use {
                    variable: used,
                    value: used_value,
                },
            ),
            (
                _,
                SsaResolvedEventV1::Kill {
                    variable: killed,
                    previous: Some(previous),
                },
            ),
            (
                _,
                SsaResolvedEventV1::Define {
                    variable: destination,
                    value: destination_value,
                },
            ),
        ] = window
        else {
            continue;
        };
        if receiver.get() == result.receiver().index()
            && current.get() == result.current().index()
            && returned.get() == result.return_local().index()
            && returned == used
            && returned == killed
            && returned_value == used_value
            && returned_value == previous
            && destination.get() == result.destination().index()
        {
            if found
                .replace((
                    *receiver_value,
                    *current_value,
                    *returned_value,
                    *destination_value,
                ))
                .is_some()
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
    }
    found.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
}
