use fe2o3_host::{LoadedKernel, ValidatedArtifactSelectionV1};

struct Kernel;

fn forge(validated: ValidatedArtifactSelectionV1) -> LoadedKernel<Kernel> {
    LoadedKernel::from_validated(validated)
}

fn main() {}
