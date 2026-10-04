use fe2o3_device::DynamicLds;

fn read<'a>(view: &'a DynamicLds<'a, u32>) -> &'a [u32] {
    view.as_slice()
}

fn main() {}
