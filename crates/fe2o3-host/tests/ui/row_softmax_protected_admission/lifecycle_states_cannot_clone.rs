use fe2o3_host::{
    CompletedProtectedRowSoftmaxV1, JoinedProtectedRowSoftmaxV1,
    LoadedProtectedRowSoftmaxV1, ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1,
};

fn joined(value: JoinedProtectedRowSoftmaxV1<'_, '_>) {
    let _ = value.clone();
}

fn loaded<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1>(
    value: LoadedProtectedRowSoftmaxV1<'_, '_, A>,
) {
    let _ = value.clone();
}

fn completed<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1>(
    value: CompletedProtectedRowSoftmaxV1<A>,
) {
    let _ = value.clone();
}

fn main() {}
