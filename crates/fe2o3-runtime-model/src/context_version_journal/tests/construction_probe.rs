use crate::context_version_journal::construction::ConstructorAllocatorV1;
use alloc::vec::Vec;
use core::any::type_name;
use core::mem::size_of;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ReserveEvent {
    pub site: u8,
    pub requested: usize,
    pub width: usize,
    pub element: &'static str,
    pub succeeded: bool,
    pub pointer: usize,
    pub capacity: usize,
}

pub(crate) struct ReserveProbe<'a> {
    outcomes: &'a [bool],
    extra: usize,
    pub events: Vec<ReserveEvent>,
}

impl<'a> ReserveProbe<'a> {
    pub fn new(outcomes: &'a [bool], extra: usize) -> Self {
        Self {
            outcomes,
            extra,
            events: Vec::new(),
        }
    }

    pub fn assert_requests(&self, capacities: &[usize]) {
        assert_eq!(self.events.len(), capacities.len());
        for (index, (event, requested)) in self.events.iter().zip(capacities).enumerate() {
            assert_eq!(event.site as usize, index);
            assert_eq!(event.requested, *requested);
            assert_ne!(event.width, 0);
            if event.succeeded {
                assert!(event.capacity >= requested + self.extra);
            } else {
                assert_eq!(event.capacity, 0);
                assert_eq!(index + 1, self.events.len(), "stops at first failure");
            }
        }
    }

    pub fn assert_same_trace(&self, other: &Self) {
        assert_eq!(self.events.len(), other.events.len());
        for (left, right) in self.events.iter().zip(&other.events) {
            assert_eq!(left.site, right.site);
            assert_eq!(left.requested, right.requested);
            assert_eq!(left.width, right.width);
            assert_eq!(left.element, right.element);
            assert_eq!(left.succeeded, right.succeeded);
        }
    }

    pub fn assert_storage(&self, actual: &[(usize, usize)]) {
        assert_eq!(self.events.len(), actual.len());
        for (event, storage) in self.events.iter().zip(actual) {
            assert!(event.succeeded);
            assert_eq!((event.pointer, event.capacity), *storage);
        }
    }
}

impl ConstructorAllocatorV1 for ReserveProbe<'_> {
    fn reserve<T>(&mut self, site: u8, capacity: usize, storage: &mut Vec<T>) -> bool {
        assert!(storage.is_empty());
        assert_eq!(storage.capacity(), 0);
        let succeeded = self
            .outcomes
            .get(self.events.len())
            .copied()
            .unwrap_or(false);
        if succeeded {
            storage
                .try_reserve_exact(capacity.checked_add(self.extra).unwrap())
                .expect("small deterministic constructor fixture");
        }
        self.events.push(ReserveEvent {
            site,
            requested: capacity,
            width: size_of::<T>(),
            element: type_name::<T>(),
            succeeded,
            pointer: storage.as_ptr() as usize,
            capacity: storage.capacity(),
        });
        succeeded
    }
}
