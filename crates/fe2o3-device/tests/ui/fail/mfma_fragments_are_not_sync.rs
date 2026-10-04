use fe2o3_device::{Bf16MfmaAFragment, F32AccumulatorFragment};

fn requires_sync<T: Sync>() {}

fn main() {
    requires_sync::<Bf16MfmaAFragment<'static>>();
    requires_sync::<F32AccumulatorFragment<'static>>();
}
