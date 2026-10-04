use fe2o3_core::{BorrowedDeviceOperation, DeviceBuffer, Result, Stream};

fn rejected(stream: &Stream, mut output: DeviceBuffer<u32>) -> Result<()> {
    unsafe {
        BorrowedDeviceOperation::run_scoped_unchecked(
            stream,
            &mut output,
            |_| Ok(()),
            |_| drop(output),
        )
    }?;
    Ok(())
}

fn main() {}
