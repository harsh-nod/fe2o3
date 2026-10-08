//! One inline original-device slot; Ready and Cold custody cannot coexist.

use super::{CheckedGfx942XnackMinusDevice, ColdDeviceV1};

pub(in crate::kfd_backend) enum DeviceCustodyV1<
    Ready = CheckedGfx942XnackMinusDevice,
    Cold = ColdDeviceV1,
> {
    Vacant,
    Ready(Ready),
    Cold(Cold),
}

impl<Ready, Cold> DeviceCustodyV1<Ready, Cold> {
    pub(in crate::kfd_backend) fn from_ready(owner: Option<Ready>) -> Self {
        match owner {
            Some(owner) => Self::Ready(owner),
            None => Self::Vacant,
        }
    }

    pub(in crate::kfd_backend) fn as_ref(&self) -> Option<&Ready> {
        match self {
            Self::Ready(owner) => Some(owner),
            Self::Vacant | Self::Cold(_) => None,
        }
    }

    pub(in crate::kfd_backend) fn as_mut(&mut self) -> Option<&mut Ready> {
        match self {
            Self::Ready(owner) => Some(owner),
            Self::Vacant | Self::Cold(_) => None,
        }
    }

    pub(in crate::kfd_backend) fn is_some(&self) -> bool {
        self.as_ref().is_some()
    }

    pub(in crate::kfd_backend) fn is_none(&self) -> bool {
        self.as_ref().is_none()
    }

    pub(in crate::kfd_backend) fn cold(&self) -> Option<&Cold> {
        match self {
            Self::Cold(owner) => Some(owner),
            Self::Vacant | Self::Ready(_) => None,
        }
    }

    pub(in crate::kfd_backend) fn take(&mut self) -> Option<Ready> {
        match std::mem::replace(self, Self::Vacant) {
            Self::Ready(owner) => Some(owner),
            other => {
                *self = other;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Original<'a>(&'a Cell<usize>);

    impl Drop for Original<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn ready_take_does_not_remove_or_drop_original_cold_owner() {
        let drops = Cell::new(0);
        let mut slot: DeviceCustodyV1<Original<'_>, Original<'_>> =
            DeviceCustodyV1::Cold(Original(&drops));
        assert!(slot.is_none());
        assert!(!slot.is_some());
        assert!(slot.as_mut().is_none());
        assert!(slot.take().is_none());
        assert!(std::ptr::eq(slot.cold().unwrap().0, &drops));
        assert_eq!(drops.get(), 0);
        assert_eq!(drops.get(), 0);
        drop(slot);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn exact_ready_borrows_survive_until_actual_typed_transfer() {
        let drops = Cell::new(0);
        let mut slot: DeviceCustodyV1<Original<'_>, Original<'_>> =
            DeviceCustodyV1::from_ready(Some(Original(&drops)));
        assert!(slot.cold().is_none());
        assert!(std::ptr::eq(slot.as_ref().unwrap().0, &drops));
        assert!(std::ptr::eq(slot.as_mut().unwrap().0, &drops));
        let original = slot.take().unwrap();
        assert!(matches!(slot, DeviceCustodyV1::Vacant));
        assert_eq!(drops.get(), 0);
        slot = DeviceCustodyV1::Cold(original);
        assert!(slot.take().is_none());
        assert_eq!(drops.get(), 0);
        drop(slot);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn original_native_slot_uses_one_union_not_two_owner_storage_regions() {
        let original = std::mem::size_of::<CheckedGfx942XnackMinusDevice>();
        let cold = std::mem::size_of::<ColdDeviceV1>();
        let slot = std::mem::size_of::<DeviceCustodyV1>();
        let alignment = std::mem::align_of::<DeviceCustodyV1>();
        assert!(slot <= original.max(cold) + alignment);
        assert!(slot < original + cold);
        assert!(std::mem::size_of::<super::super::super::KfdRuntimeBackendV1>() <= 64 * 1024);
    }
}
