//! Shared module/provider checks after each outer source owner is admitted.

use fe2o3_compiler_ffi::{CompilerModuleHandoffV2, CompilerModuleSymbolRoleV1};

use crate::{
    ContentIdentityV1, InertDecodedWorkerExchangeV2, MultiInputLinkPlanV1, WorkerMeasurementV1,
    WorkerResponseV2, WorkerV3HsacoInspectionError as Error,
    request_construction::decode_link_options,
    worker_v3_hsaco_admission::{
        map_compiler_code_object_version, validate_strict_v3_gfx942_device_ffi,
        validate_strict_v3_gfx942_provider_exchange, validate_strict_v3_gfx950_device_ffi,
        validate_strict_v3_gfx950_provider_exchange,
    },
};

pub(crate) struct WorkerArtifactExchange<'a> {
    pub(crate) request: &'a [u8],
    pub(crate) response: &'a WorkerResponseV2,
    pub(crate) executable: ContentIdentityV1,
}

/// Borrowed immutable coordinates, never an admitted authority owner.
pub(crate) struct WorkerArtifactLineage<'a> {
    pub(crate) module: &'a CompilerModuleHandoffV2,
    pub(crate) plan: &'a MultiInputLinkPlanV1,
    pub(crate) measurement: &'a WorkerMeasurementV1,
    pub(crate) exchanges: [WorkerArtifactExchange<'a>; 2],
    pub(crate) output: &'a [u8],
}

impl WorkerArtifactLineage<'_> {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        let nested = self.module;
        if nested.target().to_string() != self.plan.target().to_string()
            || map_compiler_code_object_version(nested.code_object_version())
                != decode_link_options(self.plan.options())
                    .map_err(|_| Error::LinkPolicy)?
                    .0
        {
            return Err(Error::LineageMismatch(
                "strict V3 compiler envelope target/code-object version",
            ));
        }
        let gfx942 = validate_strict_v3_gfx942_device_ffi(nested)?;
        let gfx950 = validate_strict_v3_gfx950_device_ffi(nested)?;
        let directional = nested.envelope().directional_symbols();
        if !nested
            .symbol_manifest()
            .symbols(CompilerModuleSymbolRoleV1::UnresolvedExternalImport)
            .eq(directional.imports())
        {
            return Err(Error::CompilerEnvelopeImportRoleMismatch);
        }
        if !nested
            .symbol_manifest()
            .symbols(CompilerModuleSymbolRoleV1::DeviceFfiExport)
            .eq(directional.exports())
        {
            return Err(Error::CompilerEnvelopeExportRoleMismatch);
        }
        let target = nested.target().to_string();
        // Only the selected backend's existing provider policy is applicable.
        // Decode once per exchange; do not introduce a permissive native policy.
        let provider_error = || match target.as_str() {
            "gfx950:xnack-" => Error::StrictV3Gfx950OcmlProviderClosureMismatch,
            _ => Error::StrictV3Gfx942OcmlProviderClosureMismatch,
        };
        let mut providers = [None, None];
        for (index, exchange) in self.exchanges.iter().enumerate() {
            let response = exchange.response;
            if matches!(target.as_str(), "gfx942:xnack-" | "gfx950:xnack-") {
                let decoded = InertDecodedWorkerExchangeV2::decode(
                    exchange.request,
                    response.canonical_bytes(),
                )
                .map_err(|_| provider_error())?;
                if target == "gfx942:xnack-" {
                    validate_strict_v3_gfx942_provider_exchange(gfx942, &decoded)?;
                } else {
                    validate_strict_v3_gfx950_provider_exchange(gfx950, &decoded)?;
                }
                providers[index] = response.device_library_provider();
            }
            if self.measurement.executable() != exchange.executable
                || self.measurement.worker_build_identity() != response.worker_build_identity()
            {
                return Err(Error::LineageMismatch("strict V3 worker measurement"));
            }
            if response.compiler_envelope_identity().as_bytes()
                != nested.envelope().identity().as_bytes()
            {
                return Err(Error::LineageMismatch(
                    "strict V3 compiler envelope identity",
                ));
            }
            let output = response
                .output()
                .ok_or(Error::LineageMismatch("missing strict V3 linked output"))?;
            if output.identity() != self.plan.output().identity()
                || output.request_identity() != response.request_identity()
                || output.compiler_envelope_identity() != response.compiler_envelope_identity()
            {
                return Err(Error::LineageMismatch(
                    "strict V3 sealed request/response/output identity",
                ));
            }
            if output.bytes() != self.output {
                return Err(Error::LineageMismatch(
                    "strict V3 reproducible output bytes",
                ));
            }
        }
        if providers[0] != providers[1] {
            return Err(provider_error());
        }
        Ok(())
    }
}
