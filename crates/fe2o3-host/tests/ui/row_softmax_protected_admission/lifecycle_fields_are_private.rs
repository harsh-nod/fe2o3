use fe2o3_host::{
    CompletedProtectedRowSoftmaxV1, JoinedProtectedRowSoftmaxV1,
    LoadedProtectedRowSoftmaxV1, ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1,
};

fn extract_joined(value: JoinedProtectedRowSoftmaxV1<'_, '_>) {
    let JoinedProtectedRowSoftmaxV1 { token, host } = value;
    let _ = (token, host);
}

fn extract_loaded<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1>(
    value: LoadedProtectedRowSoftmaxV1<'_, '_, A>,
) {
    let LoadedProtectedRowSoftmaxV1 { state } = value;
    let _ = state;
}

fn extract_completed<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1>(
    value: CompletedProtectedRowSoftmaxV1<A>,
) {
    let CompletedProtectedRowSoftmaxV1 { state, receipt } = value;
    let _ = (state, receipt);
}

fn main() {}
