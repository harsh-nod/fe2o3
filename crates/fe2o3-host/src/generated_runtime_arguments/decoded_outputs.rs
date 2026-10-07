use super::*;

impl GeneratedRuntimeOutputDecoderV1 {
    /// Consumes owned, address-free buffer data after validating the entire shape first.
    /// No native resource is freed, no completion certificate is created, and no work is issued.
    /// Each buffer must have capacity exactly equal to its length; excess backing is rejected.
    pub fn decode_buffers(
        self,
        buffers: Vec<(Gfx942RuntimeBufferAccessV1, Vec<u8>)>,
    ) -> Result<(), GeneratedRuntimeArgumentErrorV1> {
        if self.result_gate.is_some() {
            return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
        }
        let count = self
            .expectations
            .iter()
            .filter(|expected| expected.byte_len != 0)
            .count();
        if count != buffers.len() {
            return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
        }
        let mut data = buffers.iter();
        for expected in &self.expectations {
            if expected.byte_len != 0 {
                let buffer = data
                    .next()
                    .ok_or(GeneratedRuntimeArgumentErrorV1::BindingMismatch)?;
                if buffer.1.len() != expected.byte_len
                    || buffer.1.capacity() != expected.byte_len
                    || buffer.0 != expected.access
                {
                    return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
                }
            }
            if let Some(custody) = &expected.custody {
                custody.bound_to(None)?;
            }
        }
        let mut data = buffers.into_iter();
        for expected in self.expectations {
            let bytes = if expected.byte_len == 0 {
                Vec::new()
            } else {
                data.next()
                    .ok_or(GeneratedRuntimeArgumentErrorV1::BindingMismatch)?
                    .1
            };
            if let Some(OutputCustody::Legacy(custody)) = expected.custody {
                *custody
                    .state
                    .lock()
                    .map_err(|_| GeneratedRuntimeArgumentErrorV1::Custody)? =
                    OutputState::Decoded {
                        scalar: custody.scalar,
                        bytes,
                    };
            }
        }
        Ok(())
    }
}

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeResultV1<T> {
    pub fn try_take(&mut self) -> Result<Option<Box<[T]>>, GeneratedRuntimeArgumentErrorV1> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| GeneratedRuntimeArgumentErrorV1::Custody)?;
        match &*state {
            OutputState::Unbound | OutputState::Bound => Ok(None),
            OutputState::Taken | OutputState::Unavailable => {
                Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
            }
            OutputState::Decoded { scalar, bytes } => {
                if *scalar != T::RUST_SCALAR_TYPE || !bytes.len().is_multiple_of(size_of::<T>()) {
                    return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
                }
                let mut values = Vec::new();
                values
                    .try_reserve_exact(bytes.len() / size_of::<T>())
                    .map_err(|_| GeneratedRuntimeArgumentErrorV1::Allocation)?;
                for encoded in bytes.chunks_exact(size_of::<T>()) {
                    values.push(
                        T::decode_le_bytes_v1(encoded)
                            .ok_or(GeneratedRuntimeArgumentErrorV1::BindingMismatch)?,
                    );
                }
                *state = OutputState::Taken;
                Ok(Some(values.into_boxed_slice()))
            }
        }
    }
}
