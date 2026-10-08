use super::*;
use crate::generated_source::GeneratedProfileV1;

// Shell commit transfers original packets without allocating another owner.
#[allow(clippy::large_enum_variant)]
pub(in crate::kfd_backend) enum GeneratedControlV1 {
    Singleton(Option<Gfx942FixedDispatchPacketV1>),
    Cohort3([Option<Gfx942FixedDispatchPacketV1>; 3]),
    Registry4([Option<Gfx942FixedDispatchPacketV1>; 4]),
    Registry4Repeat2([Option<Gfx942FixedDispatchPacketV1>; 4]),
    Registry16([Option<Gfx942FixedDispatchPacketV1>; 16]),
    Arena1024(Option<crate::generated_source::arena1024::ArenaPacketsV1>),
    IndependentArena1024(Option<crate::generated_source::arena1024::ArenaPacketsV1>),
    IndependentArena2048(Option<crate::generated_source::arena1024::ArenaPacketsV1>),
    IndependentArenaMember,
}

impl GeneratedControlV1 {
    pub(in crate::kfd_backend) fn empty(profile: GeneratedProfileV1) -> Self {
        match profile {
            GeneratedProfileV1::Singleton => Self::Singleton(None),
            GeneratedProfileV1::NativeFillCohort3 => Self::Cohort3([None, None, None]),
            GeneratedProfileV1::NativeFillRegistry4 => Self::Registry4([None, None, None, None]),
            GeneratedProfileV1::NativeFillRegistry4Repeat2 => {
                Self::Registry4Repeat2([None, None, None, None])
            }
            GeneratedProfileV1::NativeFillRegistry16 => {
                Self::Registry16(core::array::from_fn(|_| None))
            }
            GeneratedProfileV1::NativeFillArena1024 => Self::Arena1024(None),
            GeneratedProfileV1::IndependentFillArena1024 => Self::IndependentArena1024(None),
            GeneratedProfileV1::IndependentFillArena2048 => Self::IndependentArena2048(None),
            GeneratedProfileV1::IndependentArenaMember => Self::IndependentArenaMember,
        }
    }

    pub(in crate::kfd_backend) fn profile(&self) -> GeneratedProfileV1 {
        match self {
            Self::Singleton(_) => GeneratedProfileV1::Singleton,
            Self::Cohort3(_) => GeneratedProfileV1::NativeFillCohort3,
            Self::Registry4(_) => GeneratedProfileV1::NativeFillRegistry4,
            Self::Registry4Repeat2(_) => GeneratedProfileV1::NativeFillRegistry4Repeat2,
            Self::Registry16(_) => GeneratedProfileV1::NativeFillRegistry16,
            Self::Arena1024(_) => GeneratedProfileV1::NativeFillArena1024,
            Self::IndependentArena1024(_) => GeneratedProfileV1::IndependentFillArena1024,
            Self::IndependentArena2048(_) => GeneratedProfileV1::IndependentFillArena2048,
            Self::IndependentArenaMember => GeneratedProfileV1::IndependentArenaMember,
        }
    }

    pub(in crate::kfd_backend) fn is_some(&self) -> bool {
        match self {
            Self::Singleton(packet) => packet.is_some(),
            Self::Cohort3(packets) => packets.iter().all(Option::is_some),
            Self::Registry4(packets) | Self::Registry4Repeat2(packets) => {
                packets.iter().all(Option::is_some)
            }
            Self::Registry16(packets) => packets.iter().all(Option::is_some),
            Self::Arena1024(packets)
            | Self::IndependentArena1024(packets)
            | Self::IndependentArena2048(packets) => packets
                .as_ref()
                .is_some_and(|packets| packets.matches(self.profile())),
            Self::IndependentArenaMember => false,
        }
    }

    pub(in crate::kfd_backend) fn is_none(&self) -> bool {
        match self {
            Self::Singleton(packet) => packet.is_none(),
            Self::Cohort3(packets) => packets.iter().all(Option::is_none),
            Self::Registry4(packets) | Self::Registry4Repeat2(packets) => {
                packets.iter().all(Option::is_none)
            }
            Self::Registry16(packets) => packets.iter().all(Option::is_none),
            Self::Arena1024(packets)
            | Self::IndependentArena1024(packets)
            | Self::IndependentArena2048(packets) => packets.is_none(),
            Self::IndependentArenaMember => true,
        }
    }

    pub(in crate::kfd_backend) fn take(&mut self) -> Option<Gfx942FixedDispatchPacketV1> {
        match self {
            Self::Singleton(packet) => packet.take(),
            Self::Cohort3(_)
            | Self::Registry4(_)
            | Self::Registry4Repeat2(_)
            | Self::Registry16(_)
            | Self::Arena1024(_)
            | Self::IndependentArena1024(_)
            | Self::IndependentArena2048(_)
            | Self::IndependentArenaMember => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry16_control_header_stays_within_the_fixed_inline_roster() {
        assert!(
            core::mem::size_of::<GeneratedControlV1>()
                <= 16 * core::mem::size_of::<Option<Gfx942FixedDispatchPacketV1>>() + 16
        );
    }
}
