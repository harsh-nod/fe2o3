use fe2o3_device::DeviceMath;

fn device_only(value: f32) -> f32 {
    let math = DeviceMath::current();
    math.sqrt_f32(value)
}

fn main() {}
