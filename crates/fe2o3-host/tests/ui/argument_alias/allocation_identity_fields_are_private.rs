use fe2o3_host::AllocationIdentity;

fn forge_identity() -> AllocationIdentity {
    AllocationIdentity {
        context: 1,
        allocation: 0x1000,
    }
}

fn main() {
    let _ = forge_identity();
}
