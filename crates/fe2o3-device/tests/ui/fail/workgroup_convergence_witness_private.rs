use core::marker::PhantomData;
use fe2o3_device::{Workgroup, WorkgroupConvergence};

fn forge<'group, 'invocation>(
    workgroup: &'group Workgroup<'invocation>,
) -> WorkgroupConvergence<'group, 'invocation> {
    WorkgroupConvergence {
        _workgroup: workgroup,
        _not_send_sync: PhantomData,
    }
}

fn main() {}
