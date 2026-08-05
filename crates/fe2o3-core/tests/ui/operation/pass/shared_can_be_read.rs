use fe2o3_core::{BorrowedDeviceOperation, DeviceBuffer, Result, Stream};

fn accepted(stream: &Stream, input: &DeviceBuffer<u32>) -> Result<usize> {
    unsafe {
        BorrowedDeviceOperation::run_scoped_unchecked(
            stream,
            input,
            |_| Ok(()),
            |_| input.len(),
        )
    }
}

fn main() {}
