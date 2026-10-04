use fe2o3_device::{Bf16MfmaAFragment, F32AccumulatorFragment};

fn requires_send<T: Send>() {}

fn main() {
    requires_send::<Bf16MfmaAFragment<'static>>();
    requires_send::<F32AccumulatorFragment<'static>>();
}
