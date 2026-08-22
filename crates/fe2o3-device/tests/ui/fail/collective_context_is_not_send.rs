use fe2o3_device::Gfx942Collectives;

fn require_send<T: Send>() {}

fn main() {
    require_send::<Gfx942Collectives>();
}
