use fe2o3_pliron_scalar_add_v1::{ObservedRepositoryScalarAddV1, RepositoryScalarAddProfileV1};

fn self_approve(observed: ObservedRepositoryScalarAddV1) -> RepositoryScalarAddProfileV1 {
    observed.into()
}

fn main() {}
