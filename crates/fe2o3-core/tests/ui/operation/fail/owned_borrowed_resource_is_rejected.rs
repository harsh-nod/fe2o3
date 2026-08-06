use fe2o3_core::{DeviceBuffer, OwnedDeviceOperation, Result, Stream};

fn rejected(stream: &Stream, output: &mut DeviceBuffer<u32>) -> Result<()> {
    let _operation = unsafe {
        OwnedDeviceOperation::submit_unchecked(stream, &mut *output, |_| Ok(()))?
    };
    Ok(())
}

fn main() {}
