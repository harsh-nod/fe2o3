use fe2o3_device::Bf16MfmaAFragment;

fn require_copy<T: Copy>() {}

fn main() {
    require_copy::<Bf16MfmaAFragment<'static>>();
}
