#![deny(unused_must_use)]

use fe2o3_core::{OwnedDeviceOperation, Result, Stream};

fn rejected(stream: &Stream) -> Result<()> {
    unsafe { OwnedDeviceOperation::submit_unchecked(stream, (), |_| Ok(()))? };
    Ok(())
}

fn main() {}
