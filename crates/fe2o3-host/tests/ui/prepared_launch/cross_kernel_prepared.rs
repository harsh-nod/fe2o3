use fe2o3_host::PreparedLaunch;

struct KernelA;
struct KernelB;

fn launch_b(_: &PreparedLaunch<KernelB>) {}

fn cannot_launch_a_as_b(prepared: &PreparedLaunch<KernelA>) {
    launch_b(prepared);
}

fn main() {}
