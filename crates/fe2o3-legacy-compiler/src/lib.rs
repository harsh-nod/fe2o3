#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Dormant adapter contract for the existing legacy compiler path.
//!
//! The existing codegen implementation remains in its current crate. This
//! crate only defines the contract by which that owner may later expose a
//! bounded `Legacy` transaction to `fe2o3-compiler-driver`. Merely constructing
//! this adapter does not alter production compiler selection.
//!
//! This crate has no COMGR, artifact publication, module loading, dispatch, or
//! launch authority.

use fe2o3_compiler_api::{CompileOutputV1, CompileRequestV1, PipelineSelectorV1};
use fe2o3_compiler_driver::{CompilerBackendFailureV1, TransactionalCompilerBackendV1};

/// Contract implemented by the owner of the existing legacy compiler path.
///
/// A source-level compiler rejection should be returned as a validated
/// [`CompileOutputV1`] so its diagnostics and receipt chain survive adaptation.
/// An `Err` commits no partial compiler output.
pub trait LegacyCompilePathV1 {
    /// Executes one bounded transaction through the existing legacy path.
    fn compile_legacy_transaction(
        &mut self,
        request: &CompileRequestV1,
    ) -> Result<CompileOutputV1, CompilerBackendFailureV1>;
}

/// Selector-guarded adapter for an existing legacy compiler path.
#[derive(Clone, Debug)]
pub struct LegacyCompilerAdapterV1<Path> {
    path: Path,
}

impl<Path> LegacyCompilerAdapterV1<Path> {
    /// Wraps a legacy path without registering or selecting it in production.
    pub const fn new(path: Path) -> Self {
        Self { path }
    }

    /// Returns a shared reference to the wrapped legacy path.
    pub const fn path(&self) -> &Path {
        &self.path
    }

    /// Returns a mutable reference to the wrapped legacy path.
    pub const fn path_mut(&mut self) -> &mut Path {
        &mut self.path
    }

    /// Returns the wrapped legacy path.
    pub fn into_inner(self) -> Path {
        self.path
    }
}

impl<Path> TransactionalCompilerBackendV1 for LegacyCompilerAdapterV1<Path>
where
    Path: LegacyCompilePathV1,
{
    fn compile_transaction(
        &mut self,
        request: &CompileRequestV1,
    ) -> Result<CompileOutputV1, CompilerBackendFailureV1> {
        if request.selector() != PipelineSelectorV1::Legacy {
            return Err(CompilerBackendFailureV1::UnsupportedRequest);
        }
        self.path.compile_legacy_transaction(request)
    }
}
