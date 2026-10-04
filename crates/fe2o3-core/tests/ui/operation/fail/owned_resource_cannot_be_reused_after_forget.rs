use fe2o3_core::{DeviceBuffer, OwnedDeviceOperation, Result, Stream};

fn rejected(stream: &Stream, output: DeviceBuffer<u32>) -> Result<()> {
    let operation = unsafe { OwnedDeviceOperation::submit_unchecked(stream, output, |_| Ok(()))? };
    core::mem::forget(operation);
    let _ = output.len();
    Ok(())
}

fn main() {}
