#![no_std]

use renamed_core::DeviceCopy as DeviceCopyTrait;

#[derive(Clone, Copy, renamed_core::DeviceCopy)]
#[repr(C)]
pub struct RenamedPair {
    pub left: u32,
    pub right: u32,
}

const _: () = {
    fn assert_device_copy<T: DeviceCopyTrait>() {}
    let _ = assert_device_copy::<RenamedPair>;
};
