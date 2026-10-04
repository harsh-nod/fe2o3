use vstd::prelude::*;
verus! {
pub struct AttachmentV1 { pub queue: nat, pub generation: nat }
pub open spec fn mutated_same_attachment_v1(left: AttachmentV1, right: AttachmentV1) -> bool {
    left.queue == right.queue
}
pub proof fn mutated_attachment_generation_substitution_is_rejected_v1()
    ensures !mutated_same_attachment_v1(
        AttachmentV1 { queue: 7, generation: 1 },
        AttachmentV1 { queue: 7, generation: 2 },
    ),
{}
}
