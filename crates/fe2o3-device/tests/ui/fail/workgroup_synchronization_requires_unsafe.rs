use fe2o3_device::Workgroup;

fn synchronize(group: &Workgroup<'_>) {
    group.synchronize();
}

fn main() {}
