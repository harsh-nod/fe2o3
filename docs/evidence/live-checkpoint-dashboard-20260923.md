# Live checkpoint source and resource dashboard — 2026-09-23

This additive qualification covers an explicit live CPU dashboard for ordinary
Rust assembly kernels. It does not close the complete #280/#281/#282 contracts.
Accepted milestone exits remain M1/V1/U1 (3/18). The earlier live-controls report
and its results remain separate.

## Implementation and source selection

The bridge admits three additional commands: `allocations`,
`accesses ORDINAL 0`, and `variables 1`. Existing console, ResourceV1 and
SourceVariableV2 replies retain their schemas and share one correlated session
ledger. A captured, successful exact operation step supplies the checkpoint.
State/discovery, filter mutation, uncertainty and uncaptured stops cannot
supply or preserve that authority.

The site presents named source bindings separately from whole SSA values,
allocation inventory, retained access history and initialized memory. Explicit
refresh reserves at most three read-only calls, or five for a selected range;
missing stack next-operation information uses two or four actual calls.
The complete collection is limited to 2 MiB of actual HTTP response text.
It follows no pagination token. The existing recorded viewers are independent.

Source-level reproduction is documented in the
[live checkpoint tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/live-checkpoint-dashboard-v1.md).
It uses the existing `assembly-authoring-v30` fixture, not the separate vecadd
tutorial. Its seven typed calls cover six integer instruction forms. Grid
[4,1,1], workgroup [64,1,1], scalar inputs 0xfffffff0 and 0x25, four initialized
sentinel output words and eight canary bytes produce four u32 words of 469.
Logical wave32 is a CPU visualization grouping, not physical GPU execution.

Initial main bases were compiler `afdb40d1921cf6d09e6e725198c6f7bf8c6217cf`,
mirror `5d355f914c81a58cff6d1fc716f531d3a4c47e0a`, and site
`8b6327592cc86b0aa478e761cd16e901d58b60fb`. Qualification used isolated
working candidates on mi350. The final publication adds documentation only to
the functional snapshots below; publication checks must independently verify
that distinction. This report does not claim that its own final bytes were
already present during earlier tests.

| Functional source snapshot | Files | Bytes | SHA-256 |
| --- | ---: | ---: | --- |
| Compiler | 6628 | 100684026 | 9b9a1db4f069497cd5854db18785b37267c5142850d19b4485ffe879054b4fd2 |
| Mirror | 6621 | 100622901 | 9e3cc684aa4d51262ea6be1225bde193e64edda47625350d68bcaa6563326a24 |
| Site, including new routed browser tests | 686 | 16998749 | f2cf284d8b45451625c2c41a31747df4dec8690dde9f6f50b7b6433fb7b3d9fe |

The census hashes sorted repository-relative paths, NUL, full bytes, NUL for
tracked and unignored new files. It is not compiler-closure attestation.

## Fresh normal-source and HTTP observations

Both forks made fresh normal fixture exports and CPU runs. The selected base
result was 469; the separate edited-feature check was 725. These runs reused
the previously qualified Rust tool binaries: they are fresh source exports,
not a claim of new Rust tool builds. The unchanged fixture is 957 bytes,
SHA-256 `f6cdd5fc4a937979fd5d12e7fa199fd3590fae9cb8f303b680781f9ca5b7df5e`.
The base request is 528 bytes,
`1440cebfd18d500152090639c34021b319a75114b5af00f5faf1e9918f97369a`.

| Selected normal-source result | Bytes | SHA-256 |
| --- | ---: | --- |
| Compiler source receipt | 14368 | 8312e82e05ad4e6852075f61ce2f4cab438b221e8c9b86190a695c15505721f3 |
| Compiler BundleV6 | 35574 | fec8bb035881bad96b99edb7e157d10789a0074840e254b68d6c22ff376c3112 |
| Mirror source receipt | 14368 | 1e3d87cd7e021922353d52fac69f9b39b8e4926e28d97bf7f2f61959ef8c9f6a |
| Mirror BundleV6 | 35570 | c15577b2b271306dc532a65ab41e50b87b5aed7891c008fd208463538f29b1a7 |

The real HTTP experiments each retained 105 exchanges, three fresh debugger
children, five positive source/resource collections and seven live-query
predispatch refusals. All three owned child identities were observed reaped
per fork. Twenty-five total HTTP refusals include the seven query refusals.
Each experiment retained 61 successful console replies, five SourceVariableV2
replies and ten ResourceV1 replies, without schema relabeling.

Five collections join the original control, memory, stack, source, inventory
and access replies. Their events/revisions were 1/1, 1/1, 34/5, 31/6 and 34/7;
source rows were twelve each, SSA counts 3/3/3/18/3, retained writes 0/0/1/0/1.
The input bindings a/b were captured and ten other bindings remained explicitly
unrepresented. The immediate post-store source request was refused because
the stack had no next operation; later lane1 source values were not attributed
to the earlier write. Reverse/repeat restored values under fresh revisions.

Each HTTP experiment separately completed all four output words:
`0xd5010000d5010000d5010000d5010000deadbeefcafebabe`, initialization
`0xffffff`. The browser experiment below checks only the first store.

The fresh runtime selection includes six bridge and four console modules,
including the two new live-query modules. Neither the old eight-module
selection nor an earlier HTTP receipt was substituted.

## Actual desktop and mobile dashboard

A separate real browser run used the canonical fork's selected source/runtime
and its own two fresh sessions. Desktop and emulated mobile both passed,
workers 1, retries 0, with exactly 55 captured POSTs and eight dashboard
collections per project. There was no route interception, fetch mock or
synthetic positive fallback. Two owned CPU child identities were observed
and reaped. The independent owner replay consumed every retained HTTP pair.

| Checkpoint | Event/revision | Inventory / selected calls | Source rows | SSA rows | Selected retained writes |
| --- | --- | --- | ---: | ---: | ---: |
| Initial | 1 / 1 | 3 / 5 | 12 | 3 | 0 |
| Postwrite | 33 / 7 | 2 / 4 | unavailable | 18 | 1 |
| Reverse | 31 / 8 | 3 / 5 | 12 | 18 | 0 |
| Repeat | 33 / 9 | 2 / 4 | unavailable | 18 | 1 |

Actual DOM observations were joined independently to all source-binding,
SSA, inventory and access columns, their session/checkpoint/page summaries,
and all 24 memory-cell labels including initialization and canaries.
Postwrite/repeat source absence was displayed explicitly, with no source query
or replacement by older values. The retained write was the actual watch event;
its historical range was not treated as event-time memory.

The run checked explicit break/watch installation, genuine stops and removal;
range-edit clearing without a request; preservation of otherwise-current
source/SSA on range edits; clearing on filter edits/mutations, uncaptured
watch stops and disconnect; and fresh reverse/repeat revisions. Intermediate
query replies were not falsely described as individually rendered raw replies:
only each collection's last raw reply was joined to that panel.

Root viewed the four retained light/dark desktop/mobile PNGs. These are viewport
images of the checkpoint heading and surrounding content, not full-page pixel
coverage of every lower table. Table correctness is established by the actual
DOM assertions, not inferred from those screenshots. Both layouts passed the
width check. No broad accessibility certification is claimed.

## Regression gates and retained receipts

Both forks passed 55 bridge plus 37 console tests. The site passed 1099 unit
tests, 21 authoring-lab checks, lint/typecheck/build/evidence gates and 166
desktop/mobile regression tests. Four new dashboard browser regressions use
routed mocks and are not counted as real CPU sessions. Focused site tests
passed 188 cases. The HTTP helper passed 31 pure controls. The initial browser
owner passed 49 controls; the effective diagnostic-only version passed 50.

The existing large-bundle build warning remains. No runtime feature was
changed after the functional qualification snapshot.

Retained paths below are relative to the private mi350 task root
`fe2o3-authoring-280-282-mi350.4VZ42zNr`; hashes identify complete files,
not downloadable public attachments.

| Receipt under logs/ | Bytes | SHA-256 |
| --- | ---: | --- |
| phase27-compiler-live-query-unit-r1/receipt.json | 21086 | 631ceff2e44d6f1a34bf18f5b90542faf6e56dc40089e5640e0a5ac5e82e6abd |
| phase27-mirror-live-query-unit-r1/receipt.json | 21074 | cdbf716a963739c12de94de81de85c76f9651011787437990f92472665d4d2cc |
| phase27-site-full-validation-r1/receipt.json | 30954 | 278a0699769155b7451501b4e9b099d6e5a2b2f1da52a4f48d87ce9b1302c00a |
| phase27-site-full-browser-r1/receipt.json | 35673 | 4bc60bbe3fbd9c3763fb875075aed81f300ce0eba6e65fabbf7ecc09847e60ae |
| phase27-compiler-live-query-http-actual-r1/receipt.json | 118560 | 1b5ae67dd0b4d63018f595af1b000e8aa57005814bf9cd735271e73a03665e7b |
| phase27-mirror-live-query-http-actual-r2/receipt.json | 118627 | e6ad30b7ee67bd37eb45f91def3e153977eb6778cac152377ef128e3ded6a7d6 |
| phase27-site-dashboard-browser-actual-r3/receipt.json | 37282 | 1b73c9998d5f1a5ad5b96802dd22ea822b3d2daca99db192cf9c6ba9c3165797 |
| phase27-site-dashboard-browser-r3/receipt.json | 63035 | 50da0212f7e889d93e1952746ce9fe138a0453ab53c42b3e8287d7de86ef4322 |
| phase27-compiler-dashboard-owner-unit-r3/receipt.json | 26509 | f4d659990c784f3dea1b57bd4ac88383c2d2ed6f007d287aaa103c86e50a456e |

The compiler/mirror HTTP artifacts were independently rehashed and all five
response groups matched their retained raw HTTP evidence. Original HTTP
response bodies are retained; the inner protocol string is a lossless bridge
re-encoding, not original child JSONL. Authentication headers are not retained.

## Preserved failures and limits

Mirror HTTP R1 exited during startup before any HTTP request. Its generic
startup error does not establish the precise cause. Its inner failure is
6231 bytes, SHA-256
`f5fddadff859c03e414f4c5fbd3d64eaf5192a9276e92ecae8a044e05edb6e80`.
R2 used a fresh secret/output/configuration and passed; the failed output was
not overwritten or promoted.

Actual browser R1 passed desktop but failed during mobile's initial selected
collection: 15 POSTs, 14 complete captured pairs. A swallowed asynchronous
response-capture failure produced a generic guard diagnostic; its precise
header/body/decode/parse cause remains unknown. This is not a demonstrated
product fix or a proven transport diagnosis. Raw failed browser streams were
withheld under the credential policy, not treated as acceptance evidence.

The effective harness preserved the first failure code and added four closed
response-capture-stage diagnostics, without relaxing any assertion, cap or
secret check. Its first pure run failed because the frozen enum-count test
still expected 117 rather than the explicitly extended 121 codes. That failure
was preserved; a fresh effective copy corrected that exact expectation and
passed all 50 controls. Reserved browser R2 configurations were not executed.
Fresh actual R3 passed both projects with the unchanged product code.

No automatic mutation retry, idle 900-second expiry, uncertain-mutating-abort,
large-stream backpressure, escaped-descendant closure, terminal-fault snapshot,
dynamic activation, allocation reuse/lifetime, physical registers, GPU result,
source authentication, protected proof or production admission is established.
Source frame1/occurrence1 and generation0 retain their existing limited meanings.
The work advances V2 but does not replace its remaining producer and ordinary-
source lifecycle/fault requirements, or the other authoring milestones.
