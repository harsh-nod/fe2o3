//! One original executable loan, with every original WO source independently checked.

use super::*;
type NativeResult = Result<(), crate::RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>;
type LoanResult = Result<NativeResult, GeneratedNativeInputErrorV1>;

fn invalid() -> GeneratedNativeInputErrorV1 {
    GeneratedNativeInputErrorV1::Source(RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster)
}

impl<P: RuntimeGfx942GeneratedCarrierV1> RuntimeGfx942GeneratedArena1024V1<P> {
    pub(crate) fn with_native_inputs_v1(
        &self,
        uid: u64,
        expected: &GeneratedHostRosterV1,
        callback: impl for<'a> FnOnce(
            fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'a>,
            u64,
        ) -> NativeResult,
    ) -> LoanResult {
        if !self
            .validate_sources(uid)
            .map_err(GeneratedNativeInputErrorV1::Source)?
            .matches(expected)
        {
            return Err(invalid());
        }
        let first = self.members[0]
            .as_ref()
            .unwrap_or_else(|| std::process::abort())
            .source();
        let first_roster = first
            .validate(uid)
            .map_err(GeneratedNativeInputErrorV1::Source)?;
        let mut nested_error = None;
        let result = first.with_native_inputs_v1(uid, &first_roster, |program, buffers| {
            if buffers.len() != 1 || !self.matches_member(0, &first_roster) {
                nested_error = Some(invalid());
                return Ok(());
            }
            for index in 1..SLOTS {
                let source = self.members[index]
                    .as_ref()
                    .unwrap_or_else(|| std::process::abort())
                    .source();
                let actual = match source.validate(uid) {
                    Ok(actual) if self.matches_member(index, &actual) => actual,
                    Ok(_) => {
                        nested_error = Some(invalid());
                        return Ok(());
                    }
                    Err(error) => {
                        nested_error = Some(GeneratedNativeInputErrorV1::Source(error));
                        return Ok(());
                    }
                };
                let mut matched = false;
                // These are WO inputs. Their native storage needs no initial CPU
                // content. Their original carriers remain borrowed and retained;
                // only the first actual, byte-equal executable is lent to KFD.
                let checked = source.with_native_inputs_v1(uid, &actual, |other, inputs| {
                    matched = inputs.len() == 1
                        && inputs[0].bytes().len() as u64 == actual.readback_bytes
                        && other.envelope().bytes() == program.envelope().bytes()
                        && other.selected_kernel_index() == program.selected_kernel_index()
                        && other.selected_binding() == program.selected_binding();
                    Ok(())
                });
                match checked {
                    Err(error) => {
                        nested_error = Some(error);
                        return Ok(());
                    }
                    Ok(Err(error)) => return Err(error),
                    Ok(Ok(())) if !matched => {
                        nested_error = Some(invalid());
                        return Ok(());
                    }
                    Ok(Ok(())) => {}
                }
            }
            let result = callback(program, expected.readback_bytes);
            if let Err(error) = self.revalidate_sources() {
                nested_error = Some(GeneratedNativeInputErrorV1::Source(error));
            }
            result
        })?;
        match nested_error {
            Some(error) => Err(error),
            None => Ok(result),
        }
    }
}
