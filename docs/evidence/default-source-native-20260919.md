# Default-register source candidate: native qualification — 2026-09-19

Fresh full source-machine r6 and default-source-native r2 passed on clean
compiler commit `123363584d329bb4d6aa2d528e0515d5eea04361`.
This closes the previously missing default-plan final-code observation, not
the public promotion interface or #282 U2. Earlier edited-plan evidence remains
a separate historical observation; it is not relabeled to this source census.

The source gate measured 3,792 Rust/Cargo/crate-README files, 73,835,558 bytes,
SHA-256 `0caab044a8a8eeb7c756623f99b973585ff8380d0c5dbe127272d119cd9c5052`
before and after. Its full ladder retained seven actual frontend callbacks,
90 positive whole-kernel oracle simulations and three exact source refusals.
The new native runner rejoined that complete source ancestry and selected its
default LLVM output, not the aggregate edited-input field. It also consumed
the actual edited output as an exact low-profile refusal control.

## Final code and descriptor observations

| Instruction | Exact register operands | Encoding |
| --- | --- | --- |
| XOR | v4, v0, v1 | `0003082a` |
| AND | v4, v4, v2 | `04050826` |
| XOR | v5, v1, v4 | `01090a2a` |

The same three contiguous instructions were decoded from both final O0 and
O3 code objects. Each reads implicit EXEC and has no implicit register writes.
Independent literal encoding checks did not derive their expectations from
the source descriptors or the decoder under test.

| Case | Kernel instructions | HSACO bytes | VGPR encoded capacity | Architected boundary |
| --- | ---: | ---: | ---: | ---: |
| O0 | 124 | 6,280 | 24 | 8 |
| O3 | 21 | 5,384 | 8 | 8 |

Authored binding high-water is six, not a claim of six final allocated VGPRs.
v0 is also an ABI live-in: no lexical use scan is promoted into a physical
value/lifetime proof. O0/O3 differences elsewhere are permitted.
Post-link inspection checked gfx942:xnack-, code-object version 6, wave64,
required workgroup [64,1,1], exports choose_bits/choose_bits.kd, no unresolved
symbols, 288-byte kernarg and zero group/private segments.

Exact selected default LLVM: 2,015 bytes, SHA-256
`885e86be44204b59b0405493c78a2dd94da952c106fdd9628aed4d589299b613`.
The existing native worker/LLD/inspection path performed two compilation
cases in one positive process. A second process refused the retained high
profile before worker compilation with the exact typed-call diagnostic.
Configure/build took 412/17,622 ms; the positive/refusal processes took 48/8 ms.
These are observations, not approved performance budgets.

## Retained receipts and failure history

Under `/home/harmenon/fe2o3-authoring-280-282.FEW3gj`:

- `phase11-source-machine-r6/observation.json`: 49,606 bytes, SHA-256
  `100595d5a374d899002f644ced3ff0dd68eb7e8266869f3b72f994a47660b0a5`.
- `rebuildable-cache-phase10.fpFy5o/secondary/phase11-default-source-native-r2/records/receipt.json`:
  269,890 bytes, SHA-256
  `d18513f3b6fe806b31c6dfa5752ec7408589c0d2da3f89cb1d18ded91ca8a295`.
- The neighboring `default.observation.json`: 6,320 bytes, SHA-256
  `84f080cfa3a41de540f62b6304f523d72fa1845dfad99122b6b94dcf4a091ff6`.

Native r1 failed C++ compilation: two LLVM json::Object returns required the
explicit constructor. The r2 prototype copy changes only those constructors.
All expectation predicates remain unchanged. Original frozen inputs and failed
r1 records remain intact; 21 pure runner controls also passed after correction.

All new build/records remain under charged cache. Guards retain two build jobs,
20 GiB combined output/cache, 512 MiB new output, 40 GiB free disk, 64 GiB RAM,
stage deadlines and bounded streams. These are stage-boundary safety checks,
not continuous resource quotas. Process-group kill requests do not guarantee
bounded close/drain/reap completion. Selected tool/link/loader inputs were
measured and rechecked; complete runtime-closure attestation is unavailable.

HSACO observations/digests are retained, not a protected launchable artifact.
No GPU execution, source authentication, functional proof, public production
continuation, physical allocation/lifetime proof or whole-kernel byte-stability
claim follows. The private source materializer still needs its reviewed
release-active compiler-owned boundary before U2 can be closed.
