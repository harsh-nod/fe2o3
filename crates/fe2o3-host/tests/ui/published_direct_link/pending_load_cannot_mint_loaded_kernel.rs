use fe2o3_host::{LoadedKernel, PendingPublishedDirectLinkLoadAdmissionV1};

struct Kernel;

fn forge(pending: PendingPublishedDirectLinkLoadAdmissionV1) -> LoadedKernel<Kernel> {
    pending.into()
}

fn main() {}
