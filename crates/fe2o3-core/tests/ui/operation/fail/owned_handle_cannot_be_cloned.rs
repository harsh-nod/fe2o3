use fe2o3_core::{OwnedDeviceOperation, Result, Stream};

fn rejected(stream: &Stream) -> Result<()> {
    let operation = unsafe { OwnedDeviceOperation::submit_unchecked(stream, (), |_| Ok(()))? };
    let _duplicate = operation.clone();
    Ok(())
}

fn main() {}
