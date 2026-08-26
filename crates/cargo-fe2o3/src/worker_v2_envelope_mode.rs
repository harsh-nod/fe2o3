#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerV2EnvelopeModeV1 {
    NonAuthoritative,
    Required,
}

impl WorkerV2EnvelopeModeV1 {
    pub(crate) const fn is_required(self) -> bool {
        matches!(self, Self::Required)
    }

    #[cfg(feature = "qualification-oracles-test-only")]
    pub(crate) const fn grants_load_authority(self) -> bool {
        false
    }

    #[cfg(feature = "qualification-oracles-test-only")]
    pub(crate) const fn grants_launch_authority(self) -> bool {
        false
    }
}
