resource_domain_declarations_v1! {
pub const R75_RESOURCE_DOMAIN_LEVELS_V1: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R75ResourceDomainErrorV1 {
    Invariant,
    InvalidMemberCount,
    RecordCapacity,
    GenerationExhausted,
    Capacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R75ResourceDomainFactsV1 {
    pub used: R67ResourceVectorV1,
    pub capacity: R67ResourceVectorV1,
    pub counts: [usize; 3],
    pub record_limit: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R75ResourceDomainPlanV1 {
    pub next_used: [R67ResourceVectorV1; R75_RESOURCE_DOMAIN_LEVELS_V1],
    pub next_reserved: [usize; R75_RESOURCE_DOMAIN_LEVELS_V1],
    pub next_owner: u64,
}
}
