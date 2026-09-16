use fe2o3_device::{KernelLaunch, KernelTarget, SynchronizationEpoch};

struct ForgedTarget;
struct ForgedLaunch;
struct ForgedEpoch;

impl KernelTarget for ForgedTarget {}
impl KernelLaunch for ForgedLaunch {}
impl SynchronizationEpoch for ForgedEpoch {}

fn main() {}
