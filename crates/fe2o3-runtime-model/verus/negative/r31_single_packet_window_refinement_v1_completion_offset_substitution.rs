use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)] pub enum PhaseV1 { Published, Completed }

#[derive(PartialEq, Eq)] pub struct RequestV1 {
    pub transfer_id: nat,
    pub host_offset: nat,
    pub device_offset: nat,
    pub copy_bytes: nat,
}

#[derive(PartialEq, Eq)] pub struct CompletionV1 {
    pub transfer_id: nat,
    pub host_offset: nat,
    pub device_offset: nat,
    pub copy_bytes: nat,
    pub packet_count: nat,
}

pub struct StateV1 {
    pub phase: PhaseV1,
    pub request: RequestV1,
    pub completion: Option<CompletionV1>,
}

// Mutation of the positive completion transition: single completion custody is
// normalized with host and device offsets exchanged.
pub open spec fn mutated_complete_with_swapped_offsets_v1(state: StateV1) -> StateV1 {
    if state.phase != PhaseV1::Published { state } else {
        StateV1 {
            phase: PhaseV1::Completed,
            completion: Some(CompletionV1 {
                transfer_id: state.request.transfer_id,
                host_offset: state.request.device_offset,
                device_offset: state.request.host_offset,
                copy_bytes: state.request.copy_bytes,
                packet_count: 1,
            }),
            ..state
        }
    }
}

pub proof fn mutated_completion_projection_retains_exact_offsets_v1(state: StateV1)
    requires state.phase == PhaseV1::Published,
        state.request.host_offset != state.request.device_offset,
    ensures {
        let post = mutated_complete_with_swapped_offsets_v1(state);
        &&& post.phase == PhaseV1::Completed
        &&& post.completion.unwrap().host_offset == state.request.host_offset
        &&& post.completion.unwrap().device_offset == state.request.device_offset
        &&& post.completion.unwrap().copy_bytes == state.request.copy_bytes
        &&& post.completion.unwrap().packet_count == 1
    }, {}

}
