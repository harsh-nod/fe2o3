# Binder, Producer Preflight and Retained Payload Qualification

The available CPU/static/bounded host-proof gates passed on signed source
`3d4ee2d91b0713bb1dfe207d16eede331125a7ab`, with 6,407 unchanged selected
source inputs. **Full runtime qualification did not pass:** three telemetry
tests failed socket inspection with `EPERM`. Their failures remain recorded;
later successful integration and documentation tests do not supersede them.
No A0-A7 exit, native qualification, performance gain or HIP/HSA parity is added.

## Implementation and Proof Boundaries

Template binding now shares the actual preflight, preparation, heap conversion,
roster projection and reservation composition with Verus over a non-Copy owner.
Projection avoids the explicit scratch generation-binding Vec and caches the
first binding while retaining exact transcript and first-error order. Binder
and roster campaigns each pass 64 overlapping obligations and 12 executable
logical negatives over the same 13-file proof closure. Four explicit Hash/SHA
transcript adapters, two standard-library Box conversion contracts and vstd
container contracts remain trusted. Neither SHA implementation/collision
resistance nor allocation failure, unwinding, Drop, compiler/ISA or native
authority is proved by this composition.

Producer-input preflight shares the actual read-only prefix over production
HashMaps, complete IDs, optional construction records, domains, families,
counts and flags. Checked arithmetic preserves short-circuit ordering. Its
campaign passes 15 obligations and 22 executable logical negatives under an
explicit `obeys_key_model` premise and standard/vstd library contracts. This is
not the complete Context journal/status reconciliation fold or a resource-credit
observation theorem.

Retained launch payloads check kernarg-plus-binding size and `isize` overflow,
reserve optional `ControlResidentBytes` credits before copying or native work,
and share one debit across Arc aliases. The last owner drops the retained slices
before refunding credit. Production deep cloning is removed; 11 CPU tests cover
the actual factory/driver handoff. This does not account for allocator, Arc,
dependency, metadata, Context, native or process-RSS overhead, nor qualify the
owning worker/flush path or an aggregate memory ceiling.

The no-retirement checkpoint path preserves initial validation and monotonicity
checks, then shares existing journals without rebuilding and revalidating them.
Eight focused oracle/frame/COW/watermark groups pass. The initial potentially
quadratic validation scan remains; no whole-method allocation-free or latency
claim is made. The R64 Tokio regression fixture now publishes dependencies,
Pending state and its observable issue count under the same mutex; production
semantics and timeouts are unchanged.

## Recorded Results

| Check | Result and scope |
| --- | --- |
| Complete KFD CPU | 1,779 passed, zero ignores or exclusions; historical `signed-final-2` execution explicitly reused |
| Fresh KFD rebuild/list | Exact same retained ELF and complete test roster; not another full execution |
| Runtime-model CPU | 1,088 passed; 19 named manual release-mode benchmarks ignored |
| Focused CPU groups | 1 timer, 11 payload, 8 checkpoint, 6 roster and 4 producer-input tests passed |
| Runtime library | **1,851 passed, 3 failed, 32 ignored; command status 101** |
| Supplementary integrations | 11 passed; 3 exact hardware/fixture ignores across six targets |
| Doctests | 124 passed |
| Protocol checks | 2 R66 Rust tests, 21 R66 Python and 12 owner Python groups passed |
| Static checks | Strict all-target/all-feature Clippy, formatting and whitespace checks passed |
| No-default production check | Cargo status 0 with two existing dead-code warnings; not warning-free qualification |
| Positive proof roots | Nine full roots passed, with their disclosed trust boundaries |
| Signed proof campaigns | 70 stages; all 46 logical negatives passed across three campaigns |
| Independent audit controls | 11 general, 6 prefix and 4 blocked-runtime groups passed |

The nine root counts are preparation 30, reservation 23, cancellation 16,
completion binding 55, rollback 51, dependency failure 21, preflight 46,
binder composition 64 and producer preflight 15. They overlap and must not be
summed as coverage. The no-default warnings are private fixture-only
`CompletionPacketTemplatesV1::from_array` and
`CompletionSignalArenaOwnerV1::bind_batch`; this packet does not change them.

The three failed library tests are
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal` and
`pre_native_telemetry_failure_is_returned_and_poisoned`, all under
`authorized_execution::tests`. They report `InspectSocket` with OS code 1 at
`authorized_execution.rs:1317:91`. The particular failed socket operation and
denial mechanism were not established. Checks were not bypassed and tests were
not skipped. Requalification in an environment supporting the required socket
inspection remains necessary.

The interrupted `signed-final-1`, model-ignore-policy-rejected `signed-final-2`
and runtime-failed `signed-rest-1` attempts remain rejected. Only the complete
KFD execution from `signed-final-2` is reused, after independent signature,
source, profile, feature, ELF and exact-roster validation. Development passes
are not promoted to a signed full-runtime pass.

## Evidence and Replay

`raw.tar.gz` is 58,741,275 bytes and contains 2,772 manifested files plus its
inner `SHA256SUMS`. Its SHA256 is
`3b798a2097a2145493bf8a779fb20476eed99b8eb07be5972fab91d1521e5e4f`.
The retained KFD ELF is 33,954,832 bytes, SHA256
`82316fcae7ec999acaa9b094eabc0f16bc8df42c0e69e4361babb73ada5e8b04`.

The partial audit joins 137 records: 118 static/campaign records, 18 historical
KFD-prefix records and one supplementary integration record. The archive's
318 process receipts additionally retain development, rejected and packaging
history; they are not 318 passing stages. Historical receipts are replayed as
recorded evidence, never as new host-wide process observations. The 118 fresh
static records and one supplementary record have their original same-recorder
PID-namespace closure observations. Only the five current packager children
were freshly observed by the still-live packager in its own PID namespace.

The corrected auditor pins all supplementary environment, plan, tool, helper
and namespace metadata before and after replay. Changed profiles, missing
files, byte drift and symlinks are rejected. The earlier package with the narrower
metadata audit remains retained locally and is not the published packet.
Fresh restoration, exact inventory/hash checks and relocated replay passed.
`restore-replay-collected.json` was durably written before removing only the
owned restoration; `restore-replay.json` records its absence and receipt hash.

After authenticating this packet's Git signature and extracting its archive:

```sh
python3 -I -B test-audit-static.py
python3 -I -B test-kfd-prefix.py
python3 -I -B test-runtime-blocked.py
python3 -I -B audit-static.py --repo /path/to/source-at-3d4ee2d9
```

Replay checks retained artifacts, not fresh builds, solvers or hardware. Current
selected source bytes must match the signed source. `--live-processes` refuses
because historical PID-namespace identity was not recorded. Tool provenance
is not a hermetic compiler/dependency-cache audit or machine-code proof.

Next gates include the unresolved runtime library tests, actual queued-query
and full Context reconciliation refinement, aggregate ownership accounting,
retained native-pair runtime integration, protected compiler/application
authority and matched performance qualification. Accepted Native R125,
Admission R118B C1/C2/C3 and Resources R116/V3 checkpoints remain unchanged.
