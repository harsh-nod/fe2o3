pub(super) fn validate_attachment(
    f: &SemanticFunctionDeclV1,
    r: SemanticGuardedGridLeaderV1,
) -> Result<(), SemanticMirErrorV1> {
    let mut work = 2 * MAX_BODY_BYTES;
    require(
        body::observe(f, r.types) == Some(r.roles)
            && identity(r.function(), f, &mut work)? == r.origin,
    )
}
pub(super) fn validate(
    context: &mut ValidationContextV1<'_>,
    function: SemanticFunctionIdV1,
    record: SemanticGuardedGridLeaderV1,
) -> Result<(), SemanticMirErrorV1> {
    context.one()?;
    require(
        record.function() == function
            && kernel_capability_provenance_matches(context.request, record.provenance),
    )?;
    for ty in record.types.all() {
        context.type_reference(ty, SemanticMirLocationV1::Function(function))?;
    }
    let remaining = context
        .limits
        .limit(SemanticMirResourceV1::ValidationWork)
        .saturating_sub(context.work);
    let mut work = remaining;
    let observed = SemanticGuardedGridLeaderV1::for_defined_function(
        function,
        &context.request.functions,
        &context.request.callables,
        &context.request.types,
        record.types,
        record.source,
        record.provenance,
        record.brand,
        &mut work,
    );
    charge_validation_work(context, (remaining - work) as usize)?;
    require(observed? == record)
}
impl SemanticGuardedGridLeaderV1 {
    pub(super) fn encode_payload(
        self,
        w: &mut CanonicalWriterV1,
    ) -> Result<(), SemanticMirErrorV1> {
        for body in [
            self.origin,
            self.issuer,
            self.caller,
            self.grid_getter,
            self.grid_current,
        ] {
            w.u32(body.function.index())?;
            w.identity(*body.source.as_bytes())?;
            w.identity(*body.abi.as_bytes())?;
            w.identity(body.body)?;
        }
        for ty in self.types.all() {
            w.u32(ty.index())?;
        }
        for value in [
            self.source.caller.index(),
            self.source.call_block.index(),
            self.source.grid_getter.index(),
            self.source.grid_current.index(),
            self.source.grid_call_block.index(),
            self.roles.guard.index(),
            self.roles.issuer.index(),
            self.roles.some.index(),
            self.roles.none.index(),
            self.roles.exit.index(),
            self.roles.receiver.index(),
            self.roles.result.index(),
            self.roles.issued.index(),
            self.roles.issuer_callable.index(),
        ] {
            w.u32(value)?;
        }
        encode_kernel_capability_provenance(w, self.provenance)?;
        w.identity(*self.brand.as_bytes())
    }
}
