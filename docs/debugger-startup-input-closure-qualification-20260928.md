# Debugger startup input closure — 2026-09-28

The root-side startup preparation on mi350 now passes its complete selected-input
check. This is a qualified input-closure and supervision checkpoint, not an
observed debugger startup, loaded-file closure, live adapter or GPU capture.

The fresh source generation passed 28 graph controls and 88 selector/reader
controls. Three helpers were rebuilt with Rust 2024 and warnings denied.
The generated 63-file source tree passed 55 supervisor Rust tests, 16
process-census Rust tests, 277 JavaScript tests and 16 Python tests.
Deployment reproduced 63 source files and three helper products; all 45
runtime pin rows matched their complete bytes.

Five fresh benign service families passed: normal completion, timeout,
double-fork descendants, a descendant retaining stdout, and failed execution.
Root checked all 60 records, including exact request/nonce/build identities,
startup and ready ownership, manager identity, release authorization, receipt,
reaping and cleanup. All five owned cgroups were subsequently absent.
Manager cleanup is not child reaping, and this does not prove historical
cleanup of unrelated process families.

The complete selection then read and revalidated all 1,024 named files,
retaining all 879 duties and four mixed-configuration pairs. Selected file
payloads totaled 1,798,580,929 bytes. The separate outer read ledger charged
1,801,031,093 inclusive bytes and 29,285 calls, including EOF and five logical
module-loader envelopes. Retained buffers totaled 37,432,067 bytes. The
separate 512 MiB inner artifact limit was unchanged; the implementation does
not claim to meter every Node module-loader read or total process RSS.

The previous refusal at a selected symlink is resolved by three exact rules:
rustc to rustup, apply_patch to codex, and the system sitecustomize.py alias.
Each rule checks the exact link text, named identity, canonical regular-file
identity and full target content before and after bounded reads. It does not
permit arbitrary symlinks. Independently selected canonical roles remain
independently charged; the sitecustomize target is read under its already
selected named role, not silently admitted as a 1,025th input.

The full check also reproduced generated sources and joined complete
request/receipt/stream evidence for graph, helper, CPU, deployment, runtime
pins and the five benign cases. Passing an input selection does not grant
permission to execute a target or establish that the debugger loaded only
those files.

Complete-selection receipt:
`c1a155f7ba8afc3138e323f3c96231efabdd519ceeb76f0f0fd58ec0885c2ec6`.
Root complete-selection readback:
`d73a9235d5b40e6fc013484c624d98f89bdf40f71550ad73826332b92ba9602c`.
CPU receipt:
`71f8b759ff50067b6505e107da759fd7fb0e47cf7ed719f420870af6d50202bf`.
Five-case receipt:
`af41c76f451ec84ec3b6f3084b8274bc9a6974d3726bec9972f72540ad90b284`.
Full benign readback:
`0cbda3ef4329c9fd857fb09ea280c6daab9e6d528ed7e8c876db8b233aa0c901`.

The associated root coordination expires at 2026-09-28 21:30 UTC. These
historical receipts do not extend that window or prove global writer
exclusion. No GDB, inferior, attach, dispatch, privileged process census or
physical sampler was executed by these gates.

Next steps remain separately qualified debugger startup and actual loaded-file
reconciliation, then same-client native producer/adapter integration and
physical register/memory capture. The input-closure checkpoint supplies no
visualization recording or native replay command. V4 remains open.
Broad accepted exits remain M1/V1/V2/U1/U2/U3 (6/18).
