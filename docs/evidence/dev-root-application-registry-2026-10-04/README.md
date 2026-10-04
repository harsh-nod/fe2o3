# Root Application Registration

Source: `f439ede594745372e3da7221cb0e901fc3a0bcb4`.

This completes the root registry and observation-handshake prerequisite on the
[multi-GPU critical path](../../runtime-multi-gpu-critical-path.md). It does not
activate production application startup or complete the ordinary two-GPU pipeline.

## Implementation

RegisterApplication carries the exact 840-byte binding and two original compiler
descriptors. AttachApplication references that same unexpired registration and
transfers the candidate proof peer plus original Cargo pidfd. Root duplicates the
original application process token without retaining another compiler-peer alias.
The independently bounded application table is installed before its response; a
missing, duplicated, mismatched or retired attachment cannot bind an issuer.
Pending installation responses recheck the exact compiler marker and original
process/socket continuity. Compiler-session removal does not retire a bound app.

Installation retains registration/containment custody, not yet observed inputs.
The application must first send authenticated Hello after claiming its descriptors.
Root then observes the exact original process, four inputs and proof counterpart.
The shared bounded codec binds Hello/Challenge/Accept/Ready to both fresh nonces
and the complete registration. The 840-byte registration layout is unchanged.

Root permits one initial observation per registry iteration. After exact Accept
and successful issuer binding, it writes/closes a separate 72-byte observation gate
before sending Ready. This prevents immediate ACK/unload from racing publication.
The supervisor-side move-only gate requires the exact record, terminal EOF and a
live original root. Installation or ordinary compiler readiness cannot complete it.
Temporary ACK writers and application-side proof aliases are not retained.

Startup failure contains the exact original application until pidfd-confirmed exit.
Confirmed EOF after registration retires observation-only custody without signaling
a still-running app. Neither EOF nor process exit proves GPU settlement. No proof
owner, native invocation authority, new publication lock or sandbox syscall is added.

## Qualification

| Check | Result |
| --- | --- |
| Runtime protocol unit tests | 42 passed |
| Broker unit tests | 209 passed; 17 default ignored |
| Client unit, binary and integration tests | 54 passed |
| Supervisor unit and integration tests | 61 passed; 4 default ignored |
| Cargo main, wrapper, proxy and fixture binaries | 444 passed; 5 default ignored |
| Protocol, broker, client and supervisor doc tests | 96 passed |
| Isolated root campaigns | 4 passed |
| Five-package all-target Clippy | Passed with `--no-deps -- -D warnings` |
| Scoped formatting, whitespace, frozen hashes and signed-source audit | Passed |

Total: 910 passed, excluding nested helper reruns. The 26 default-ignored tests
include helpers exercised by parent tests and the four separately enabled root
campaigns. This is not a claim that every ignored deployment test was enabled.

The root session campaign checks authenticated pre-ACK success, Accept-before-Bind,
gate-before-Ready ordering, ACK EOF, normal proof closure without signal, wrong
Hello, extra rights, stale Accept, early EOF, timeout, changed proof flags, and
duplicate Accept. Registry transition cases check attachment, independent lifetime,
lost markers, retirement and expiry, plus swapped descriptors and wrong parent
pidfds. A real root/supervisor wire test exercises both registration exchanges and
proves installation alone cannot complete the gate. The original cross-UID
application-observation campaign also passes against the extended C fixture.

Root tests run in private PID/IPC/UTS/network namespaces with a read-only host
filesystem, private `/tmp`, and only the listed credential/inspection/containment
capabilities. They use real original process handles and UID1000 subprocesses,
but not a measured deployed supervisor or the final no-fork host bootstrap.

An initial dependency-inclusive Clippy attempt encountered the existing
`derivable_impls` warning in `fe2o3-semantic-import/src/profiler_bundle.rs`; that
unrelated file was not changed. A broad Cargo integration build was stopped before
test execution to exclude unrelated GPU/deployment builds. The completed command
explicitly selects the Cargo binaries and four client/supervisor integration targets.
Those broader Cargo integration tests are not claimed here.

## Remaining Work

Activate one atomic production handoff dispatcher and seal the returned compiler
registration plus gate into one private application route before launch. The current
low-level tuple is not itself a typed association; never accept independently
supplied or swapped gate owners in the readiness path. Gate failure must clean up
the bound issuer as well as prevent Cargo readiness: root application containment
alone is not the production supervisor's issuer cleanup contract.

Implement the app-side poll-only root-pidfd bootstrap and pre-ACK exchange, then
qualify the complete wire path through issuer readiness and Cargo ACK. Next deploy
the fixed keyless retained-proof custodian and consuming native invocation join,
then execute fill and guarded bidirectional XGMI using two selected MI300X devices.
No new formal machine proof, GPU result, HIP/HSA parity or performance claim is made.

## Evidence

`evidence.tar.gz` contains commands/logs, source patches, 5,090 frozen source/config
hashes, 19 selected test-binary hashes, signed-source audit and an internal manifest.
Temporary evidence staging is removed after archive verification. The target cache
is retained. No MI300X processes, GPU allocations or temporary files were created.

Source patch SHA256:
`18f6cc3feb1a7eedef3011447c83f94a3db86693af3551a0ecfd20bb57d5ea8b`.

Archive SHA256:
`6db06550b20e978c9311b43e7260988da07c35e224f9dbb0007530d8c4b9a35c`.
