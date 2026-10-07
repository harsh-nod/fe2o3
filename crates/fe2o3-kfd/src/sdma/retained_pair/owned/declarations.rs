retained_pair_owned_declarations_v1! {
    struct Parts<Q, S, D> {
        queue: Q,
        source: S,
        destination: D,
    }

    struct Owned<C: Custody> {
        context: Option<C>,
    }

    struct Failure<C: Custody, E> {
        owner: Owned<C>,
        error: E,
        entry_refusal: bool,
    }
}
