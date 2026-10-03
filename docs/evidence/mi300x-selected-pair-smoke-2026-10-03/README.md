# Selected-Pair Multi-GPU Smoke

This checkpoint makes the existing production copy-only path repeatable on a
shared GPU host. It adds a selected-pair controller, not a new transport or
permission to launch application kernels. Qualification is against the candidate
above `8eb0ebcb954c5cf795f08ba4265da54681bf40c9`.

## Accepted Scope

All four cases pass on MI300X GPUs 6 and 7: native XGMI forward/reverse and
host-staged forward/reverse. Each case performs two changed-content
8,388,581-byte copies under production deny-all kernel authority, verifies every
source/destination byte and explicitly shuts down logical/native resources.

The total is eight logical copies, four native and four staged, 12 native SDMA
packets, 32 full-buffer readbacks and zero kernel launches. Native counters are
checked before publication and only advance at final logical completion. Staged
controls report zero native copies. These are correctness results, not throughput.

The current-source no-default-feature executable SHA-256 is
`19f00a1067363ea95e59e0da0074e001fd0237f68a527100d2c909d71413d9fd`.
The controller pins the ELF through an open descriptor, validates its full hash
and checks pathname identity afterward. Exact PASS fields, not merely exit zero,
are required for each selected direction and transport.

All 17 controller tests pass, including real local-process success, timeout,
output-limit and surviving-child cases. Process-group cleanup retains the leader
unreaped until census/signaling, never signals after reaping, and still performs
owned cleanup after a census failure. Output/core limits apply only to children.

## Shared-Host Safety

Sixteen endpoint observations and 36 inner command receipts are retained. The
unchanged pinned helper checks UID/BDF, sysfs/SMI activity and memory, and process
attachments before and after every case. Missing/ambiguous evidence rejects the
run. Device selection is explicit and never expands to all GPUs. These checks
are point observations, not an exclusive reservation or physical-overlap proof.

GPU 0's existing workload was not targeted. No GPU reset, native fault injection,
foreign process termination or broad temporary-directory cleanup was performed.
Only the three uploaded files and results under the uniquely owned
`/tmp/fe2o3-selected-pair-20261003.TBigkqRN` directory were removed after retrieval.
The terminal absence check and independently replayed observations accompany the
raw command, identity, test and cleanup receipts in `qualification.tar.xz`.

The first native build succeeded but its source snapshot changed while the
controller tests were being completed. That capture is retained as a diagnostic;
the unchanged-source second build is the selected executable qualification.

## Next Runtime Work

The next functional slice is journal-authenticated pending segmented-frame
forwarding to a scalar peer window, including late admission after native
publication. It must retain the exact latest writer/frame ancestry and cannot
manufacture a compute origin or silently use a staged fallback. Post-arm fault
isolation, ordinary application-kernel admission, eight-device coverage and
matched HIP/HSA performance remain open. This does not complete A3 or prove
whole-runtime semantic refinement.

Run instructions are in [Multi-Device Runtime Qualification](../../runtime-multi-device-qualification-v1.md).
