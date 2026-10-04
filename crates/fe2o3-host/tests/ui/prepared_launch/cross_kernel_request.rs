use fe2o3_host::{KernelBrand, ObservedContext, UntrustedLaunchRequest};

struct KernelA;
struct KernelB;

fn prepare_a(
    brand: &KernelBrand<KernelA>,
    context: &ObservedContext,
    request: UntrustedLaunchRequest<KernelB>,
) {
    let _ = brand.prepare(context, request);
}

fn main() {}
