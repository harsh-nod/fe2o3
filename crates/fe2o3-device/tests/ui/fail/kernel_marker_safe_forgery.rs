use fe2o3_device::KernelMarkerV1;

struct Forged;

static REGISTRATION: (u64, u16, u16, &str, &str, fn()) = (0, 1, 1, "forged", "forged", forged);

fn forged() {}

impl KernelMarkerV1 for Forged {
    type Function = fn();
    type Registration = (u64, u16, u16, &'static str, &'static str, fn());

    const LOGICAL_NAME: &'static str = "forged";
    const EXPORT_NAME: &'static str = "forged";
    const FUNCTION: Self::Function = forged;
    const REGISTRATION: &'static Self::Registration = &REGISTRATION;
}

fn main() {}
