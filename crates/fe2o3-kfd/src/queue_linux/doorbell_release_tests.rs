use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LocalDoorbellObservation {
    address: usize,
    plan: DoorbellMmapPlanV1,
    opener_pid: u32,
    active: bool,
}

pub(crate) fn observe_local_doorbell(owner: &LinuxDoorbellSliceV1) -> LocalDoorbellObservation {
    LocalDoorbellObservation {
        address: owner.address.as_ptr() as usize,
        plan: owner.plan,
        opener_pid: owner.opener_pid,
        active: owner.active,
    }
}

pub(crate) fn local_doorbell() -> LinuxDoorbellSliceV1 {
    let bytes = KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES as usize;
    // SAFETY: this independent anonymous VMA is exclusively owned by the returned
    // test slice. It is never MMIO; tests unmap it once after their assertions.
    let address = unsafe {
        rustix::mm::mmap_anonymous(
            core::ptr::null_mut(),
            bytes,
            ProtFlags::READ | ProtFlags::WRITE,
            MapFlags::PRIVATE,
        )
    }
    .unwrap();
    LinuxDoorbellSliceV1 {
        address: NonNull::new(address).unwrap(),
        plan: DoorbellMmapPlanV1 {
            encoded_slice_offset: 0,
            queue_byte_offset: 0,
            slice_bytes: bytes,
        },
        opener_pid: std::process::id(),
        active: true,
    }
}

pub(crate) fn cleanup_local_doorbell(doorbell: &mut LinuxDoorbellSliceV1) {
    if doorbell.active {
        doorbell.unmap_owned_v1().unwrap();
        doorbell.active = false;
    }
}

pub(crate) fn release_local_doorbell(
    doorbell: &mut LinuxDoorbellSliceV1,
    progress: &mut LinuxDoorbellReleaseProgressV1,
    fault: Option<(usize, bool)>,
) -> Result<(), LinuxDoorbellErrorV1> {
    doorbell.release_retaining_with_v1(progress, |owner| {
        if let Some((occurrence, panic)) = fault {
            if panic {
                std::panic::panic_any(("sdma-doorbell", occurrence));
            }
            return Err(rustix::io::Errno::IO);
        }
        owner.unmap_owned_v1()
    })
}

#[test]
fn retained_doorbell_success_error_panic_and_preflight_are_one_shot() {
    for mode in 0..4 {
        let mut owner = local_doorbell();
        let address = owner.address;
        let mut progress = LinuxDoorbellReleaseProgressV1::default();
        if mode == 3 {
            owner.opener_pid = std::process::id().wrapping_add(1);
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            owner.release_retaining_with_v1(&mut progress, |owner| match mode {
                0 => owner.unmap_owned_v1(),
                1 => Err(rustix::io::Errno::IO),
                2 => std::panic::panic_any("doorbell unmap panic"),
                _ => unreachable!(),
            })
        }));
        match mode {
            0 => result.unwrap().unwrap(),
            1 => assert!(matches!(
                result.unwrap(),
                Err(LinuxDoorbellErrorV1::Syscall {
                    source: rustix::io::Errno::IO,
                    ..
                })
            )),
            2 => assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"doorbell unmap panic")
            ),
            3 => assert!(matches!(
                result.unwrap(),
                Err(LinuxDoorbellErrorV1::ProcessChanged)
            )),
            _ => unreachable!(),
        }
        assert_eq!(owner.address, address);
        assert_eq!(owner.active, mode != 0);
        assert!(progress.started);
        assert_eq!(progress.attempted, mode != 3);
        assert_eq!(
            progress.result,
            match mode {
                0 => Some(Ok(())),
                1 => Some(Err(rustix::io::Errno::IO)),
                _ => None,
            }
        );
        let before = (progress, owner.active, owner.address, owner.opener_pid);
        assert!(owner.release_retaining_v1(&mut progress).is_err());
        assert_eq!(
            (progress, owner.active, owner.address, owner.opener_pid),
            before
        );
        cleanup_local_doorbell(&mut owner);
    }
}
