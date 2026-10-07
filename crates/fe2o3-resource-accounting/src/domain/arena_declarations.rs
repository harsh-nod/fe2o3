resource_domain_arena_declarations_v1! {
pub const MAX_RESOURCE_DOMAIN_DEPTH_V1: usize = 3;
pub const MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Key {
    slot: usize,
    generation: u64,
}

const ROOT: Key = Key { slot: 0, generation: 1 };

struct Node {
    key: Key,
    parent: Option<Key>,
    capacity: ResourceVectorV1,
    used: ResourceVectorV1,
    record_limit: usize,
    counts: [usize; 3],
    handles: usize,
    children: usize,
}
}
