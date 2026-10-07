macro_rules! dependency_source_output_reserve_body {
    ($syntax:ident, $n:ident) => {
        dependency_source_output_reserve_body!(@annotated $syntax, $n,
            dependency_source_output_result, reservation, [])
    };
    (@annotated $syntax:ident, $n:ident, $finish:ident, $reservation:ident, [$($observed:tt)*]) => {
        $syntax!({
            let mut output = Vec::<Gfx942ComputeDependencyEventV1>::new();
            let $reservation = output.try_reserve_exact($n);
            $($observed)*
            match $reservation {
                Ok(()) => $finish!(Ok(output)),
                Err(_error) => $finish!(Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                    ComputeAqlQueueSessionErrorV1::Contract("dependency source event output allocation"),
                ))),
            }
        })
    };
}

macro_rules! dependency_source_output_result {
    ($result:expr) => {
        $result
    };
}

macro_rules! dependency_source_output_pack_body {
    ($syntax:ident, $output:ident, $events:ident, $lane:ident) => {
        dependency_source_output_pack_body!(@annotated $syntax, $output, $events, $lane,
            pending, event, [], [], [], [], [])
    };
    (@annotated $syntax:ident, $output:ident, $events:ident, $lane:ident, $pending:ident, $event:ident,
        [$($start:tt)*], [$($invariant:tt)*], [$($peek:tt)*], [$($step:tt)*], [$($finished:tt)*]) => {
        $syntax!({
            $($start)*
            let mut $pending = $events.into_iter();
            loop $($invariant)* {
                $($peek)*
                let Some($event) = $pending.next() else { $($finished)* break; };
                $output.push(Gfx942ComputeDependencyEventV1 { lane: $lane, event: $event });
                $($step)*
            }
            $output
        })
    };
}
