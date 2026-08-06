use fe2o3_host::{LoadedKernel, ValidatedPublishedDirectLinkSelectionV1};

struct Kernel;

fn forge(token: ValidatedPublishedDirectLinkSelectionV1) -> LoadedKernel<Kernel> {
    token.into()
}

fn main() {}
