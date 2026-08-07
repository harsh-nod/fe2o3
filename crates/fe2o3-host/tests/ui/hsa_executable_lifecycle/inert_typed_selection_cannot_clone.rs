use fe2o3_host::InertLoadedWorkerV2KernelSelectionV1;

fn duplicate<K>(selection: InertLoadedWorkerV2KernelSelectionV1<'_, K>) {
    let _duplicate = selection.clone();
}

fn main() {}
