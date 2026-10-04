use fe2o3_core::{DeviceBuffer, OwnedDeviceOperation, Result, Stream};

fn accepted(stream: &Stream, output: DeviceBuffer<u32>) -> Result<DeviceBuffer<u32>> {
    let operation = unsafe {
        OwnedDeviceOperation::submit_unchecked(stream, output, |retained| {
            let _ = retained.len();
            Ok(())
        })?
    };
    let output = operation.wait()?;
    let _ = output.len();
    Ok(output)
}

fn main() {}
