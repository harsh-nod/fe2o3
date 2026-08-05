use fe2o3_core::{BorrowedDeviceOperation, DeviceBuffer, Result, Stream};

fn rejected(stream: &Stream, output: &mut DeviceBuffer<u32>) -> Result<()> {
    unsafe {
        BorrowedDeviceOperation::run_scoped_unchecked(
            stream,
            &mut *output,
            |_| Ok(()),
            |_| {
                let _ = output.to_host_vec(stream)?;
                Ok::<(), fe2o3_core::Error>(())
            },
        )
    }??;
    Ok(())
}

fn main() {}
