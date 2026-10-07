// Visibility differs only because the proof root is not the backend submodule.
macro_rules! retained_pair_routing_declarations {
    ($syntax:ident, $phase_visibility:vis) => {
        $syntax! {
            $phase_visibility enum BatchPhase<R, T, S, W> {
                Prepared(Vec<R>),
                Pending(Vec<T>),
                Ready,
                SubmitFailure(S),
                WaitFailure(W),
            }

            enum Advanced<P, C, E> {
                Unchanged(P),
                Waited(Result<C, E>),
            }
        }
    };
}
