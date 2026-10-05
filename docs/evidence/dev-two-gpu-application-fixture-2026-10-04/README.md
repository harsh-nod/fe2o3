# Two GPU Application Fixture

Base: `011d5baf535e635dc0cc650ec0b26e2eef45b948`.
The genuine conditional application now has an optional two-device execution
path. Host typechecking, strict Clippy, pure data-oracle tests and a fresh genuine
zero-argument admission campaign pass. **The hardware branch has not run.**
This checkpoint does not close native multi-GPU qualification, A3/A7, performance
parity or general formal verification.

## Implementation

- Keep the original device kernel and no-argument admission path unchanged.
  Hardware mode requires exactly two distinct nonzero `0x`-prefixed unique IDs;
  malformed inputs reject before consuming the inherited handoff.
- Open the existing generated-only multi-device native-peer backend. Check both
  requested identities and gfx942:xnack- targets, retain one remote artifact and
  use the owned current-thread engine for two 65-element fills (G128/WG64).
- Require each exact completion receipt before taking its charged typed result,
  then check every value. Both launches are activated before explicitly awaiting
  completions; this is not a physical-overlap claim.
- Retain caller-owned charged results through queued `write_staging_v1`, transport
  and successful shutdown. Stage the actual result bytes, initialize separate
  guarded sentinel destinations, and use ordinary async uploads followed by both
  native peer-copy directions. Require no rejected observations, successful
  completion, exact native-copy counts one then two, and full source/payload/guard
  readbacks. Release settled submissions and allocations explicitly.
- Revalidate the retained artifact, destroy streams, drain, inspect owned native
  shutdown, and require fully refunded result credits. Ordinary operation errors
  still pass through inspected shutdown. Uncertain native custody is not dropped
  or reported as quiescent. Emit the `fe2o3.genuine-two-gpu.v1` JSON record only
  after all success checks.

## Validation

- Four standard-library-only tests pass through the integration test source
  `crates/cargo-fe2o3/tests/conditional_native_case.rs`, compiled with the pinned
  rustc. They exercise the fixture's actual helpers: CLI arity/encoding/order,
  every fill element, every source/destination/guard byte, wrong extents, missing
  directions and a distinct-payload cross-source control. No proof or native
  authority is fabricated.
- `cargo-fe2o3 check --all-targets` and binding-only `clippy --all-targets -- -D
  warnings` pass. The initial check correctly rejected a target source changed
  during compilation; source was frozen and the rerun passed. These commands
  establish host compilation, not execution authority.
- Fresh installed-service genuine campaign: **1 passed, 0 failed**, exit 0;
  474.38 seconds in the test and 484.47 seconds including deployment/cleanup.
  Real selected-rustc compilation, issuer/anchor publication, Worker finalization,
  ordinary static host linking, FD195/current-record audit and retained conditional
  proof admission pass with the new GPU branch compiled into the application.
  Manager/coordinator continuity checks pass. The later coordinator quarantine
  message occurs during teardown; the owned empty proof scope and outer cgroup
  are drained and removed.
- Three native agents reviewed lifecycle, result/copy oracles and hardware-harness
  prerequisites. Review findings for rejected observation history and charged
  staging were fixed. Final review found no remaining code blocker. Broader
  frontend/runtime suites from the prior checkpoint were not rerun here.

CLI SHA256: `c959ad064c604e62a9b7a8b0c9e37b34a30f55dfe8278e54fe958161ad112e44`.
Backend SHA256: `2bfad4b0dcc64f4fba38a88dbb6123919da730a06d5867009397a7e0edc7102d`.
Both were unchanged and held fixed throughout the genuine campaign.
The archive contains commands, environment, logs, source patch and next-step
review notes, not compiled executables or downloaded third-party sources.

Archive: [qualification.tar.gz](qualification.tar.gz).
SHA256: `c9a9d6a41344ab23128e59139164ddef3325fa24ed278c3e297ab4a6cfb7d8cc`.

## Remaining Hardware Gate

Add a distinct `genuine-two-gpu` harness mode and ignored campaign, preserving the
zero-argument control. Resolve selected hardware IDs through existing checked
topology discovery, retain only their render nodes plus KFD across all four
namespace layers, and grant only necessary numeric GPU supplementary groups to
the application. Require the matching hardware JSON record with bounded stdout
capture that cannot wait indefinitely on descendant-held pipes.

The required MI300X service deployment is still unavailable as recorded in the
previous checkpoint. This checkpoint made no remote changes and ran no GPU workload.
After deployment, select freshly observed free devices and run the positive path
plus second-invocation/transfer-deadline failure controls. Neither a process exit
nor a timeout substitutes for native settlement.

The unchanged fills produce identical index payloads. Distinct destination
sentinels detect missing copies, while routing evidence comes from checked device
and allocation selection plus native retirement counters. This is not a
data-differentiated routing benchmark, throughput result or HIP/HSA comparison.
