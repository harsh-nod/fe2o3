use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct DeviceKeyV1 {
    pub physical: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct VmKeyV1 {
    pub device: DeviceKeyV1,
    pub id: nat,
}

#[derive(PartialEq, Eq)]
pub struct MappingKeyV1 {
    pub vm: VmKeyV1,
    pub id: nat,
}

#[derive(PartialEq, Eq)]
pub enum DispatchStateV1 {
    Prepared,
    Published,
    Ambiguous,
    Terminal,
}

pub struct MappingRecordV1 {
    pub key: MappingKeyV1,
    pub live: bool,
}

pub struct DispatchRecordV1 {
    pub vm: VmKeyV1,
    pub resource_index: nat,
    pub resource_key: MappingKeyV1,
    pub state: DispatchStateV1,
}

pub struct RuntimeStateV1 {
    pub mappings: Seq<MappingRecordV1>,
    pub dispatches: Seq<DispatchRecordV1>,
}

pub open spec fn retains_resources_v1(state: DispatchStateV1) -> bool {
    match state {
        DispatchStateV1::Prepared => true,
        DispatchStateV1::Published => true,
        DispatchStateV1::Ambiguous => true,
        DispatchStateV1::Terminal => false,
    }
}

pub open spec fn bindings_well_formed_v1(state: RuntimeStateV1) -> bool {
    forall |d: int| 0 <= d < state.dispatches.len() ==> {
        let dispatch = state.dispatches[d];
        &&& #[trigger] state.dispatches[d].resource_index < state.mappings.len()
        &&& state.mappings[dispatch.resource_index as int].key == dispatch.resource_key
        &&& dispatch.resource_key.vm == dispatch.vm
    }
}

pub open spec fn no_early_release_v1(state: RuntimeStateV1) -> bool {
    forall |d: int| 0 <= d < state.dispatches.len()
        && #[trigger] retains_resources_v1(state.dispatches[d].state)
        ==> state.mappings[state.dispatches[d].resource_index as int].live
}

pub open spec fn runtime_invariant_v1(state: RuntimeStateV1) -> bool {
    bindings_well_formed_v1(state) && no_early_release_v1(state)
}

pub open spec fn can_release_mapping_v1(state: RuntimeStateV1, mapping_index: nat) -> bool {
    &&& mapping_index < state.mappings.len()
    &&& forall |d: int| 0 <= d < state.dispatches.len()
        && #[trigger] retains_resources_v1(state.dispatches[d].state)
        ==> state.dispatches[d].resource_index != mapping_index
}

pub open spec fn release_mapping_v1(
    state: RuntimeStateV1,
    mapping_index: nat,
) -> RuntimeStateV1
    recommends mapping_index < state.mappings.len(),
{
    let old = state.mappings[mapping_index as int];
    RuntimeStateV1 {
        mappings: state.mappings.update(
            mapping_index as int,
            MappingRecordV1 { key: old.key, live: false },
        ),
        dispatches: state.dispatches,
    }
}

pub proof fn retained_dispatch_is_bound_to_exact_device_generation_v1(
    state: RuntimeStateV1,
    dispatch_index: int,
)
    requires
        runtime_invariant_v1(state),
        0 <= dispatch_index < state.dispatches.len(),
        retains_resources_v1(state.dispatches[dispatch_index].state),
    ensures
        state.dispatches[dispatch_index].resource_key.vm
            == state.dispatches[dispatch_index].vm,
        state.mappings[state.dispatches[dispatch_index].resource_index as int].key
            == state.dispatches[dispatch_index].resource_key,
        state.mappings[state.dispatches[dispatch_index].resource_index as int].live,
{
}

pub proof fn legal_mapping_release_preserves_runtime_invariant_v1(
    state: RuntimeStateV1,
    mapping_index: nat,
)
    requires
        runtime_invariant_v1(state),
        can_release_mapping_v1(state, mapping_index),
    ensures
        runtime_invariant_v1(release_mapping_v1(state, mapping_index)),
{
    let next = release_mapping_v1(state, mapping_index);
    assert(bindings_well_formed_v1(next)) by {
        assert forall |d: int| 0 <= d < next.dispatches.len() implies {
            let dispatch = next.dispatches[d];
            &&& #[trigger] next.dispatches[d].resource_index < next.mappings.len()
            &&& next.mappings[dispatch.resource_index as int].key == dispatch.resource_key
            &&& dispatch.resource_key.vm == dispatch.vm
        } by {
            let dispatch = state.dispatches[d];
            assert(dispatch.resource_index < state.mappings.len());
            assert(state.mappings[dispatch.resource_index as int].key == dispatch.resource_key);
            assert(dispatch.resource_key.vm == dispatch.vm);
            if dispatch.resource_index == mapping_index {
                assert(next.mappings[dispatch.resource_index as int].key
                    == state.mappings[dispatch.resource_index as int].key);
            } else {
                assert(next.mappings[dispatch.resource_index as int]
                    == state.mappings[dispatch.resource_index as int]);
            }
        }
    }
    assert(no_early_release_v1(next)) by {
        assert forall |d: int| 0 <= d < next.dispatches.len()
            && #[trigger] retains_resources_v1(next.dispatches[d].state)
            implies next.mappings[next.dispatches[d].resource_index as int].live by {
            let dispatch = state.dispatches[d];
            assert(dispatch.resource_index != mapping_index);
            assert(state.mappings[dispatch.resource_index as int].live);
            assert(next.mappings[dispatch.resource_index as int]
                == state.mappings[dispatch.resource_index as int]);
        }
    }
}

} // verus!
