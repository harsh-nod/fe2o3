# Live local CPU debugger qualification — 2026-09-23

The optional browser panel can issue explicit commands to an owner-started
loopback bridge over one fixed CPU simulation input. The bridge and panel add
live step/reverse, source-location and stack queries, allocation-relative
memory, and breakpoint/watchpoint controls. They do not compile browser-supplied
source, select a new executable, execute GPU code or change the recorded viewers.

This is a bounded increment toward #281 V2. The accepted broad exits remain
M1/V1/U1 (3/18); the remaining original criteria are not waived.

## Implementation and trust boundary

The six new leaves under `scripts/debug-session-bridge/` are identical in the
two compiler forks. They reuse the existing public JSONL debugger and unchanged
`scripts/debug-console/` command/session/process modules. No Rust code, provider
closure, production policy, existing KIR/debugger schema or runtime allocator
is changed; the bridge adds only its closed HTTP envelope.

The host chooses the debugger, Bundle V6 and request before launch, with exact
length/SHA-256 checks. The service binds numeric IPv4 loopback only, checks exact
Host/Origin and an owner-generated CSPRNG token file, and refuses unknown
request shapes. Token shape and ownership are checked; sufficient entropy
remains a launcher obligation. Tokens never belong in URLs, browser storage, reports or examples.
Selected path checks assume a trusted launcher/filesystem; they are not an
atomic input capsule, complete loader/dependency attestation or OS sandbox.

One child and one pending operation are admitted. Bridge sequence numbers,
backend request IDs and configuration/revision/cursor joins are checked without
rounding large identities. Inner JSON is losslessly re-encoded, not claimed to
be original JSONL bytes. The existing console validator checks selected protocol
relations, not every invariant of the complete Rust payload validator.

Limits include four connections, 1024 sockets, 900 seconds per child, 1800 seconds
per service, discovery plus 254 commands, a 4096-byte request, 1 MiB HTTP reply,
256 KiB inner reply and bounded process streams. A failed or uncertain mutation
is not automatically retried. Cleanup acknowledgement requires leader reaping;
no escaped-descendant guarantee is claimed.

The site lazily loads the panel after explicit opt-in. It does not auto-connect.
Old values clear during pending/replaced/refused requests; unavailable values
stay unavailable. Commands are serialized, late generations cannot replace the
current selection, and cleanup uses the captured connection credentials.
The panel displays logical CPU SSA/source locations and memory, not physical
VGPR contents or a linked Source Variable V2/resource projection.

## Source, tool and tested snapshots

| Fork | Parent main | Functional snapshot SHA-256 |
| --- | --- | --- |
| compiler | `c4bc8a6b16b241ea9125de4364e3cfede1d50b61` | `1696bc331f7f814ef0b1e4e0a1b3b7bb8d39b243c5cf81fdb091001309ef34c3` |
| mirror | `89837a00abe45ac56d92adbb2e92c6fdcb08c6ee` | `1abd313a739b40b7e4f44d20f4061bac0c0c680d3b157ae9015d7c189bc08fe7` |
| site, final focused regressions | `8c27c108dee500bd5b201ee6b1c5fae5d1292ef9` | `37c5e7345eaa74f0e9a4a138826992ee2cd558f47f458be7c815523316350938` |

These are tested functional trees before the final Markdown additions, not
retrospective clean-commit/source signatures. Compiler/mirror counts are
6624/6617 files and 100609090/100547965 bytes; site is 675 files/16856175 bytes.

The unchanged Rust tools were independently built and qualified for the
preceding published increment, from each matching fork. Fresh exports here
use those exact selected tools; no new Rust build is claimed for this bridge.
The debugger is 59708104 bytes, SHA-256
`ed25a90d2ff5fdea2d620cb4cc2db0a12507df1322ba76db645b1ee18d53385e`.
The selected DSOs remain fork-specific. Full selected pins are in the receipts.
Python 3.12, Node 22.22.3 and nightly-2026-04-03 are pinned; Cargo is offline,
locked, jobs 2 / incremental 0. The frontend uses retained dependencies/browsers.

## Real HTTP-to-debugger profile

Both forks freshly exported the unchanged `assembly-authoring-v30` registered
Rust fixture through the normal Bundle V6 route, then passed its independent
CPU checks. The live profile is `assembly_chain`, u32, grid 4 / workgroup 64, with
logical wave-width 32 grouping. Seven typed assembly occurrences compute wrapping
ADD/SUB/XOR/AND/OR: base output 469, separately edited source output 725.
Only the base bundle is selected for the live bridge. The scalar inputs are
`a = 0xfffffff0` and `b = 0x25`; the independent wrapping-u32 reference is
`((((a + b) - b) ^ b) & 255) | 256`. The edited source changes the final constant
to 512. The unchanged source is 957 bytes, SHA-256
`f6cdd5fc4a937979fd5d12e7fa199fd3590fae9cb8f303b680781f9ca5b7df5e`.

| Fork | Base bundle bytes / SHA-256 | Live configuration identity |
| --- | --- | --- |
| compiler | 35573 / `24787523f503dd463455526333d879f6515be270931445907b71c2336d706569` | `2c0176249fda8dd5ee8e18b4359934bf6f6a2e7ce6e18a8e67fbbd41b307ff50` |
| mirror | 35564 / `387cdd0313f1f3f55a7c172b2a553d2f07a2f34f4a2016ba227359448220ee26` | `0707da682ceca77b1de9e30e2f9e443c600eaacf534d8eec77756c49b1ffbb66` |

The identical 528-byte request has SHA-256
`1440cebfd18d500152090639c34021b319a75114b5af00f5faf1e9918f97369a`.
Raw file hashes, bundle identities, source-map identities and debugger
configuration identities are different domains, never substituted for one another.

Each actual loopback HTTP qualification passed 69 exchanges across three fresh
connections, with three observed debugger leaders reaped. Only the middle
connection runs the kernel to completion; this is not three complete executions.
Each retains 140 artifacts, independently audited with its selected input pins.

Checks include 18 exact predispatch refusals without consuming the sequence,
one genuine AFTER breakpoint, four real write-watchpoint stops, ten complete
24-byte memory windows, initialization, unchanged canaries, distinct reverse/
repeat revisions, and exact source/SSA/scope joins. Changed-word counts across
the windows are 0,0,0,1,1,0,1,2,3,4. Allocation/filter IDs come from current
replies. Generation 0 is not evidence of allocation reuse.

A watchpoint can be an exact access stop with an unavailable/not_captured
snapshot. Values from the next captured checkpoint are not relabeled as values
at that stop. Logical active_mask 15 is not hardware EXEC.

## Actual browser qualification

Two actual Chromium projects passed, desktop and mobile, sequentially with one
worker and no retries. Each used a fresh explicit connection and made 27 real
HTTP POST exchanges through the selected canonical bridge and debugger. There
was no route interception, replacement fetch, direct JSONL shortcut or synthetic
positive fallback. The owner observed and subsequently reaped both distinct CPU
debugger leaders; the CLI exited 0 and the owned bridge exited 130 on SIGTERM.

Each project checked keyboard connection, source resolution and stack joins,
five full 24-byte memory windows, two one-byte windows with exact `0x01`
initialization masks, one genuine AFTER breakpoint and one real write-watchpoint
stop. At the watchpoint, snapshot and memory were explicitly unavailable.
The later captured first write was `0xd5010000`; the other three words and all
eight `deadbeefcafebabe` canary bytes remained unchanged. Reverse/repeat used
distinct revisions and independently checked values, scope and source site.
This reaches the first output write, not completion of all four output words.

The control event/revision sequence in each fresh session was:
step 1/1, breakpoint 2/3, uncaptured watchpoint 32/6, later step 33/7,
reverse 31/8, repeat 33/9. Those are this profile's observations, not IDs for
authors to reuse in another session.

The retained observations bind every rendered protocol object to its actual
HTTP pair. Root independently replayed all 52 non-disconnect responses through
the current client validator, rehashed 25 selected inputs and all 11 retained/
framework artifacts, and confirmed both observed CPU process IDs absent.
The four light/dark desktop/mobile viewport PNGs were also visually inspected:
shown controls and response facts were readable and wrapped within the viewport.
That is not an all-state, full-page or quantitative accessibility claim.

The wrapper's pure suite separately passes 31 controls. Mocked late-reply,
replacement and failure tests remain distinct from these actual sessions.
The 28.72-second live run does not qualify real idle expiry, uncertain mutation
abort, response backpressure, GPU execution or protected production admission.

## Separate ordinary-Rust tutorial preparation

The tutorial uses unchanged `examples/vecadd`, f32, grid 4 / workgroup 256.
It is not the assembly_chain/u32/workgroup 64 live qualification above.

A fresh normal Bundle V6/gfx942 export and one standalone CPU simulation passed.
The exact 742-byte tutorial request is retained unchanged. Inputs [1,2,4,8]
and [.5,1.5,2.5,3.5] produce independently checked binary32 values
[1.5,3.5,6.5,11.5]. All 32 input bytes remain unchanged; all 24 output bytes,
initialization and eight guard bytes are exact:

`0x0000c03f000060400000d04000003841deadbeefcafebabe`

This rehearsal ran four in-grid invocations among 256 scheduled slots, one
workgroup and four cooperative decisions. The fresh bundle is 27919 bytes,
SHA-256 `24491db2d8fde736238836e9c8f75561352903c72821dc6d658e3bef81736911`.
The simulator-reported KIR digest is retained as reported, not independently
decoded by the two-stage rehearsal. This does not qualify the vecadd-specific
live browser breakpoint/watchpoint walkthrough, GPU behavior or universal
race freedom. The tutorial labels its manual live exercise accordingly.

## Regression results and preserved failures

- Each compiler fork: 33 bridge controls plus 37 existing console controls pass.
  The actual HTTP helper separately passes 18 pure framing/validation controls.
- Site focused lint/typecheck and 28 client/UI tests pass. Full validation
  passes 1061 unit tests in 75 files, 21 authoring-lab tests, build and evidence
  validation. The existing large-bundle build warning remains.
- Full browser regression passes 162/162 with workers 2 / retries 0 and no skipped,
  unexpected or flaky tests. Four new live-panel tests use routed mocks; they
  are separate from the actual browser qualification above.

The first focused site run retained 11 passing / 12 failing tests. A realm-sensitive
`instanceof Uint8Array` check rejected valid native response chunks in the test
environment. The implementation now uses the intrinsic typed-array brand check.
A genuine foreign-realm Uint8Array is accepted; DataView/other typed arrays and
tag spoofs still reject. Two additional controls pass. The failed receipt is
retained, not rewritten.

The first mirror HTTP attempt exited before any exchange, with only the generic
service-refusal diagnostic. The cause was not established. After checking port
quiescence, a new token/output-path retry passed without code, port or input
changes. A transient bind condition is plausible, not confirmed. Both attempts
remain retained.

The first actual browser attempt failed under the coarse “Read CPU memory”
label. A diagnostic-only rerun showed the one-byte read had already passed its
HTTP, cursor, memory and visibility checks. The next dropdown lookup used
Playwright's exact label-text matching, which includes the nested option text;
it did not match the short label. The private test now selects the two dropdowns
by exact combobox role and accessible name. Three added UI controls verify the
names and selected breakpoint/watchpoint commands. No production debugger,
client, memory or component behavior was changed for this test correction.
Both failed browser attempts remain retained; raw failed framework streams are
withheld, with only closed nonsecret diagnostic fields kept.

Short runs do not establish the real 900-second idle-expiry path or actual
uncertain-mutation timeout handling; related mock controls are not promoted to
real-session evidence. No full dependency, source-authentication, GPU, native,
proof, measured performance or escaped-descendant cleanup claim is made.

## Retained receipts

Paths are relative to the private retained task root on mi350:
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr`.
These are custody references, not public download URLs or authority tokens.
Tokens remain in separate private files and are not included in the evidence.

| Receipt | Bytes | SHA-256 |
| --- | --- | --- |
| logs/phase26-compiler-bridge-unit-r1/receipt.json | 21166 | `a781943050e992ab2859218d51040b0bc24e7af69b2477d37ed50237e25283a3` |
| logs/phase26-mirror-bridge-unit-r1/receipt.json | 21154 | `9f41d44e52f74c125e3dbb9d9f00db382a4ef59c9129376a73e205024b598284` |
| logs/phase26-compiler-bridge-source-r1/receipt.json | 33379 | `1894a8200cbe5c8e88c496c6b31177c84a94bf866544f0c1b8dad708a7104083` |
| logs/phase26-mirror-bridge-source-r1/receipt.json | 33269 | `0f9ce4b64c18bae632ebe626588d9f00075fc032eb4165135259d069c4d77d68` |
| logs/phase26-site-bridge-focused-r1/receipt.json | 15093 | `69b286efafabcd123a122a08d294dfa17c6c33a4b8c6fb7ffcc0ee2e1d649348` |
| logs/phase26-site-bridge-focused-r2/receipt.json | 24743 | `fb89a5bcd97dd74d19821315ffdb582d72bc9bdde184d59e514447187afa78bc` |
| logs/phase26-site-full-validation-r1/receipt.json | 30757 | `a1edc172ed52160d167c281d151165e1e039d1cd2fcc934455703d532c5e9076` |
| logs/phase26-site-full-browser-r1/receipt.json | 34262 | `56e17eada5ed232b6b0b2870740705a79c11b1258597e528c9eb9b99ed2ac2b0` |
| logs/phase26-compiler-http-helper-unit-r1/receipt.json | 25112 | `1c65465adac1c55ba6605291947aaba3524ee5a1ff8794112ee63051b6d92120` |
| logs/phase26-compiler-bridge-http-r1/receipt.json | 45488 | `ea3e7cb5296e708ab8a19bbabf5d617244436c4ef689e286c052e9b77bd40e8d` |
| logs/phase26-mirror-bridge-http-r1/receipt.json | 27075 | `acb6ae8c81e3080e9f7b24a737937858386c1ec9ec09617b4a1c6a8c101c502f` |
| logs/phase26-mirror-bridge-http-r2/receipt.json | 45739 | `095f25c31d1c6ebb3a5679c95db2eeff137253dff2f506d146207e60343dbbb0` |
| logs/phase26-compiler-vecadd-tutorial-r1/receipt.json | 41093 | `9bc679a5ad996589e27868b697cc490d18d88514ec9126b2f61f329b53d5da02` |
| logs/phase26-compiler-bridge-http-actual-r1/receipt.json | 24311 | `14ab27ae54c693890f66178851c8e0df2aa9a2076486310f40b833009ae4d6fc` |
| logs/phase26-mirror-bridge-http-actual-r1/failure.json | 5520 | `2587d1b4cfb7f45cda28f83f066fa97bb97c28496ddd3f8b17e153d25aae5706` |
| logs/phase26-mirror-bridge-http-actual-r2/receipt.json | 24364 | `eaa2363efe930172eedc2d54c756f5e1d5ebda495218c5798f1d25da4004d764` |
| logs/phase26-vecadd-tutorial-actual-r1/receipt.json | 15196 | `bb6ad81caa2facab19230b2de550764ef5d873b996d6ebd4b19a3761ebe43877` |
| logs/phase26-site-browser-owner-unit-r2/receipt.json | 29315 | `ab7174053bd35b52014638b450cdd05baf16e7e44aabcc04b3e4d7a83c3fc65f` |
| logs/phase26-site-browser-owner-unit-r3/receipt.json | 29327 | `4eb2263104299a12fd546d2e433b46a34387efe2b0cc7be61ab6f39d09f7b8de` |
| logs/phase26-site-bridge-browser-r1/receipt.json | 37285 | `75bc2e7e73a067239078234c68b2700a202d2006cce3b3a5549665f97b427b5e` |
| logs/phase26-site-bridge-browser-actual-r1/failure.json | 737 | `a638e0f649104a60d9d969cf40998d93b1fdb4ff022485d27852b76dc2d47174` |
| logs/phase26-site-bridge-browser-r2/receipt.json | 37291 | `9a195ae72312bb55ff7a7ffb9c7b89629f28930eb960070fa13db57dd5aa3ef6` |
| logs/phase26-site-bridge-browser-actual-r2/failure.json | 1169 | `b8641e79e0efb5a1f8f65fbcdfaca6a27fab4b18a3d63f59a939865174475638` |
| logs/phase26-site-dropdown-regression-red-r1/receipt.json | 23403 | `129efa3bdfb86d41a0afb0cadd0f8c066889ab811f85ef89f600366b7b869772` |
| logs/phase26-site-bridge-focused-r3/receipt.json | 27922 | `e0cbc00f156d1ab14b625e74b625a9cec2a62dff06d73099fbe1627e53181876` |
| logs/phase26-site-bridge-browser-r3/receipt.json | 62056 | `cfacc67f40fd85d12d498ca676e66dbca304e8736256cc1c4e94f5e7f9d7c697` |
| logs/phase26-site-bridge-browser-actual-r3/receipt.json | 12322 | `1b1e75e604cdb87b5dc01d800e1df6122746b0055e295dd755a56f1000c1b31b` |
| logs/phase26-site-full-validation-r2/receipt.json | 30757 | `a6e6affa93e5c05e5958aec6dcc58e08d8438238b976436efcb86ea22353c9ac` |
| logs/phase26-site-full-browser-r2/receipt.json | 34263 | `598207d2c8324f64b668b6c0c1181b4d9e07c2c5798773a475707c7a7845e08a` |

The outer supervisor enforces sampled root 160 GiB / free 40 GiB / RAM 64 GiB reserves,
per-command 20-minute deadlines, 8 GiB output and 8 MiB stream limits. Helper/browser
guards are narrower. These cooperative/sampled limits are not hard OS quotas.
All earlier captures, failed attempts and compiler scratch are retained.

## Remaining milestone work

This increment supplies an actual local CPU connection, not the complete
source-linked register/LDS/buffer debugger. Remaining work includes terminal
fault observations before unwind, real same-session allocation generation reuse,
dynamic activation identity, richer synchronized source/resource queries,
compiler physical lifetime maps, target bank models, supported hardware
adapters, complete tutorials and measured scale/interaction budgets. Those
producer and schema changes remain coordinated with #215/#216/compiler owners.

The existing #272 owner has independently published additive SubjectV2,
receipt-transport and V2 trust-input work on its integration branch (latest
observed commit `0e1158764ed74d5ea0f51705c77d61069f26af1a`, not main).
It is not adopted here and does not supply production admission for this increment. No milestone is marked complete
by borrowing another branch's test, proof or hardware observations.
