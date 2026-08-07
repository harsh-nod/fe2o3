use fe2o3_host::InertLoadedWorkerV2KernelSelectionV1;

fn extract_executable<K>(selection: InertLoadedWorkerV2KernelSelectionV1<'_, K>) {
    let _executable = selection.executable_object;
}

fn main() {}
