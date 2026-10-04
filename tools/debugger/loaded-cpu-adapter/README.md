# Bounded CPU loaded-input adapter

Explicit CPU-only APIs for a guarded precheck → historical two-pass read → postcheck. Module initialization makes no application filesystem/provider call and starts no child; Node loader IO is separate. The historical operational workflow remains unactivated: this package supplies mechanisms, not a current scope, approved cap, complete source graph, or execution authority.

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

There are 76 fixture-free controls using only injected memory providers. They import neither the operational filesystem entry nor a fixture reader. Root qualification passed those controls within the 202-control combined installed reader/profile/graph/adapter suite, and passed the 10 external historical controls separately. CPU receipt SHA-256: `1789ce1fd0cb0e0eea3787799e023a1a0e5110753b965250d4e241f61f068a41`. This is component evidence, not complete operational currentness or native qualification.

A further 10 historical bridge controls remain external to this package. They require the explicit bounded 76-role FE2O3_LOADED_REVIEW_FIXTURES manifest, with missing configuration/data failing rather than skipping. They are not included in the 76 fixture-free count and do not establish operational currentness or native acceptance.

Before any filesystem activation, root must bind the final individually pinned static source/import/runtime/loader/launcher graph, exact request Buffers, current CPU-only policy, finite process/resource ceilings, new output identities and external timeout/readback. Provider-level accounting is not a measurement of all OS loader or kernel IO; synchronous calls cannot be preempted by the in-process guard.

## Opt-in real-filesystem controls

The separate Linux fixture test is `filesystem-adapter-controls.mjs`. It is not
part of the default 76 memory-provider controls. Seven cases exercise fresh
regular files, an independently selected alias target, an empty file, bounded
absence, wrong hashes, no-replace publication, expired policy and bounded
serialization. Missing or wrong root configuration refuses instead of skipping.

Use a fresh, private, empty, canonical directory that you own. The following
binds its initial six stat fields before starting the test:

~~~sh
export FE2O3_ADAPTER_FS_TEST_ROOT="$(mktemp -d /tmp/fe2o3-adapter-test.XXXXXX)"
export FE2O3_ADAPTER_FS_TEST_ROOT_IDENTITY="$(node -e 'const fs=require("node:fs");const s=fs.lstatSync(process.env.FE2O3_ADAPTER_FS_TEST_ROOT,{bigint:true});process.stdout.write(JSON.stringify(["dev","ino","size","mode","mtimeNs","ctimeNs"].map(k=>String(s[k]))));')"
node --test tools/debugger/loaded-cpu-adapter/filesystem-adapter-controls.mjs
~~~

These opt-in controls use the MI350 qualification resource floors: at least
40 GiB free disk and 64 GiB available RAM, with a 512 MiB process-RSS ceiling.
They are not the portable low-resource default suite. Each case's finite guard
expires 60 seconds after policy creation, with a one-second not-before allowance,
a 30-second elapsed bound and at most 64 resource probes.
Use a separately bounded outer supervisor when qualifying a run; an in-process
guard cannot preempt every synchronous filesystem call.

The root identity is checked before fixture creation; short semantic role labels
do not depend on the root path length. Each case reserves at most 512 KiB per
evidence name (the aggregate-size refusal case uses 4 KiB). Seven cases create
seven 65,537-byte input bodies; the conservative evidence-name ceiling is 7 MiB.
This fixture setup and its complete output readback are separate from reader
provider accounting.

Root qualification passed all seven cases and the missing-root/wrong-identity
admission controls. Receipt SHA-256:
`42968ee118231c0746655cfbdcd8eaea3a131dace83159682185a90da20efb72`.
Root read back all seven output rosters and all 102,681 named output bytes.
An earlier fixture incorrectly expected the aggregate serializer bound at a
one-byte cap; the stricter key bound correctly refused first. That failed run
is preserved and is not counted as passing.

Tests retain their directories and print their names for inspection. Success
retains both hard-link names; refused publication preserves existing sentinels.
There is no automatic deletion. Reruns need a new empty root and fresh identity;
cleanup is a separate owner decision.

This invokes the filesystem binding only on test-owned synthetic data. It does
not activate historical input replay, prove ancestor/writer exclusion, renew
native coordination, start GDB or an inferior, dispatch a GPU kernel, or produce
physical register/LDS visualization samples. The two environment variables
configure this test harness only, not the production Buffer API.
