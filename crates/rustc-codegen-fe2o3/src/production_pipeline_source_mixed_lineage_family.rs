// Exact executed lineage and finalizer custody for concrete receipt families.
macro_rules! mixed_lineage_pipeline_family {
    ($lineage_error:ident, $check_request:ident, $layout:ident,
     $encode:ident, $replay_capsule:ident) => {
fn mismatch(detail: &'static str) -> Error {
    MixedPublicationErrorV28::Binding(detail).into()
}
fn equal(a: &[u8], b: &[u8], budget: &mut Budget<'_>) -> Result<(), Error> {
    if a.len() != b.len() {
        return Err(mismatch("mixed capsule field length"));
    }
    budget.charge_work(a.len())?;
    if a != b {
        return Err(mismatch("mixed capsule field bytes"));
    }
    Ok(())
}
fn frames<R, F, P, E>() -> Result<usize, Resource> {
    [
        size_of::<F>(),
        align_of::<F>(),
        size_of::<(&P, &E, F)>(),
        align_of::<(&P, &E, F)>(),
        size_of::<std::panic::AssertUnwindSafe<(&P, &E, F)>>(),
        size_of::<Result<R, Error>>(),
        size_of::<std::thread::Result<Result<R, Error>>>(),
        2 * size_of::<Result<(), Error>>(),
        size_of::<[(&[u8], &[u8]); 4]>(),
        size_of::<Layout>(),
        size_of::<Identity>(),
        5 * size_of::<usize>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}
fn content_error(error: ContentError) -> Error {
    match error {
        ContentError::Resource(error) => Error::Resource(error),
        ContentError::Refinement(error) => Error::MixedRelocationExpressions(error),
        other => MixedPublicationErrorV28::$lineage_error(other).into(),
    }
}

/// Constructed only by the existing execution continuation after the exact
/// protected candidate and runtime-owned request have both been checked.
pub(crate) struct ExecutedProtectedMixedPublicationV29<'e, 'r, 'a, 'v, 's> {
    candidate: &'e ProtectedMixedPublicationV28<'a, 'v, 's>,
    executed: &'e Executed<'r, 'a, 'v, 'v, 'v, 's>,
}

/// A lexical actual-Worker join, never the public content-only finalizer result.
/// Both original compiler custody and the real executed request remain alive.
pub(crate) struct FinalizedProtectedMixedLineageV29<'f, 'e, 'r, 'a, 'v, 's> {
    owner: &'f ExecutedProtectedMixedPublicationV29<'e, 'r, 'a, 'v, 's>,
    content: &'f Content<'e, 'r, 'a, 'v, 'v, 'v, 's>,
}

impl FinalizedProtectedMixedLineageV29<'_, '_, '_, '_, '_, '_> {
    pub(crate) fn replay(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.content.replay(budget).map_err(content_error)?;
        self.owner.candidate.check_mixed_lineage_handoff_v29(
            self.owner.executed,
            self.content
                .finalized(budget)
                .map_err(content_error)?
                .raw()
                .outer_handoff(),
            budget,
        )?;
        Ok(())
    }

    pub(crate) fn finalized(&self, budget: &mut Budget<'_>) -> Result<&Artifact, Error> {
        self.replay(budget)?;
        self.content.finalized(budget).map_err(content_error)
    }

    pub(crate) const fn grants_publication_or_launch_authority(&self) -> bool {
        false
    }
}

type StaticJoined =
    ExecutedProtectedMixedPublicationV29<'static, 'static, 'static, 'static, 'static>;
type StaticFinalized =
    FinalizedProtectedMixedLineageV29<'static, 'static, 'static, 'static, 'static, 'static>;
type StaticContent = Content<'static, 'static, 'static, 'static, 'static, 'static, 'static>;
type FinalizeCapture<'a, F> = (&'a StaticJoined, FirstBuild, Pending<F>);
type CallbackCapture<'a, F> = (&'a StaticFinalized, &'a mut Pending<F>);
type InvokeCapture<'a, 'w, F> = (CallbackCapture<'a, F>, &'a mut Budget<'w>);
fn finalize_frames<R, F>() -> Result<usize, Resource> {
    [
        size_of::<StaticFinalized>(),
        align_of::<StaticFinalized>(),
        size_of::<FinalizeCapture<'_, F>>(),
        align_of::<FinalizeCapture<'_, F>>(),
        size_of::<AssertUnwindSafe<FinalizeCapture<'_, F>>>(),
        size_of::<CallbackCapture<'_, F>>(),
        align_of::<CallbackCapture<'_, F>>(),
        size_of::<InvokeCapture<'_, '_, F>>(),
        align_of::<InvokeCapture<'_, '_, F>>(),
        size_of::<AssertUnwindSafe<InvokeCapture<'_, '_, F>>>(),
        size_of::<StaticContent>(),
        size_of::<Result<StaticContent, ContentError>>(),
        size_of::<Result<R, Error>>(),
        2 * size_of::<std::thread::Result<Result<R, Error>>>(),
        3 * size_of::<Result<(), Error>>(),
        size_of::<Result<(), ContentError>>(),
        size_of::<Result<Identity, Error>>(),
        size_of::<Identity>(),
        size_of::<Result<&Artifact, ContentError>>(),
        size_of::<Result<&Artifact, Error>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, n| {
        sum.checked_add(n).ok_or(Resource::Arithmetic)
    })
}

impl PreparedMixedPublicationV28<'_, '_, '_> {
    /// Checks original custody before the finalizer can touch a supplied meter.
    /// This query does not grant protected invocation or execution authority.
    pub(crate) fn check_lineage_account_v29(&self, budget: &Budget<'_>) -> Result<(), Error> {
        self.check(budget)
    }
}

impl<'e, 'r, 'a, 'v, 's> ExecutedProtectedMixedPublicationV29<'e, 'r, 'a, 'v, 's> {
    pub(super) fn new(
        candidate: &'e ProtectedMixedPublicationV28<'a, 'v, 's>,
        executed: &'e Executed<'r, 'a, 'v, 'v, 'v, 's>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        candidate.revalidate(budget)?;
        executed
            .$check_request(candidate.prepared.inputs.composed, budget)
            .map_err(Error::MixedRelocationExpressions)?;
        Ok(Self {
            candidate,
            executed,
        })
    }

    pub(crate) fn layout(&self, budget: &mut Budget<'_>) -> Result<Layout, Error> {
        self.candidate
            .mixed_lineage_layout_v29(self.executed, budget)
    }

    pub(crate) fn encode(
        &self,
        bytes: &mut [u8],
        budget: &mut Budget<'_>,
    ) -> Result<Identity, Error> {
        self.candidate
            .encode_mixed_lineage_v29(self.executed, bytes, budget)
    }

    /// The same first-build owner passes the mandatory actual-Worker join before
    /// the unchanged shared finalizer consumes it. No arbitrary callback can
    /// create the joined nominal owner or outlive its original custody.
    pub(crate) fn with_finalized_lineage<R, F>(
        &self,
        source: FirstBuild,
        budget: &mut Budget<'_>,
        consume: F,
    ) -> Result<R, Error>
    where
        F: for<'f> FnOnce(
            &FinalizedProtectedMixedLineageV29<'f, 'e, 'r, 'a, 'v, 's>,
            &mut Budget<'_>,
        ) -> Result<R, Error>,
    {
        let consume = Pending::new(consume);
        self.candidate.prepared.check_lineage_account_v29(budget)?;
        let header = finalize_frames::<R, F>()?;
        let floor = budget.storage();
        let source_owner = self.candidate.prepared.inputs.source;
        let capture = (self, source, consume);
        let operation = move |budget: &mut Budget<'_>| {
            let (owner, source, mut consume) = std::convert::identity(capture);
            owner.candidate.check_mixed_lineage_handoff_v29(
                owner.executed,
                source.handoff(),
                budget,
            )?;
            let content = finalize_content(
                source,
                owner.executed,
                budget,
            )
            .map_err(content_error)?;
            let joined = FinalizedProtectedMixedLineageV29 {
                owner,
                content: &content,
            };
            let capture = (&joined, &mut consume);
            let callback = move |budget: &mut Budget<'_>| {
                let (joined, consume) = std::convert::identity(capture);
                joined.replay(budget)?;
                let result = consume.take()(joined, budget)?;
                joined.replay(budget)?;
                Ok(result)
            };
            #[cfg(test)]
            {
                assert_eq!(
                    std::mem::size_of_val(&callback),
                    size_of::<CallbackCapture<'_, F>>()
                );
                assert_eq!(
                    std::mem::align_of_val(&callback),
                    align_of::<CallbackCapture<'_, F>>()
                );
            }
            let invoke_capture = (callback, &mut *budget);
            let invoke = move || {
                let (callback, budget) = std::convert::identity(invoke_capture);
                callback(budget)
            };
            #[cfg(test)]
            {
                assert_eq!(
                    std::mem::size_of_val(&invoke),
                    size_of::<InvokeCapture<'_, '_, F>>()
                );
                assert_eq!(
                    std::mem::align_of_val(&invoke),
                    align_of::<InvokeCapture<'_, '_, F>>()
                );
            }
            let result = catch_unwind(AssertUnwindSafe(invoke));
            let settled = content.discard(budget).map_err(content_error);
            match result {
                Ok(Ok(value)) => {
                    settled?;
                    Ok(value)
                }
                Ok(Err(error)) => {
                    let _ = settled;
                    Err(error)
                }
                Err(payload) => resume_unwind(payload),
            }
        };
        #[cfg(test)]
        {
            assert_eq!(
                std::mem::size_of_val(&operation),
                size_of::<FinalizeCapture<'_, F>>()
            );
            assert_eq!(
                std::mem::align_of_val(&operation),
                align_of::<FinalizeCapture<'_, F>>()
            );
        }
        budget
            .with_prepaid_scope(floor, 0, 0, header, operation)
            .map_err(|error| match error {
                Error::Resource(error) => {
                    Error::Source(source_owner.retain_query_resource_error_v18(error))
                }
                other => other,
            })
    }
}

impl<'a, 'v, 's> ProtectedMixedPublicationV28<'a, 'v, 's> {
    fn with_lineage<R, F>(
        &self,
        executed: &Executed<'_, 'a, 'v, 'v, 'v, 's>,
        external_bytes: usize,
        budget: &mut Budget<'_>,
        visit: F,
    ) -> Result<R, Error>
    where
        F: FnOnce(&mut Budget<'_>) -> Result<R, Error>,
    {
        self.prepared.check(budget)?;
        let source = self.prepared.inputs.source;
        let floor = self
            .prepared
            .required
            .checked_add(external_bytes)
            .ok_or(Resource::Arithmetic)?;
        let header = frames::<R, F, Self, Executed<'_, 'a, 'v, 'v, 'v, 's>>()?;
        let capture = (self, executed, visit);
        let operation = move |budget: &mut Budget<'_>| {
            let (owner, executed, visit) = std::convert::identity(capture);
            owner.revalidate(budget)?;
            executed
                .$check_request(owner.prepared.inputs.composed, budget)
                .map_err(Error::MixedRelocationExpressions)?;
            let result = visit(budget)?;
            owner.revalidate(budget)?;
            executed
                .$check_request(owner.prepared.inputs.composed, budget)
                .map_err(Error::MixedRelocationExpressions)?;
            Ok(result)
        };
        #[cfg(test)]
        {
            assert_eq!(
                std::mem::size_of_val(&operation),
                size_of::<(&Self, &Executed<'_, 'a, 'v, 'v, 'v, 's>, F)>()
            );
            assert_eq!(
                std::mem::align_of_val(&operation),
                align_of::<(&Self, &Executed<'_, 'a, 'v, 'v, 'v, 's>, F)>()
            );
        }
        budget
            .with_prepaid_scope(floor, 0, 0, header, operation)
            .map_err(|error| match error {
                Error::Resource(error) => {
                    Error::Source(source.retain_query_resource_error_v18(error))
                }
                other => other,
            })
    }
    /// Exact inert stage extent from this candidate's actual executed request.
    pub(crate) fn mixed_lineage_layout_v29(
        &self,
        executed: &Executed<'_, 'a, 'v, 'v, 'v, 's>,
        budget: &mut Budget<'_>,
    ) -> Result<Layout, Error> {
        self.with_lineage(executed, 0, budget, |budget| {
            executed
                .$layout(budget)
                .map_err(Error::MixedRelocationExpressions)
        })
    }
    /// Writes into caller-prepaid backing without manufacturing remaining stage
    /// receipts. The output is an inert middle-end preimage, not a capsule.
    pub(crate) fn encode_mixed_lineage_v29(
        &self,
        executed: &Executed<'_, 'a, 'v, 'v, 'v, 's>,
        bytes: &mut [u8],
        budget: &mut Budget<'_>,
    ) -> Result<Identity, Error> {
        self.with_lineage(executed, bytes.len(), budget, |budget| {
            executed
                .$encode(bytes, budget)
                .map_err(Error::MixedRelocationExpressions)
        })
    }
    /// Same-owner join to the existing outer V3 handoff. Exact original inventory,
    /// preflight, invocation, ABI and LLVM bytes are required, in addition to all
    /// composed content fields. This does not admit other proof/currentness gates.
    pub(crate) fn check_mixed_lineage_handoff_v29(
        &self,
        executed: &Executed<'_, 'a, 'v, 'v, 'v, 's>,
        handoff: &Handoff,
        budget: &mut Budget<'_>,
    ) -> Result<Identity, Error> {
        self.with_lineage(
            executed,
            handoff.canonical_bytes().len(),
            budget,
            |budget| {
                // The predicated family carries the complete descriptor and
                // retained module suffix, not the historical nominal V3 projection.
                if COMPLETE_VERSIONED_HANDOFF {
                    let floor = budget.storage();
                    let capture = (self, executed, handoff);
                    let scratch = [
                        std::mem::size_of_val(&capture),
                        std::mem::align_of_val(&capture),
                        std::mem::size_of_val(&AssertUnwindSafe(capture)),
                        size_of::<StaticJoined>(),
                        align_of::<StaticJoined>(),
                        size_of::<Result<StaticJoined, Error>>(),
                        size_of::<Result<(), Error>>(),
                        size_of::<Result<Identity, Error>>(),
                        size_of::<std::thread::Result<Result<Identity, Error>>>(),
                    ]
                    .into_iter()
                    .try_fold(0usize, |n, part| n.checked_add(part).ok_or(Resource::Arithmetic))?;
                    return budget.with_prepaid_scope(floor, 0, 0, scratch, move |budget| {
                        let (candidate, executed, handoff) = std::convert::identity(capture);
                        let joined = ExecutedProtectedMixedPublicationV29::new(
                            candidate, executed, budget,
                        )?;
                        joined.check_strict_handoff_v53(handoff, budget)?;
                        executed
                            .$replay_capsule(handoff.capsule().receipts(), budget)
                            .map_err(Error::MixedRelocationExpressions)
                    });
                }
                let capsule = handoff.capsule();
                let module = handoff.module_handoff();
                budget.charge_work(2 * fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3)?;
                if capsule.invocation() != self.invocation(budget)?
                    || module.kind() != CompilerModuleKindV1::LlvmTextIr
                {
                    return Err(mismatch(
                        "original protected invocation or LLVM module kind",
                    ));
                }
                let bindings = self.prepared.bindings(budget)?;
                let worker = self.prepared.worker(budget)?;
                let receipts = capsule.receipts();
                for (actual, expected) in [
                    (
                        receipts.rustc_identity_inventory().canonical_preimage(),
                        bindings.rustc_identity_inventory.canonical_transcript(),
                    ),
                    (
                        receipts.rustc_preflight_plan().canonical_preimage(),
                        bindings.rustc_preflight_plan.canonical_transcript(),
                    ),
                    (
                        receipts.abi().canonical_preimage(),
                        worker.descriptor(budget)?.canonical_bytes(),
                    ),
                    (module.module_bytes(), worker.llvm_ir(budget)?.as_bytes()),
                ] {
                    equal(actual, expected, budget)?;
                }
                executed
                    .$replay_capsule(receipts, budget)
                    .map_err(Error::MixedRelocationExpressions)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_lineage_finalizer_join_has_distinct_private_owners_and_exact_frames() {
        type F = fn();
        type R = [u8; 17];
        type Public = ProtectedMixedPublicationV28<'static, 'static, 'static>;
        type Execution = Executed<'static, 'static, 'static, 'static, 'static, 'static>;
        assert_eq!(
            size_of::<StaticJoined>(),
            size_of::<(&Public, &Execution)>()
        );
        assert_eq!(
            size_of::<StaticFinalized>(),
            size_of::<(&StaticJoined, &StaticContent)>()
        );
        let expected = size_of::<(&StaticJoined, &StaticContent)>()
            + align_of::<(&StaticJoined, &StaticContent)>()
            + size_of::<(&StaticJoined, FirstBuild, Pending<F>)>()
            + align_of::<(&StaticJoined, FirstBuild, Pending<F>)>()
            + size_of::<AssertUnwindSafe<(&StaticJoined, FirstBuild, Pending<F>)>>()
            + size_of::<(&StaticFinalized, &mut Pending<F>)>()
            + align_of::<(&StaticFinalized, &mut Pending<F>)>()
            + size_of::<((&StaticFinalized, &mut Pending<F>), &mut Budget<'_>)>()
            + align_of::<((&StaticFinalized, &mut Pending<F>), &mut Budget<'_>)>()
            + size_of::<AssertUnwindSafe<((&StaticFinalized, &mut Pending<F>), &mut Budget<'_>)>>()
            + size_of::<StaticContent>()
            + size_of::<Result<StaticContent, ContentError>>()
            + size_of::<Result<R, Error>>()
            + 2 * size_of::<std::thread::Result<Result<R, Error>>>()
            + 3 * size_of::<Result<(), Error>>()
            + size_of::<Result<(), ContentError>>()
            + size_of::<Result<Identity, Error>>()
            + size_of::<Identity>()
            + size_of::<Result<&Artifact, ContentError>>()
            + size_of::<Result<&Artifact, Error>>();
        assert_eq!(finalize_frames::<R, F>().unwrap(), expected);
        assert_ne!(
            std::any::TypeId::of::<StaticFinalized>(),
            std::any::TypeId::of::<StaticContent>()
        );
        assert_ne!(
            std::any::TypeId::of::<StaticJoined>(),
            std::any::TypeId::of::<Execution>()
        );
    }

    #[test]
    fn mixed_lineage_finalizer_content_resources_keep_exact_typed_refusal() {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(2);
        let mut budget = Budget::new(&mut work, 0);
        let error = budget.charge_work(3).unwrap_err();
        assert!(matches!(
            content_error(ContentError::Resource(error)),
            Error::Resource(Resource::Work(error)) if error.actual() == 3 && error.limit() == 2
        ));
        assert!(matches!(
            content_error(ContentError::Binding),
            Error::MixedPublication(MixedPublicationErrorV28::$lineage_error(
                ContentError::Binding
            ))
        ));
    }

    #[test]
    fn mixed_lineage_handoff_exact_field_join_rejects_mutation_and_one_short_work() {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(3);
        let mut budget = Budget::new(&mut work, 0);
        equal(b"abc", b"abc", &mut budget).unwrap();
        assert_eq!(budget.work(), 3);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(2);
        let mut budget = Budget::new(&mut work, 0);
        assert!(
            matches!(equal(b"abc", b"abc", &mut budget), Err(Error::Resource(Resource::Work(e)))
            if e.actual() == 3 && e.limit() == 2)
        );
        for changed in [&b"xbc"[..], &b"abcx"[..], &b"ab"[..]] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10);
            let mut budget = Budget::new(&mut work, 0);
            assert!(matches!(
                equal(b"abc", changed, &mut budget),
                Err(Error::MixedPublication(MixedPublicationErrorV28::Binding(
                    _
                )))
            ));
        }
    }
}

    };
}
