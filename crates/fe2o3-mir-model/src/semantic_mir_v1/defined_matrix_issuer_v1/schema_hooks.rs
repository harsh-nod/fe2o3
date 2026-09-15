// Include in the model child only when the enum/codec/admission mount is complete.
impl SemanticMatrixIssuerBodyV1 {
    fn encode(self, writer: &mut CanonicalWriterV1) -> Result<(), SemanticMirErrorV1> {
        writer.u32(self.function.index())?;
        writer.identity(*self.source_identity.as_bytes())?;
        writer.identity(*self.abi_identity.as_bytes())?;
        writer.identity(self.body_identity)
    }
}
impl SemanticKernelMatrixDeriveV1 {
    /// Payload only. The shared closed enum must assign its own discriminant.
    pub(super) fn encode_payload(
        self,
        writer: &mut CanonicalWriterV1,
    ) -> Result<(), SemanticMirErrorV1> {
        self.origin.encode(writer)?;
        self.bridge.encode(writer)?;
        writer.u32(self.current_callable.index())?;
        writer.identity(*self.current_source_identity.as_bytes())?;
        writer.identity(*self.current_abi_identity.as_bytes())?;
        for id in self.types.all() {
            writer.u32(id.index())?;
        }
        encode_kernel_capability_provenance(writer, self.provenance)?;
        writer.identity(*self.kernel_brand.as_bytes())
    }
}

pub(super) fn validate_matrix_derive_attachment(
    body: &SemanticFunctionDeclV1,
    record: SemanticKernelMatrixDeriveV1,
) -> Result<(), SemanticMirErrorV1> {
    let mut budget = MAX_BODY_BYTES;
    require(
        body.role() == SemanticFunctionRoleV1::InternalHelper
            && body.export().is_none()
            && getter_body(body, record.types)
                == Some(SemanticCallableIdV1::from_index(
                    record.bridge.function.index(),
                ))
            && SemanticMatrixIssuerBodyV1::observe(record.function(), body, &mut budget)?
                == record.origin,
    )
}

/// Full roster/type/root revalidation is mandatory before attachment is admitted.
pub(super) fn validate_matrix_derive(
    context: &mut ValidationContextV1<'_>,
    function: SemanticFunctionIdV1,
    record: SemanticKernelMatrixDeriveV1,
) -> Result<(), SemanticMirErrorV1> {
    context.one()?;
    require(
        record.function() == function
            && kernel_capability_provenance_matches(context.request, record.provenance),
    )?;
    for ty in record.types.all() {
        context.type_reference(ty, SemanticMirLocationV1::Function(function))?;
    }
    let mut budget = validation_digest_budget(context, 2 * MAX_BODY_BYTES);
    let before = budget;
    let expected = SemanticKernelMatrixDeriveV1::observe(
        function,
        &context.request.functions,
        &context.request.callables,
        &context.request.types,
        record.types,
        record.provenance,
        record.kernel_brand,
        &mut budget,
    )
    .map_err(|error| digest_budget_error(context, error))?;
    charge_validation_work(context, ((before - budget) * 2) as usize)?;
    require(expected == record)
}

fn validation_digest_budget(context: &ValidationContextV1<'_>, max: u64) -> u64 {
    (context
        .limits
        .limit(SemanticMirResourceV1::ValidationWork)
        .saturating_sub(context.work)
        / 2)
    .min(max)
}

fn digest_budget_error(
    context: &ValidationContextV1<'_>,
    error: SemanticMirErrorV1,
) -> SemanticMirErrorV1 {
    match error {
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        } => {
            let max = context.limits.limit(SemanticMirResourceV1::ValidationWork);
            SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                actual: max.saturating_add(1),
                max,
            }
        }
        error => error,
    }
}
