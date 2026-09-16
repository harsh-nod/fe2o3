// Prospective descriptors only. Compiled failures and restoration are not yet qualified.
const path = 'crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs';
const oracle = 'crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/';
const prefix = 'queue::dispatch_binding::control_release::tests::persistent::';
const tests = Object.fromEntries(Object.entries({
  A: 'persistent_success_preserves_exact_mixed_data_and_forward_cleanup',
  B: 'persistent_generation_errors_precede_cardinality_and_capacity',
  C: 'persistent_cardinality_and_capacity_reject_before_disposal',
  D: 'persistent_zero_data_uses_explicit_readiness_and_one_shot_transfer',
  E: 'persistent_native_errors_return_data_while_panics_retain_it_at_every_control',
  F: 'persistent_currentness_failures_preserve_all_eighteen_control_boundaries',
  G: 'persistent_partial_unmap_returns_data_without_losing_original_control',
  H: 'persistent_projection_and_actual_commit_failures_preserve_receipts_and_output_split',
  I: 'persistent_incomplete_callbacks_return_data_but_never_claim_control_completion',
  J: 'persistent_wrong_wrappers_and_repeated_extraction_reject_before_effects',
  K: 'persistent_genuine_single_and_three_binding_preparation_preserve_metadata_and_data',
  L: 'persistent_output_capacity_survives_success_and_error_extraction',
  M: 'persistent_panic_root_rejects_extraction_without_mutating_retained_custody',
}).map(([key, value]) => [key, prefix + value]));
const equality = 'assertion `left == right` failed';
const phase = 'assertion failed: matches!';
const output = 'persistent output lost identity, order or initialization';
const unwrapPhase = 'called `Result::unwrap()` on an `Err` value: ResourcePhase';
const activeLost = 'persistent failure lost active control';
const mutations = [];
function patch(from, old, replacement = '') {
  if (!old || from.split(old).length !== 2) throw new Error('ambiguous inner replacement');
  return [from, from.replace(old, replacement)];
}
function add(name, alias, line, edits, expected = equality, file = 'persistent_tests.rs') {
  mutations.push({name: 'persistent-' + name, kind: 'production', test: tests[alias],
    oracle_path: oracle + file, oracle_line: line, expected: [expected], patches: [{path, edits}]});
}
const constructor = '            data: owner.data,\n            data_premises: owner.data_premises,\n';
add('drop-constructor-data', 'A', 255, [patch(constructor, 'data: owner.data', 'data: Vec::new()')],
  'constructor changed original owner', 'tests.rs');
add('drop-constructor-premises', 'A', 255, [patch(constructor, 'data_premises: owner.data_premises', 'data_premises: Vec::new()')],
  'constructor changed original owner', 'tests.rs');

// Split shared arms so these three variants alter persistent modes only.
const beforeArm = `            ReturningControlModeV1::ReturningDestroy
            | ReturningControlModeV1::PersistentBeforePublication => {
                Some(self.generation.returning_destroy_generation()?)
            }
`;
const retainedBefore = `            ReturningControlModeV1::ReturningDestroy => {
                Some(self.generation.returning_destroy_generation()?)
            }
`;
add('before-uses-returned-generation', 'A', 205, [[beforeArm, retainedBefore +
  '            ReturningControlModeV1::PersistentBeforePublication => {\n' +
  '                Some(self.generation.returned_generation()?)\n            }\n']], unwrapPhase);
const afterArm = `            ReturningControlModeV1::AfterRecycle
            | ReturningControlModeV1::PersistentAfterRecycle => {
                Some(self.generation.returned_generation()?)
            }
`;
add('after-uses-destroy-generation', 'B', 265, [[afterArm,
  '            ReturningControlModeV1::AfterRecycle => {\n' +
  '                Some(self.generation.returned_generation()?)\n            }\n' +
  '            ReturningControlModeV1::PersistentAfterRecycle => {\n' +
  '                Some(self.generation.returning_destroy_generation()?)\n            }\n']], phase);
add('swallow-before-generation-error', 'B', 265, [[beforeArm, retainedBefore +
  '            ReturningControlModeV1::PersistentBeforePublication => {\n' +
  '                Some(self.generation.returning_destroy_generation().unwrap_or(0))\n            }\n']], phase);
const cardinality = `            if self.data.len() != self.data_premises.len() {
                return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                    index: self.data.len().min(self.data_premises.len()),
                    detail: "retained data/premise cardinality",
                });
            }
`;
const start = '        self.started = true;\n        let generation = match self.mode {\n';
add('cardinality-before-generation', 'B', 265, [[start,
  '        self.started = true;\n' + cardinality + '        let generation = match self.mode {\n']], phase);
add('omit-cardinality', 'C', 295, [[cardinality, '']], phase);
const reservation = `                self.persistent_returned
                    .try_reserve_exact(capacity)
                    .map_err(|_| Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                        operation: "persistent dispatch data",
                    })?;
                if self.persistent_returned.capacity() < self.data.len() {
                    return Err(Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                        operation: "persistent dispatch data",
                    });
                }
`;
const release = '        let release = self.release_controls(memory);\n';
const lateReservation = release +
  '        if let PersistentOutputStateV1::Prepared(_) = self.persistent_output {\n' +
  '            let capacity = self.data.len();\n            #[cfg(test)]\n' +
  '            let capacity = self.return_capacity_override.unwrap_or(capacity);\n' + reservation + '        }\n';
add('late-reservation', 'C', 312, [[reservation, ''], [release, lateReservation]]);
add('reserve-ordinary-output', 'A', 78, [[reservation, reservation.replaceAll('persistent_returned', 'returned')]],
  'persistent cleanup allocated ordinary output');
const taken = '        self.persistent_output = PersistentOutputStateV1::Taken;\n';
add('replace-preallocated-output', 'L', 907, [[taken, taken +
  '        let replacement = Vec::with_capacity(self.persistent_returned.capacity());\n' +
  '        self.persistent_returned = replacement;\n']],
  'persistent extraction changed preallocated output storage');
const ready = `        if let PersistentOutputStateV1::Prepared(generation) = self.persistent_output {
            self.persistent_output = PersistentOutputStateV1::Returnable(generation);
        }
`;
add('returnable-before-callback', 'M', 935, [[ready, ''], [release, ready + release]],
  'panic root must not authorize persistent extraction');
add('propagate-error-before-returnable', 'E', 59, [[release, release + '        release?;\n'],
  [ready + '        release?;\n', ready]], output);
add('omit-taken', 'A', 87, [[taken, '']], phase);
const extract = `        let PersistentOutputStateV1::Returnable(generation) = self.persistent_output else {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        };
`;
add('reject-empty-data', 'D', 336, [[extract,
  '        if self.data.is_empty() {\n' +
  '            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);\n        }\n' + extract]], unwrapPhase);
const extraction = '        for (authority, premise) in self.data.drain(..).zip(self.data_premises.iter()) {\n';
add('reverse-output', 'A', 59, [patch(extraction, '.zip(self.data_premises.iter())', '.zip(self.data_premises.iter()).rev()')], output);
add('truncate-output', 'A', 59, [patch(extraction, '.zip(self.data_premises.iter())', '.zip(self.data_premises.iter()).take(1)')], output);
const conversion = `            self.persistent_returned
                .push(dispatch_data_from_authority_v1(
                    authority,
                    premise.fully_initialized,
                ));
`;
add('invert-initialization', 'A', 59, [patch(conversion, 'premise.fully_initialized', '!premise.fully_initialized')], output);
add('revive-content-descriptor', 'A', 59, [[conversion, `            let data = match (authority, premise.initialized_content) {
                (DispatchDataAuthorityV1::Device(authority), Some(content)) => {
                    let initialized =
                        Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                            authority.into_lease(),
                            content,
                        )
                        .expect("retained fixture content has the exact extent");
                    Gfx942FixedDispatchDataV1::initialized(initialized)
                }
                (authority, None) => {
                    dispatch_data_from_authority_v1(authority, premise.fully_initialized)
                }
                (_, Some(_)) => unreachable!("fixture has no host content descriptor"),
            };
            self.persistent_returned.push(data);
`]], output);
add('drain-retained-premises', 'A', 71, [patch(extraction, 'self.data_premises.iter()', 'self.data_premises.drain(..)')]);
add('discard-error-output', 'E', 59, [['                Err((error, data))\n',
  '                drop(data);\n                Err((error, Vec::new()))\n']], output);
const wrapper = `pub(super) fn release_persistent_with_v1(
    mut root: ReturningControlCleanupCustodyV1,
    memory: &mut impl PristineControlReleaseV1,
    retain: impl FnOnce(ReturningControlCleanupCustodyV1),
) -> Result<
    (u64, Vec<Gfx942FixedDispatchDataV1>),
    (Gfx942DispatchBindingErrorV1, Vec<Gfx942FixedDispatchDataV1>),
> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if !matches!(
            root.mode,
            ReturningControlModeV1::PersistentBeforePublication
                | ReturningControlModeV1::PersistentAfterRecycle
        ) {
            return Err((Gfx942DispatchBindingErrorV1::ResourcePhase, Vec::new()));
        }
        match root.release_in_place(memory) {
            Ok(()) => root
                .take_persistent_data()
                .map_err(|error| (error, Vec::new())),
            Err(error) => {
                let data = match root.take_persistent_data() {
                    Ok((_, data)) => data,
                    Err(_) => Vec::new(),
                };
                Err((error, data))
            }
        }
    }));
    match result {
        Ok(Ok(returned)) => Ok(returned),
        Ok(Err(error)) => {
            retain(root);
            Err(error)
        }
        Err(payload) => {
            retain(root);
            resume_unwind(payload)
        }
    }
}
`;
add('drop-error-root', 'E', 35, [patch(wrapper,
  '        Ok(Err(error)) => {\n            retain(root);',
  '        Ok(Err(error)) => {\n            drop(root);')], 'persistent cleanup lost its retained root');
add('drop-panic-root', 'E', 35, [patch(wrapper,
  '        Err(payload) => {\n            retain(root);',
  '        Err(payload) => {\n            drop(root);')], 'persistent cleanup lost its retained root');
add('replace-panic-payload', 'E', 178, [patch(wrapper,
  '            resume_unwind(payload)', '            resume_unwind(Box::new("substituted persistent panic"))')]);
add('omit-started', 'B', 272, [patch(start, '        self.started = true;\n')]);
const code = '        while let Some(code) = self.code.next() {\n';
add('reverse-code-cleanup', 'K', 874, [patch(code, 'self.code.next()', 'self.code.next_back()')]);
const active = `        let active = self.active_control.as_mut().expect("rooted active control");
        memory.release_control(active)?;
`;
add('unroot-active-control', 'E', 135, [[active,
  '        let mut active = self.active_control.take().expect("rooted active control");\n' +
  '        memory.release_control(&mut active)?;\n']], activeLost);
add('accept-incomplete-control', 'I', 30, [[`        if !active.is_complete() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
`, '']], 'persistent cleanup unexpectedly succeeded');
add('omit-persistent-wrapper-mode', 'J', 746, [[`        if !matches!(
            root.mode,
            ReturningControlModeV1::PersistentBeforePublication
                | ReturningControlModeV1::PersistentAfterRecycle
        ) {
            return Err((Gfx942DispatchBindingErrorV1::ResourcePhase, Vec::new()));
        }
`, '']], phase);
add('omit-returning-wrapper-mode', 'J', 269, [[`        if !matches!(
            root.mode,
            ReturningControlModeV1::AfterRecycle | ReturningControlModeV1::ReturningDestroy
        ) {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
`, '']], 'returning retry entered cleanup callback', 'tests.rs');
add('omit-detached-wrapper-mode', 'J', 269, [[`        if !matches!(root.mode, ReturningControlModeV1::DetachedPersistent { .. }) {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
`, '']], 'returning retry entered cleanup callback', 'tests.rs');
const sharedSourceGroups = [];
function repeat(original, executions) {
  const source = mutations.find(mutation => mutation.name === 'persistent-' + original);
  if (!source) throw new Error('missing repeated variant');
  const names = [source.name];
  for (const [suffix, alias, line, expected] of executions) {
    const name = source.name + '-' + suffix;
    mutations.push({...source, name, test: tests[alias], oracle_line: line, expected: [expected]});
    names.push(name);
  }
  sharedSourceGroups.push(names);
}
repeat('late-reservation', [
  ['native-panic', 'E', 113, 'failed.root.persistent_returned.capacity() >= before.data.len()'],
  ['commit-panic', 'H', 113, 'failed.root.persistent_returned.capacity() >= before.data.len()'],
]);
repeat('returnable-before-callback', [['empty-panic', 'D', 109, equality]]);
repeat('propagate-error-before-returnable', [['empty-error', 'D', 121, equality]]);
repeat('unroot-active-control', [
  ['currentness', 'F', 135, activeLost], ['partial-unmap', 'G', 135, activeLost], ['actual-commit', 'H', 135, activeLost],
]);
module.exports = {path, tests, mutations, sharedSourceGroups,
  expected: {executions: 37, distinctSources: 30, production: 37, helperCalibrations: 0},
  sourcePrerequisite: {path, sha256: '4170e056ef0313b27acf27645d9f3fa274cbd56be4b7c0e2ade1490757ed6628'},
  scope: 'Prospective persistent-control/data cleanup negatives; not compiled or native qualification'};
