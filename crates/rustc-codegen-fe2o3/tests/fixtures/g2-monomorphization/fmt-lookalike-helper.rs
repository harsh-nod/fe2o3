#![no_std]

pub mod fmt {
    #[inline]
    pub fn hidden(function: fn(u32) -> u32, value: u32) -> u32 {
        function(value)
    }
}
