use fe2o3_host::JoinedExactLdsGemmSlice1V1;

fn require_clone<T: Clone>() {}
fn require_copy<T: Copy>() {}

fn joined_is_linear() {
    require_clone::<JoinedExactLdsGemmSlice1V1<'static, 'static, 'static>>();
    require_copy::<JoinedExactLdsGemmSlice1V1<'static, 'static, 'static>>();
}

fn main() {}
