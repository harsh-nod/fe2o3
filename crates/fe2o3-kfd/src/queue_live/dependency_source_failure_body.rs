macro_rules! native_dependency_source_cancel_body {
    ($syntax:ident, $session:ident, $identity:ident) => {
        $syntax!({
            $session
                .dispatch
                .as_mut()
                .expect("dependency source dispatch owner remains retained")
                .cancel_binding($identity)
        })
    };
}

macro_rules! dependency_source_recipe_cancel_call {
    ($recipe:ident, $session:ident, $identity:ident) => {
        $recipe.cancel($session, $identity)
    };
}

macro_rules! dependency_source_failure_body {
    ($syntax:ident, $session:ident, $recipe:ident, $identity:ident, $failure:ident) => {
        dependency_source_failure_body!(@annotated $syntax, $session, $recipe, $identity, $failure,
            dependency_source_recipe_cancel_call)
    };
    (@annotated $syntax:ident, $session:ident, $recipe:ident, $identity:ident, $failure:ident,
        $cancel:ident) => {
        $syntax!({
            match $failure {
                FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error) => {
                    if $cancel!($recipe, $session, $identity).is_err() {
                        FixedDispatchSubmissionFailureV1::Terminal(
                            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                Gfx942DispatchBindingErrorV1::StaleDispatchGeneration,
                            ),
                        )
                    } else {
                        FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error)
                    }
                }
                FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)
                | FixedDispatchSubmissionFailureV1::Terminal(error) => {
                    FixedDispatchSubmissionFailureV1::Terminal(error)
                }
            }
        })
    };
}
