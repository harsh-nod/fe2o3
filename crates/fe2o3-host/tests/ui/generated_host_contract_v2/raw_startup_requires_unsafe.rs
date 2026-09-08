use fe2o3_host::__generated::{
    CompilerGeneratedKernelExpectationRosterEntryV1, CompilerGeneratedKernelExpectationRosterV1,
};

#[derive(Debug)]
struct Roster;

impl CompilerGeneratedKernelExpectationRosterV1 for Roster {
    const ENTRIES: &'static [CompilerGeneratedKernelExpectationRosterEntryV1] = &[];
}

fn main() {
    let _ = fe2o3_host::__generated::authenticate_inherited_worker_v3_capability_application_v1::<
        Roster,
    >();
}
