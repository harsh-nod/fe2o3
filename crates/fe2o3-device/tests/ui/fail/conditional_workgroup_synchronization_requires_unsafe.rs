use fe2o3_device::Workgroup;

fn conditional_synchronize(group: &Workgroup<'_>, participates: bool) {
    if participates {
        group.synchronize();
    }
}

fn main() {}
