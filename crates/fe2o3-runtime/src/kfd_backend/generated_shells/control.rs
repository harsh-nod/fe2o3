use super::*;
use crate::generated_source::GeneratedProfileV1;

pub(in crate::kfd_backend) enum GeneratedControlV1 {
    Singleton(Option<Gfx942FixedDispatchPacketV1>),
    Cohort3([Option<Gfx942FixedDispatchPacketV1>; 3]),
}

impl GeneratedControlV1 {
    pub(in crate::kfd_backend) fn empty(profile: GeneratedProfileV1) -> Self {
        match profile {
            GeneratedProfileV1::Singleton => Self::Singleton(None),
            GeneratedProfileV1::NativeFillCohort3 => Self::Cohort3([None, None, None]),
        }
    }

    pub(in crate::kfd_backend) fn profile(&self) -> GeneratedProfileV1 {
        match self {
            Self::Singleton(_) => GeneratedProfileV1::Singleton,
            Self::Cohort3(_) => GeneratedProfileV1::NativeFillCohort3,
        }
    }

    pub(in crate::kfd_backend) fn is_some(&self) -> bool {
        match self {
            Self::Singleton(packet) => packet.is_some(),
            Self::Cohort3(packets) => packets.iter().all(Option::is_some),
        }
    }

    pub(in crate::kfd_backend) fn is_none(&self) -> bool {
        match self {
            Self::Singleton(packet) => packet.is_none(),
            Self::Cohort3(packets) => packets.iter().all(Option::is_none),
        }
    }

    pub(in crate::kfd_backend) fn take(&mut self) -> Option<Gfx942FixedDispatchPacketV1> {
        match self {
            Self::Singleton(packet) => packet.take(),
            Self::Cohort3(_) => None,
        }
    }
}
