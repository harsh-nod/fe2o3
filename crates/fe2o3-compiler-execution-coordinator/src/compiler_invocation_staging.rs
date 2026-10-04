//! Exact process bytes from cargo's inert prepared V3 descriptor.
//!
//! This module stages data only. The native launch owner must retain and
//! revalidate the original compiler runtime, executable transfers, invocation
//! capture and cwd directory object. In particular, the canonical cwd below is
//! an untranslated pathname: it does not identify a directory descriptor or
//! admit a namespace mapping. No process, runtime profile or authority is made.

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_rustc_invocation::{
    MAX_ARGUMENT_BYTES_V2, MAX_COMPILE_ENVIRONMENT_ENTRIES_V2, MAX_DESCRIPTOR_BYTES_V2,
    MAX_ENVIRONMENT_VALUE_BYTES_V2, MAX_NAME_BYTES_V2, MAX_PATH_BYTES_V2, MAX_RUSTC_ARGUMENTS_V2,
    RustcInvocationDescriptorV3,
};
use std::{
    error::Error,
    ffi::{CStr, CString},
    fmt,
    mem::size_of,
};

const ENTRY: usize = 8;
const MAX_STRINGS: usize = MAX_RUSTC_ARGUMENTS_V2 + MAX_COMPILE_ENVIRONMENT_ENTRIES_V2 + 1;
// C terminators and environment '=' bytes replace longer wire length prefixes.
const MAX_C_BYTES: usize = MAX_DESCRIPTOR_BYTES_V2;
const MEASURE_WORK: usize = ENTRY + 8 * (MAX_STRINGS + MAX_COMPILE_ENVIRONMENT_ENTRIES_V2);
const COPY_WORK: usize = 16 * (MAX_C_BYTES + MAX_STRINGS);
const FRAME: usize =
    4 * size_of::<(StagedRustcInvocationV1, RustcInvocationStagingChargeV1)>() + 1024;

type Result<T> = std::result::Result<T, RustcInvocationStagingErrorV1>;

/// A FULL, unreserved charge for the returned C-string owner, excluding its source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RustcInvocationStagingChargeV1(usize);

impl RustcInvocationStagingChargeV1 {
    /// Reserve before retaining the result; retire only after dropping it.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Bounded staging failures without copying invocation or environment text.
#[derive(Debug)]
pub enum RustcInvocationStagingErrorV1 {
    /// Refusal from the original work/storage ledger.
    Resource(Resource),
    /// A C-string or staging bound was violated.
    Invalid(&'static str),
}

impl From<Resource> for RustcInvocationStagingErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for RustcInvocationStagingErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Invalid(reason) => f.write_str(reason),
        }
    }
}

impl Error for RustcInvocationStagingErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

/// Move-only inert C strings for one exact prepared rustc process.
///
/// This is not a spawn owner, descriptor capability or execution receipt. It
/// contains no raw pointer tables; a native adapter must separately fund and
/// retain those tables alongside these strings through its guarded exec path.
pub struct StagedRustcInvocationV1 {
    arguments: Vec<CString>,
    environment: Vec<CString>,
    working_directory: CString,
    retained: usize,
}

impl StagedRustcInvocationV1 {
    /// Full bounded work for measurement, C-string construction and partial-drop cleanup.
    pub const STAGING_WORK: usize = MEASURE_WORK + COPY_WORK;
    /// Peak additional logical storage, including overlapping C-string conversion.
    pub const STAGING_SCRATCH: usize =
        FRAME + 3 * MAX_C_BYTES + 2 * MAX_STRINGS * size_of::<CString>();

    /// Stages the descriptor supplied by cargo's prepared invocation capture.
    ///
    /// Arguments retain order, repetitions, empty strings and exact argv[0].
    /// Environment entries retain the descriptor's complete sorted set and
    /// become `key=value` without inheritance or substitutions. V2/V3 already
    /// reject NUL and non-UTF-8 compile inputs; conversion never uses lossy text.
    ///
    /// The original descriptor's FULL backing charge must be prepaid. Its
    /// bounded capacity traversal is charged before measurement, followed by
    /// the floor check and prepaid construction. Entry storage is restored on
    /// success, failure or unwind; work, peaks and denials stay on `b`. Reserve
    /// the returned FULL charge before retaining the staged result. Source
    /// reservations remain live until their owners are dropped.
    pub fn stage(
        descriptor: &RustcInvocationDescriptorV3,
        b: &mut Budget<'_>,
    ) -> Result<(Self, RustcInvocationStagingChargeV1)> {
        b.charge_work(ENTRY)?;
        // Typed V3 inputs already satisfy the frozen descriptor bounds. Check
        // counts before even the prepaid capacity traversal or any allocation.
        let argv_count = descriptor.rustc().argv().len();
        let entries = descriptor.compile_environment().entries();
        if argv_count == 0
            || argv_count > MAX_RUSTC_ARGUMENTS_V2
            || entries.len() > MAX_COMPILE_ENVIRONMENT_ENTRIES_V2
        {
            return Err(RustcInvocationStagingErrorV1::Invalid(
                "invalid invocation count",
            ));
        }
        let source = b.with_prepaid_scope(0, 0, MEASURE_WORK - ENTRY, FRAME, |_| {
            descriptor
                .retained_storage_bytes()
                .ok_or(Resource::Arithmetic)
        })?;
        b.with_prepaid_scope(source, 0, COPY_WORK, Self::STAGING_SCRATCH, |_| {
            validate_byte_bounds(descriptor)?;
            let argv = descriptor.rustc().argv();
            let mut arguments = bounded_vec(argv.len())?;
            let mut environment = bounded_vec(entries.len())?;
            let mut bytes = 0;
            for argument in argv {
                arguments.push(c_string(&[argument.as_bytes()], &mut bytes)?);
            }
            for entry in entries {
                environment.push(c_string(
                    &[entry.key().as_bytes(), b"=", entry.value().as_bytes()],
                    &mut bytes,
                )?);
            }
            let working_directory = c_string(
                &[descriptor.rustc().working_directory().as_bytes()],
                &mut bytes,
            )?;
            let retained = size_of::<(Self, RustcInvocationStagingChargeV1)>()
                .checked_add(bytes)
                .and_then(|n| {
                    n.checked_add(arguments.capacity().checked_mul(size_of::<CString>())?)
                })
                .and_then(|n| {
                    n.checked_add(environment.capacity().checked_mul(size_of::<CString>())?)
                })
                .ok_or(Resource::Arithmetic)?;
            Ok((
                Self {
                    arguments,
                    environment,
                    working_directory,
                    retained,
                },
                RustcInvocationStagingChargeV1(retained),
            ))
        })
    }

    /// Exact argument sequence, including the descriptor's original argv[0].
    pub fn arguments(&self) -> &[CString] {
        &self.arguments
    }

    /// Complete child environment as sorted `key=value` strings.
    pub fn environment(&self) -> &[CString] {
        &self.environment
    }

    /// Exact canonical cwd pathname, with no implicit namespace translation.
    ///
    /// The native adapter must explicitly map this path to its retained cwd
    /// object and establish that mapping under its existing authority. Comparing
    /// pathname or content digests alone cannot establish that object binding.
    pub fn working_directory(&self) -> &CStr {
        &self.working_directory
    }

    /// Full backing charge for these C strings and their containing vectors.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
}

fn validate_byte_bounds(descriptor: &RustcInvocationDescriptorV3) -> Result<()> {
    let mut total = 0usize;
    let mut add = |length: usize, maximum: usize, separators: usize| -> Result<()> {
        if length > maximum {
            return Err(RustcInvocationStagingErrorV1::Invalid(
                "invocation field exceeds bound",
            ));
        }
        total = total
            .checked_add(length)
            .and_then(|n| n.checked_add(separators))
            .ok_or(Resource::Arithmetic)?;
        if total > MAX_C_BYTES {
            return Err(RustcInvocationStagingErrorV1::Invalid(
                "invocation C strings exceed bound",
            ));
        }
        Ok(())
    };
    add(
        descriptor.rustc().working_directory().len(),
        MAX_PATH_BYTES_V2,
        1,
    )?;
    for argument in descriptor.rustc().argv() {
        add(argument.len(), MAX_ARGUMENT_BYTES_V2, 1)?;
    }
    for entry in descriptor.compile_environment().entries() {
        add(entry.key().len(), MAX_NAME_BYTES_V2, 1)?;
        add(entry.value().len(), MAX_ENVIRONMENT_VALUE_BYTES_V2, 1)?;
    }
    Ok(())
}

fn bounded_vec<T>(length: usize) -> Result<Vec<T>> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| Resource::Allocation)?;
    if result.capacity() > length.checked_mul(2).ok_or(Resource::Arithmetic)? {
        return Err(Resource::Allocation.into());
    }
    Ok(result)
}

fn c_string(parts: &[&[u8]], total: &mut usize) -> Result<CString> {
    let length = parts.iter().try_fold(1usize, |n, part| {
        n.checked_add(part.len()).ok_or(Resource::Arithmetic)
    })?;
    let next = total.checked_add(length).ok_or(Resource::Arithmetic)?;
    if next > MAX_C_BYTES {
        return Err(RustcInvocationStagingErrorV1::Invalid(
            "invocation C strings exceed bound",
        ));
    }
    let mut bytes = bounded_vec(length)?;
    for part in parts {
        bytes.extend_from_slice(part);
    }
    bytes.push(0);
    let string = CString::from_vec_with_nul(bytes)
        .map_err(|_| RustcInvocationStagingErrorV1::Invalid("invocation contains interior NUL"))?;
    *total = next;
    Ok(string)
}

#[cfg(test)]
#[path = "compiler_invocation_staging_tests.rs"]
mod tests;
