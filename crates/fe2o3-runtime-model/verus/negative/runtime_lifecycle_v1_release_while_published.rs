use vstd::prelude::*;

verus! {

pub struct MutatedStateV1 {
    pub published: bool,
    pub mapping_live: bool,
}

pub open spec fn no_early_release_v1(state: MutatedStateV1) -> bool {
    state.published ==> state.mapping_live
}

pub open spec fn mutated_release_while_published_v1(state: MutatedStateV1) -> MutatedStateV1 {
    MutatedStateV1 { published: state.published, mapping_live: false }
}

pub proof fn mutated_release_while_published_is_safe_v1(state: MutatedStateV1)
    requires
        no_early_release_v1(state),
        state.published,
    ensures
        no_early_release_v1(mutated_release_while_published_v1(state)),
{
}

} // verus!
