use fe2o3_device::Workgroup;

fn synchronize(group: &Workgroup<'_>) {
    group.assume_uniform().synchronize();
}

fn main() {}
