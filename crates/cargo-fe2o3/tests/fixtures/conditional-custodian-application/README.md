# Genuine Conditional Application Fixture

This standalone package selects a freshly compiled write-only u32 index-fill
kernel. Its host entry consumes Cargo's inherited application-custodian handoff,
requests production compiler-currentness auditing and a retained conditional
proof, and revalidates the resulting remote artifact. It imports no captured
binding, compiler receipt, proof receipt, or application handoff.

Run through the `genuine` campaign in
`scripts/qualify-proof-resource-inspection.sh`, using the real installed compiler
and proof-service entrypoints. Run Cargo from this package's directory: the local
`.cargo/config.toml` supplies static GNU-host linking, and `--manifest-path` alone
does not make Cargo discover that configuration. Production supplies explicit
device and host targets, so the static flags do not affect host proc macros or
the AMDGPU compilation. The device phase selects the package library, whose
AMDGPU branch is `no_std`; the host phase selects the ordinary binary. AMDGPU
does not support a binary crate target.

The zero-argument genuine campaign checks installed compiler/currentness,
ordinary host binding and retained proof admission. It performs no GPU work.
The current integrated source, service binaries and deployment must be qualified
together; results from an earlier source tree do not qualify this integration.

The optional hardware mode accepts exactly two distinct, nonzero GPU unique IDs
after Cargo's `--`, each written with a `0x` prefix. These are hardware IDs, not
HIP ordinals or render-node indices. Missing hardware never silently selects the
admission-only mode. Hardware qualification requires the installed services and
KFD plus the selected render nodes in the application's private namespace.

The separate `genuine-two-gpu` campaign provides that namespace transport and
validates the hardware success record. From the repository root, in the real-root
qualification environment with the same pinned inputs as `genuine`, select two
freshly observed free devices and run:

```bash
FE2O3_PROOF_INSTALL_CAMPAIGN=genuine-two-gpu \
FE2O3_GENUINE_GPU_UID0="${GPU_UID0:?}" \
FE2O3_GENUINE_GPU_UID1="${GPU_UID1:?}" \
bash scripts/qualify-proof-resource-inspection.sh
```

The selector observes topology and both directed routes before creating the
private scope. It rechecks selected identities, exact character nodes and Unix
mode permissions after namespace transport and immediately before Cargo.
Application UID1000 is unchanged; only necessary numeric GPU groups are added.
ACL and device-cgroup access are still enforced by actual native opens. Selected
render mounts do not make process-global KFD a two-GPU security boundary.

The harness bounds captured stdout to 1 MiB and does not wait for descendant pipe
EOF after its direct child exits. Successful child status and exactly one matching
typed JSON record are required; missing, duplicate or mismatched records reject.
The ordinary `genuine` campaign remains admission-only. CPU tests and read-only
topology selection do not qualify the complete protected hardware campaign.

The hardware path retains one remote artifact across two conditional invocations
of the unchanged device kernel (65 elements, grid 128, workgroup 64). Exact
completion receipts gate charged result extraction. It stages the actual completed
bytes into PUBLIC allocations, copies both peer directions, requires two native
XGMI retirements, and checks every source, destination and guard byte. It then
revalidates the retained artifact, drains, inspects owned shutdown and verifies
result-credit refund before emitting the `fe2o3.genuine-two-gpu.v1` JSON record.
An operation timeout or quarantined shutdown is failure, not successful settlement.

`tests/conditional_native_case.rs` exercises the fixture's actual parser and data
oracles without constructing proof or native authority. These tests and host
typechecking do not qualify GPU execution. The two fills produce identical index
payloads; distinct destination sentinels reject missing copies, but this is not a
data-differentiated routing benchmark or a physical-overlap measurement. Full
HIP/HSA parity and performance remain separate gates.

## Explicit Device Roster

The standalone application also accepts `--roster UID0 UID1 ...` after Cargo's
application separator, with exactly two through eight distinct, nonzero `0x`
hardware unique IDs. There are no roster control-mode suffixes or automatic
device discovery. The requested order must match every device admitted by the
runtime context; missing, extra, reordered or differently targeted devices fail.
The native constructor independently admits every selected gfx942 device and
every directed route before opening its execution path. It does not admit gfx950.

The existing `genuine-two-gpu` namespace harness remains pair-only. The separate
`genuine-device-roster` campaign accepts the explicit JSON environment roster:

```bash
FE2O3_PROOF_INSTALL_CAMPAIGN=genuine-device-roster \
FE2O3_GENUINE_GPU_ROSTER='["0x0000000000000001","0x0000000000000002"]' \
bash scripts/qualify-proof-resource-inspection.sh
```

The example UIDs are placeholders, not a device selection. Use only freshly
observed, available devices and the same pinned compiler/proof inputs as the
genuine campaign. The harness observes every selected route, transports the
exact KFD/render nodes, then rechecks identities and character-node permissions
inside the private namespace and immediately before Cargo. It retains the fresh
observed host UID inventory and explicitly names unselected UIDs. Occupancy is
not measured by this adapter: an unselected UID is not asserted to be busy, and
no all-host-qualified record is emitted. Actual all-host evidence must separately
compare fresh admission and occupancy observations against the selected roster.
Merely supplying a
subset on this CLI does not establish coverage of every GPU in a host. Within the
application, coverage means every device in the exact admitted context roster.
Neither CPU oracle tests nor an admission-only run qualify this hardware path.

Each device receives a separate charged generated fill, with `65 + ordinal`
elements, grid 128 and workgroup 64. Every result is extracted through its own
completion receipt and checked in full. Host staging prefixes those actual result
bytes with the source UID, shard ordinal and extent. These tags are explicitly
host-authored routing data, not kernel output or additional proof authority. They
make wrong-source routing distinguishable despite the unchanged index-fill kernel.

The campaign checks all `N * (N - 1)` directed copies, including 56 edges for eight
devices. It resets a route-specific guarded destination before each edge, requires
an exact increment of the native peer-completion counter, then checks the full
source and destination before continuing. It retains all charged results through
artifact revalidation, drain, owned shutdown and complete result-credit refund.
Only then does it emit one `fe2o3.genuine-device-roster.v1` record containing the
exact UID roster, per-device extents and complete directed-pair list. Pair-mode
arguments, controls and records remain unchanged.

All compute completions precede the first peer submission; peer copies settle
serially. The roster record reports this logical ordering and separately declares
`copy_compute_overlap: "not-measured"`. It is not copy-engine, physical-overlap,
throughput or HIP/HSA performance evidence.

## Single-Device Receipt Coexistence

The separate, opt-in Cargo feature `receipt-coexistence` enables the application
selector `--receipt-coexistence 0xUID`. It requires one explicitly selected,
freshly admitted and available gfx942 device under the same protected inherited
handoff and device-node custody. The existing pair/roster scripts do not select
this mode implicitly. It uses the legacy conditional-fill artifact adapter, not
the separate native V5 end-to-end artifact path.

This case warms independent copy storage and a guarded destination, reserves one
65-element charged generated fill, arms the bounded runtime witness, then
publishes a 4096-byte host-to-device copy on another stream. It deliberately
retains that original copy receipt without polling it while the generated fill
publishes and completes. Success requires the runtime's exact second-publication
witness with original disjoint native owners. Missing, ambiguous or overflowed
capture is a failure; host timestamps or queued futures are never substitutes.

After taking the witness, the case settles the copy, verifies the entire fill,
copy source, payload and both guard regions, revalidates the artifact, and requires
quiescent drain, owned shutdown and full result-credit refund. Only then is a
`fe2o3.genuine-receipt-coexistence.v1` record emitted. Its scope is one selected
device. It reports `physical_overlap: "not-measured"`: either GPU operation may
already have finished while its receipt remains retained. This is not physical
engine overlap, all-device coverage, throughput or HIP/HSA parity evidence.
# Native V5 Qualification Entry

The separate `native-conditional-fill` binary is selected only with
`--native-application-proof-custodian --bin native-conditional-fill`. Its application
arguments are `--native-v5 --producer-source <exact-rustc-library-source-spelling>
--device 0x<16-lowercase-hex-digits>`. No captured legacy artifact is accepted.

It opens independently installed native compiler/proof profiles, consumes the
original inherited native slots, and requires actual currentness, ACK and retained
proof exchanges before preparing two gfx942 closed-profile launches on independent
streams. The selected device must already be reserved by the qualification runner.
Normal success requires exact 64- and 37-element outputs, quiescent native teardown
and refunded result credits. Its distinct `fe2o3.native-conditional-fill.v1` report
binds the actual proof/source/artifact identities. This source is not evidence of a
hardware pass, physical overlap, throughput, all-host-device coverage or gfx950
production admission. Legacy pair, roster and coexistence entrypoints are unchanged.

The separate `native-conditional-fill-roster` binary uses the same genuine native
bootstrap and original account. Select it with the native custodian flag and pass
`--native-v5-roster --producer-source <exact-source> --transport native-xgmi
--devices <uid> <uid> ...` with two through eight distinct canonical UIDs.
`--transport host-staged` explicitly selects bounded staging with native peer
routes disabled; it is a separate campaign, never a native fallback success.

Each selected admitted child produces a distinct 32..39-element output through
the lexical native scope. The campaign then visits every directed pair serially,
checking the tagged source, entire guarded destination and native-completion
counter after each copy. Reports use `fe2o3.native-conditional-fill-roster.v1` and
name the transport. A complete selected roster is not an all-host claim: the
external runner must independently archive fresh topology, admission and occupancy
to identify any busy/excluded devices. No compute/copy overlap is implied.
