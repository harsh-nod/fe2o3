# Bounded CPU loaded-input adapter

Explicit CPU-only APIs for a guarded precheck → historical two-pass read → postcheck. Module initialization makes no application filesystem/provider call and starts no child; Node loader IO is separate. The operational entry remains unactivated: this package supplies mechanisms, not a current scope, approved cap, complete source graph, or execution authority.

The package uses the adjacent loaded-input-reader and loaded-profile modules. It does not ship the historical adapter-plan bridge, historical controls, host-specific graph seed, fixture data, or raw host receipts.

## APIs

- executeAdapterPlan(planBuffer, {provider, guard, now}) in adapter-protocol.mjs admits a complete bounded UTF-8 JSON Buffer and both reader protocols before operational IO. The explicit provider implements lstat, realpath, readlink, open, fstat, read and close. Both the guard and clock are caller supplied.
- createFiniteGuard(policyBuffer, {monotonicNow, utcNow, resources}) in adapter-guard.mjs requires a finite closed JSON policy: UTC bounds, elapsed deadline, RSS/free-disk/available-RAM limits and a bounded resource-sampling schedule. Denial is sticky. createBoundedResourceObserver accepts explicit clocks, scope, resource root and an injected resource provider; its attempted/invoked counters are separate from selected-file IO.
- admitEvidenceSpec(specBuffer), encodeBoundedEvidence(value, capBytes), and publishExclusiveEvidence(bodyBuffer, specBuffer, provider, {guard}) in adapter-writer.mjs expose explicit output admission, bounded plain-JSON serialization and exclusive publication.
- runFilesystemAdapter(planBuffer, guardPolicyBuffer, writerSpecBuffer, {resource_root}) in adapter-fs.mjs explicitly binds the filesystem providers. It reads no implicit request filename or environment configuration. Root must separately admit and pin these three complete Buffers and the launcher that supplies them.

The one/two/one reader phases retain separate exact byte/content/metadata ceilings and finite deadlines. The existing reader enforces exact-or-refuse 64 KiB chunks, whole hashes, EOF, identity checks, bounded nonfile absence and independent alias-target duties.

All authority flags remain false. An accepted cap value is not approval to raise a cap. No native/GPU scope is granted.

## Failure, cleanup and output limits

The first reader, provider, phase-clock or guard failure remains primary through later cleanup/publication errors. Later reader phases stay explicitly not-started with their ceilings intact. Full initial named-target/descriptor agreement includes ownership even when historical ownership is unconstrained.

After expiry, only already-owned descriptor cleanup is permitted. Failed cleanup retains possible-live-descriptor state. Failed phases report bounded diagnostics/counters, not complete observation arrays. Resource failures, including non-Error throws, are sticky; attempted reservations and invocations remain distinct.

Evidence serialization has an explicit maximum 64 MiB body, structural bounds and oversized-output refusal before writer IO. The writer creates a new temporary leaf exclusively, then uses an atomic no-replace hard link to a separate final leaf. It never uses overwrite-capable rename or automatic unlink. Even success retains the temporary link; root owns later cleanup.

Temporary/final names need separate reservations: up to 128 MiB before external receipts/streams/readback. Partial/displaced custody and cleanup errors remain reported. Root must bind a private directory; bracketing is not atomic ancestor or writer exclusion. Acceptance checks currentness after file close; the result includes post-publication guard state.

The serialized record describes publication intent, not its own successful publication. Root must inspect actual command status and completely read back the output. Expiry forbids new evidence writes. A kill may prevent cleanup or any report; unwritten output is never called available.

## Controls

Run the fixture-free controls from the repository root:

    node --test tools/debugger/loaded-cpu-adapter/adapter-controls.test.mjs

There are 76 authored fixture-free controls using only injected memory providers. They import neither the operational filesystem entry nor a fixture reader. This README does not claim a root execution or qualification result; that requires a separate complete root-owned receipt and source audit.

A further 10 historical bridge controls remain external to this package. They require the explicit bounded 76-role FE2O3_LOADED_REVIEW_FIXTURES manifest, with missing configuration/data failing rather than skipping. They are not included in the 76 fixture-free count and do not establish operational currentness or native acceptance.

Before any filesystem activation, root must bind the final individually pinned static source/import/runtime/loader/launcher graph, exact request Buffers, current CPU-only policy, finite process/resource ceilings, new output identities and external timeout/readback. Provider-level accounting is not a measurement of all OS loader or kernel IO; synchronous calls cannot be preempted by the in-process guard.
