use fe2o3_host::{CompletedMoeTop2V1, ReviewedMoeTop2V1RuntimeAdapterV1};

fn unload_twice<A: ReviewedMoeTop2V1RuntimeAdapterV1>(
    completed: CompletedMoeTop2V1<A>,
) {
    let _first = completed.unload();
    let _second = completed.unload();
}

fn main() {}
