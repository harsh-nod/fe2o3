use fe2o3_hsaco_finalize::{
    LinkInputKindClosureV1, LinkSymbolClosureV1, StagedFfiLinkPlanV1, WorkerOutputV1,
};

fn reduce_or_bind(staged: &StagedFfiLinkPlanV1, output: &WorkerOutputV1) {
    let _: &LinkInputKindClosureV1 = staged.input_kind_closure();
    let _: &LinkSymbolClosureV1 = staged.symbol_closure();
    let _ = staged.input_claims();
    let _ = staged.provider_binding_claims();
    let _ = staged.final_symbols_claim();
    let _ = staged.bind_worker_output_v1(output);
}

fn main() {}
