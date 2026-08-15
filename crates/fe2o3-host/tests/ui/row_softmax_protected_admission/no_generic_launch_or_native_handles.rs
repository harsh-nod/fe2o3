use fe2o3_host::{
    CompletedProtectedRowSoftmaxV1, LoadedProtectedRowSoftmaxV1,
    ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1,
};

fn loaded<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1>(
    value: &LoadedProtectedRowSoftmaxV1<'_, '_, A>,
) {
    value.launch();
    let _ = value.native_executable_handle();
    let _ = value.native_kernel_handle();
}

fn completed<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1>(
    value: &CompletedProtectedRowSoftmaxV1<A>,
) {
    let _ = value.native_executable_handle();
    let _ = value.native_kernel_handle();
}

fn main() {}
