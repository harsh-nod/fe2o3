# Two GPU Qualification Harness

Base: `45fc4020b8c633e204a6f2d523ac327f8a3b5527`.
The genuine two-device fixture now has a distinct `genuine-two-gpu` qualification
campaign. CPU, namespace, live read-only MI300X selection and fresh genuine
admission checks pass. **The two-GPU execution campaign has not run.** This does
not close A3/A7, native multi-GPU qualification or HIP/HSA performance parity.

## Implementation

- Require two explicit, distinct, nonzero hardware UIDs. Use the existing checked
  topology discovery, correlate exact render nodes, and check both directed XGMI
  routes using kernel GPU IDs. Selection alone grants no native execution authority.
- Transport canonical hex UIDs without jq integer precision loss. Shell validation
  accepts only the exact schema and bounded numeric minors; fixed paths expose
  KFD and only two selected render nodes through all four genuine `/dev` resets.
- Revalidate selection against fresh topology before provisioning and immediately
  before Cargo. Check exact character-device metadata and derive only necessary
  numeric GPU groups for UID1000, including combined-group DAC precedence.
  Service/proof credentials and production authority APIs remain unchanged.
- Bound application stdout retention to 1 MiB, every read batch to 32 attempts,
  the application deadline to 1500 seconds, and the final drain by the earlier
  caller deadline or 100 ms. Do not await descendant-held pipe EOF. Existing
  direct-child ownership reaps on error; the private campaign cgroup owns descendants.
- Require successful child status before accepting exactly one strictly typed
  `fe2o3.genuine-two-gpu.v1` record with the ordered UIDs, 65 elements, two native
  peer completions and released shutdown. Missing, duplicate, escaped duplicate,
  malformed, foreign and mismatched records reject. Keep `genuine` admission-only.

## Validation

- Pinned nightly static-musl custodian unit suite: **38 passed, 12 ignored**, exit 0.
  New controls exercise UID/route selection, changed topology records, exact node
  types/numbers, group precedence, report validation, nonzero exit, output limits,
  continuously writing children, deadlines, EINTR and retained pipe writers.
- Strict scoped Clippy with `-D warnings`, scoped rustfmt, shell syntax checks and
  `git diff --check` pass. Shell parser controls include full-width UIDs, newline
  suffixes, duplicate records, invalid minors and path-like injection strings.
- Four nested real bwrap device resets preserve exactly the selected synthetic
  character-node aliases. This test uses private null/zero aliases and never
  passes them to GPU admission. No host device is created or modified.
- Full hardware-mode invocation on the local non-GPU host rejects missing KFD
  topology with exit 5, before cgroup creation. Before/after scope lists match;
  there is no fallback to ordinary admission.
- The same SHA256-checked static binary runs the read-only observer on MI300X as
  ordinary UID1002: **1 passed**, exit 0. UIDs `0x6ced1647a296545c` and
  `0xab83d2ffef0d3cdf` resolve to render minors 128 and 136. Both observed routes
  and selected-node metadata checks pass. No queue, kernel or copy was submitted;
  device idleness was not established. Its one owned `/tmp` directory and binary
  were removed, and a subsequent SSH check confirms absence.
- Fresh installed-service `genuine` regression: **1 passed**, exit 0; 447.28 seconds
  in the test, 458.15 seconds including isolated deployment and cleanup. Actual
  selected-rustc compilation, issuer/anchor publication, Worker finalization,
  ordinary host linking, FD195/current-record audit and retained conditional proof
  admission pass through the new stdout capture. Manager/coordinator continuity
  passes. The later coordinator quarantine message is teardown; the empty owned
  proof scope and outer cgroup are removed and absence is checked independently.
- Three native agents reviewed capture/report validation, device transport and
  credential/authority boundaries. Findings for escaped duplicate reports,
  post-exit deadline bounds and newline UID validation were fixed before the final
  tests. Final code review found no remaining issue. Broader frontend/runtime
  suites and hardware failure controls were not run for this harness-only change.

Sources and measured binaries were held fixed throughout the genuine campaign.
Static test SHA256: `fd0f2873595cfd0722af21407596bc5c0d96439faadefab4119eb0d1f621b463`.
CLI SHA256: `c959ad064c604e62a9b7a8b0c9e37b34a30f55dfe8278e54fe958161ad112e44`.
Backend SHA256: `2bfad4b0dcc64f4fba38a88dbb6123919da730a06d5867009397a7e0edc7102d`.

## Remaining Hardware Work

MI300X has the required compiler/tool paths and the four copied setup DSOs match
this host byte-for-byte. This is not a complete pinned deployment inventory.
Noninteractive sudo remains unavailable; the fixed fe2o3 services are absent.
Prepare the complete compiler/proof/offline-cache bundle and a usable real-root
service launch, without relaxing accepted runtime pins or native admission.

Next run the full fixture on two freshly observed free physical GPUs, then the
second-invocation and transfer-deadline failure controls. Require actual fills,
bidirectional peer retirements, complete payload/source/guard checks and inspected
native shutdown. Process death or cgroup cleanup is not GPU settlement.

DAC preflight does not prove ACL or device-cgroup access. KFD remains process-global;
the selected render mounts are not kernel-enforced isolation from other GPUs.
These tests are not a new whole-runtime formal proof, physical-overlap measurement,
scaling result or throughput comparison. The original fixture's identical fill
payload limitation remains unchanged.

The archive contains commands, environment, final logs, source patch and review
notes, not executables, private signing keys or third-party source archives.
Archive: [qualification.tar.gz](qualification.tar.gz).
SHA256: `4acb7d93b19ab7095cca112ab20510087d3e7c58d17e361e3ce5ee18ce66ff57`.
