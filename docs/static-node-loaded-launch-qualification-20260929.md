# Static Node CPU launch qualification — 2026-09-29

The exact reviewed draft-path launcher passed 61 fixture-free controls, an actual fresh-synthetic CLI invocation with exit 0, and 1 opt-in real-filesystem readback control. The later installed-path follow-up below has its own actual invocation and complete readback. Repository publication remains pending at this checkpoint.

## Selected source and portable placement

The source packet is `static-node-loaded-launch-r43-r2/MANIFEST.json` (57,906 bytes; SHA-256 `593070975bb3165da502fde913fa09c71055a3c40f3bdbd20d4626b394911fd6`). Ten modules are selected byte-for-byte for `tools/debugger/loaded-static-node/`:

| Leaf | Purpose |
| --- | --- |
| launch-model.mjs | Closed bootstrap/request admission and consistency |
| launch-reader.mjs | Bounded whole request reader |
| launch-run.mjs | Injected request → adapter → report orchestration |
| launch-fs.mjs | Fixed filesystem/FD 1 binding |
| launch-main.mjs | Explicit executable entry |
| launch-module-map.mjs | Fixed logical runtime import map |
| launch-controls.test.mjs | 53 fixture-free launch controls |
| launch-fixture.mjs | Pure synthetic preparation and result verification |
| launch-fixture-controls.test.mjs | 8 fixture-free preparation controls |
| real-filesystem-launch-controls.mjs | 1 explicit opt-in complete-readback control |

Sixteen existing profile/reader/graph/adapter dependency leaves are context, not duplicate installation targets. The complete 26-module source/control graph has 94 import edges; the fixed CLI runtime has 22 modules and 71 edges. This is a logical source graph, not proof of actual loader/ELF accesses. The host-specific root harness and raw historical/qualification data are not portable package leaves.

## Root evidence

The pure-control receipt SHA-256 is `a128239a741b6234aa3ce5082607d07954bc5e283ac27cc19f222e23a09eceee`. Its source-qualified record is 2,534 bytes, SHA-256 `ace7df5db43ea201912c109336ff016fbdfb893caf4e329039dc980063053ace`.

The actual draft-path CLI receipt is 90,985 bytes, SHA-256 `e35f1367ea4fe387b3ee8bd4cef7c908cb1bc20a67c118f670068939c4c2ad9c`. Root used the reviewed R2 harness with a fresh private synthetic fixture: 65,537-byte target, exact `alias.bin -> target.bin`, observed empty file and distinct absent name. Source/runtime/terminal domains 28/28 retained exact before/after identities.

The independent complete-readback record is 9,233 bytes, SHA-256 `6bad1f1af49df83d3634e7e38f12a28ed02b62f476c765e882b430b5d3cea18e`. All six output roles were read completely; 12 regular named files, including provenance and fixture files, total 451,889 bytes. Temporary/final evidence were identical retained hard-link names, and the empty stderr was observed, not a missing-evidence substitute. Root also read the final root-result publication externally.

The opt-in readback receipt is 116,574 bytes, SHA-256 `16c7601815dd687eed594e4a525231fb1b2a370ff230589bbbfc492f7ca164c7`. The aggregate root qualification record is 2,615 bytes, SHA-256 `ab22c521df24ac26ecbf65b796897167e8b8224e5ed66e5f2595c3b146cd0a2e`.

## Installed-path follow-up

Root subsequently passed 263 combined installed fixture-free controls, including the same 61 launcher controls, then invoked the actual installed compiler entry through the separately reviewed path-only harness. The child exited zero and the installed opt-in readback control passed once.

The combined installed receipt is 426,448 bytes, SHA-256 `ae8b5ee8d5f8d8d3249070b016a723664cad092fef272f782b3015d60d6e7d9e`. The installed CLI receipt is 141,708 bytes, SHA-256 `d4eb928db788112ef7e24f16b92f77708f12db4fbfd8ec275bfe2cec49fce06c`; its separate opt-in receipt is 167,549 bytes, SHA-256 `1101c81ee5a2aed23806fac67bf53d5e2d8c05cefec5c02f3a2f66c15080ec5f`.

Root completely read all six output roles and 12 named regular files totaling 425,112 bytes, including provenance and the fresh target/empty fixture. All 28 selected runtime/source/terminal domains retained exact before/after identities. The exact relative alias and separately absent name were checked; empty stderr was an observed file. The complete installed readback is 9,389 bytes, SHA-256 `fd3e3e4fb9e8cef5289eb728cf7be05e806c64a457b6fc1e2d3ca3908d648fbd`; the aggregate installed qualification record is 2,529 bytes, SHA-256 `ce5f59f86ec0922a80a838cb95e3bc51625d78d0931e306d12ea9538acb435c3`.

These are fresh installed-path observations, not a relabeling of the earlier 451,889-byte D-path readback. Source pin sizes/hashes remain unchanged across the reviewed path relocation. No historical operation or native capture was activated.

## Limits and remaining gates

The earlier process invoked the exact D qualification-tree entry. The later installed-path process has the separate receipts above; its claim comes from that actual invocation and complete readback, not merely byte-identical copies. Repository publication still requires its own root-owned gate and immutable commit binding.

Root setup/readback, source pinning, Node/ELF loader, inherited captures, resource probes, watchdog and global supervisor accounting are separate from package-provider budgets. Capture polling is not a hard per-write bound or independent child RSS enforcement. Directory/descriptor brackets do not establish global writer exclusion. Finite CPU scope and all current output/source identities must be externally bound for each run; no historical scope renews automatically.

The historical operation was not activated. All fourteen historical evidence obligations remain external; stale historical pins are preserved. There was no GDB, attach, inferior, native/GPU dispatch or physical debugger capture. V4 remains open and accepted broad exits remain M1/V1/V2/U1/U2/U3 (6/18). Installed-path qualification and future publication advance only their narrow stage, not these broader claims.
