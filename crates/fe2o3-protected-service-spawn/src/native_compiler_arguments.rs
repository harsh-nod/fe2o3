//! Owned exec bytes and frozen pointer tables. No invocation or image admission.

use super::{ProtectedServiceSpawnErrorV2 as Error, Result};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::{ffi::CString, mem::size_of};

// Mechanical exec limits, independent of the caller's source/descriptor policy.
pub(super) const MAX_ARGUMENTS: usize = 4096;
pub(super) const MAX_ENVIRONMENT: usize = 1024;
pub(super) const MAX_BYTES: usize = 256 * 1024;
const MAX_STRINGS: usize = MAX_ARGUMENTS + MAX_ENVIRONMENT;

pub(crate) struct CompilerArguments {
    arguments: Vec<CString>,
    environment: Vec<CString>,
    argv: Vec<usize>,
    envp: Vec<usize>,
}

impl CompilerArguments {
    pub(super) const WORK: usize = 16 * (MAX_BYTES + MAX_STRINGS + 2);
    pub(super) const MAX_STORAGE: usize = size_of::<Self>()
        + MAX_BYTES
        + MAX_STRINGS * size_of::<CString>()
        + (MAX_STRINGS + 2) * size_of::<usize>();
    // CString validation/boxing can temporarily overlap one entire byte copy.
    pub(super) const SCRATCH: usize = 3 * Self::MAX_STORAGE;

    /// Only called inside the native stage's prepaid copy and cleanup scope.
    pub(super) fn copy(arguments: &[CString], environment: &[CString]) -> Result<Self> {
        if arguments.is_empty()
            || arguments.len() > MAX_ARGUMENTS
            || environment.len() > MAX_ENVIRONMENT
        {
            return Err(Error::State("invalid native compiler argument count"));
        }
        let mut bytes = 0_usize;
        for value in arguments.iter().chain(environment) {
            bytes = bytes
                .checked_add(value.as_bytes_with_nul().len())
                .ok_or(Resource::Arithmetic)?;
            if bytes > MAX_BYTES {
                return Err(Error::State("native compiler argument bytes exceed bound"));
            }
        }
        let arguments = copy_strings(arguments)?;
        let environment = copy_strings(environment)?;
        // CString buffers never move or mutate after these tables are built.
        // Exposed addresses are consumed only by the x86-64 execveat syscall;
        // retaining integer words avoids an unnecessary unsafe Send implementation.
        let argv = pointers(&arguments)?;
        let envp = pointers(&environment)?;
        Ok(Self {
            arguments,
            environment,
            argv,
            envp,
        })
    }

    pub(super) fn retained_storage(&self) -> Result<usize> {
        let mut bytes = size_of::<Self>();
        for values in [&self.arguments, &self.environment] {
            bytes = bytes
                .checked_add(
                    values
                        .capacity()
                        .checked_mul(size_of::<CString>())
                        .ok_or(Resource::Arithmetic)?,
                )
                .ok_or(Resource::Arithmetic)?;
            for value in values {
                bytes = bytes
                    .checked_add(value.as_bytes_with_nul().len())
                    .ok_or(Resource::Arithmetic)?;
            }
        }
        for table in [&self.argv, &self.envp] {
            bytes = bytes
                .checked_add(
                    table
                        .capacity()
                        .checked_mul(size_of::<usize>())
                        .ok_or(Resource::Arithmetic)?,
                )
                .ok_or(Resource::Arithmetic)?;
        }
        Ok(bytes)
    }

    pub(crate) fn arguments(&self) -> &[CString] {
        &self.arguments
    }
    pub(crate) fn environment(&self) -> &[CString] {
        &self.environment
    }
    pub(crate) fn argv(&self) -> *const usize {
        self.argv.as_ptr()
    }
    pub(crate) fn envp(&self) -> *const usize {
        self.envp.as_ptr()
    }
}

fn bounded_vec<T>(length: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| Error::State("native compiler argument allocation refused"))?;
    if values.capacity() != length {
        return Err(Error::State(
            "native compiler argument capacity exceeds reservation",
        ));
    }
    Ok(values)
}

fn copy_strings(source: &[CString]) -> Result<Vec<CString>> {
    let mut values = bounded_vec(source.len())?;
    for value in source {
        let mut bytes = bounded_vec(value.as_bytes_with_nul().len())?;
        bytes.extend_from_slice(value.as_bytes_with_nul());
        values.push(
            CString::from_vec_with_nul(bytes)
                .map_err(|_| Error::State("invalid native compiler C string"))?,
        );
    }
    Ok(values)
}

fn pointers(values: &[CString]) -> Result<Vec<usize>> {
    let mut table = bounded_vec(values.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
    table.extend(
        values
            .iter()
            .map(|value| value.as_ptr().expose_provenance()),
    );
    table.push(0);
    Ok(table)
}

#[cfg(test)]
#[path = "native_compiler_arguments_tests.rs"]
mod tests;
