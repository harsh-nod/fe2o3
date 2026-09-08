use fe2o3_device::capability_memory::{MemoryRole, ReadAccess, SharedAliases};

struct ForgedRole;

impl MemoryRole for ForgedRole {
    type Access = ReadAccess;
    type Aliasing = SharedAliases;
    type Physical<'kernel, T: 'kernel> = &'kernel [T];
}

fn main() {}
