# Complete-body LLVM/native mechanism fixture

This standalone test is not source admission, a canonical owner, protected finalization, native functional proof or GPU execution. It compiles actual renderer LLVM through the unchanged worker at O0 and O3, then independently checks the selected final payload. No production worker protocol, target admission, default path or source schema changes.

Five closed plans are deliberately written independently in the Rust example and C++ fixture: one block; output diamond; scratch diamond; two terminal paths; maximum 8 blocks/16 arithmetic steps with empty blocks and v63. The C++ checker does not deserialize the Rust model or accept a renderer correspondence report as an oracle.

## Commands (integrator-owned execution only)

Build the ordinary diagnostic_complete_body_gfx942 example and its two pure tests after integrating the separately reviewed renderer. Configure this directory like the existing source-body ABI fixture, with explicit pinned LLVM_DIR, LLD_DIR, FE2O3_PINNED_LLVM_VERSION, FE2O3_EXPECTED_LLVM_BUILD_ID, FE2O3_LLVM_BUILD_ID_FILE and FE2O3_WORKER_SOURCE. Build complete-body-abi-candidate using the root's existing jobs/resource/process supervisor.

For each exact profile one, output_diamond, scratch_diamond, two_terminals, maximum:

1. Run diagnostic_complete_body_gfx942 PROFILE, retaining stdout as a new absolute LLVM path and requiring exit 0 and empty stderr.
2. Run complete-body-abi-candidate PROFILE O0 ABS_LLVM ABS_NEW_O0_HSACO.
3. Run complete-body-abi-candidate PROFILE O3 ABS_LLVM ABS_NEW_O3_HSACO.

Thus five generator calls and ten independently retained native case reports are expected. Paths must be fresh, exclusive, owned output leaves; failed files are evidence and must not be overwritten/reused. Each native call checks and reobserves exact LLVM bytes, writes a fresh HSACO with O_EXCL/O_NOFOLLOW, flushes it and independently reads it back. Exact emitted bytes are retained before the ABI relations run, including on later refusal; a retained file alone is never a successful qualification. Root must additionally pin executable/shared-library/source closure and pre/post selected-file identity. This fixture does not establish executable custody merely by placing synthetic request fields in an existing test request.


## Opt-in actual-source entry symbol

The original four-argument command still requires the exact entry
`complete_body_fixture`. It does not infer an entry from input LLVM. To observe
unchanged LLVM from the actual-source continuation, root may explicitly select
its already joined entry:

`complete-body-abi-candidate PROFILE O0|O3 ABS_LLVM ABS_NEW_HSACO --entry-symbol SYMBOL`

The flag must occur exactly in that position, without extra arguments.
`SYMBOL` must be 1..128 ASCII bytes matching
`[_A-Za-z][_A-Za-z0-9]{0,127}`, and must exactly equal the sole typed kernel
definition. No quoting, dots, dollar signs, Unicode, leading digits, embedded
controls, inference, normalization, or LLVM renaming is accepted. This is inert
syntax and equality, not an authenticated source identity.

Before invocation, root must retain and join the actual authenticated source
case, exact canonical KIR19 identity and its sole kernel/entry, exact decoded
handoff entry and executable LLVM, and exact selected LLVM bytes/hash. The
supplied symbol must be that single joined entry. Pin and reobserve the same
compiler/source/toolchain/executable closure, selected files, and fresh output
leaves used in the qualification ladder. This C++ tool has no canonical owner
or handoff decoder and does not establish that join itself.

The current finite-grid Rust fixtures in
`production-extraction-device/src/complete_body_v19.rs` independently match:

| Actual source feature | Native independent profile | Exact authored body |
| --- | --- | --- |
| `complete-body-one-v19` | `one` | Label 255: move v34 to v33, then compiler tail. |
| `complete-body-diamond-v19` | `output_diamond` | Label 240 branches on selector zero to label 17 (v34 to v33), else label 2 (v35 to v33); both reach label 4 and compiler tail. |

Both declare scratch v32, output v33, inputs v34/v35/v36, exact gfx942:xnack-
wave64 and 64x1x1 workgroup. Their source max_grid is 2x1x1; source/formal
qualification owns that 128-invocation envelope. This native ABI fixture checks
the workgroup/argument/tail mechanism, not the max-grid contract or actual
launch. The plan remains the independently handwritten C++ roster, not a
decoded source record or a renderer-provided oracle.

Run both positive source cases at O0 and O3 with fresh payload leaves. Every
existing typed LLVM, native branch/arithmetic/tail, metadata, descriptor, and
selector-provenance control still runs. A new CPU-only control group additionally
checks four valid identifier boundaries, twelve invalid identifiers (including
embedded NUL and 129 bytes), wrong expected-symbol refusal, a substituted symbol
in a temporary negative LLVM copy, and retention of the old exact default.
The original source LLVM is passed unchanged to the worker and reobserved after
native compilation; only the negative copy is mutated.

Reports add `entry_symbol`, `entry_symbol_selection`, and
`entry_symbol_controls`. The explicit selection is named
`explicit_inert_symbol`; `source_handoff_symbol_join_verified_by_fixture` is
always false. Existing synthetic worker identity, source_authentication=false,
canonical_owner_admission=false, protected_finalizer_admission=false, and
hardware_execution=false fields are unchanged. Successful static native checks
do not make this tool a protected finalizer or confer launch authority.

## Actual checks

The typed LLVM parser verifies the whole module, Function ABI, complete 18-instruction operand graph, target/layout/launch, attributes/metadata, side-effecting inline unit and exact constraints. Six explicit arguments are required; selector is the actual sixth u32 argument and actual tenth inline input bound to s22. Entire inline-assembly text must equal the independent closed plan. Generated labels use LLVM's unique-instance substitution, not caller labels or PCs.

The actual unchanged worker emits object/LLD output and supplies its existing complete instruction/CFG decoding. The checker verifies payload and descriptor joins, entry continuity with no suffix, all worker block memberships/successors and direct destinations, exact arithmetic encodings/operands (including dead/self moves), selector zero/nonzero orientation, compiler-only tail junction and exact guarded store/carry/mask/wait/restore/end sequence. The compiler prologue may vary by O0/O3, but cannot contain a write or control transfer. The actual descriptor identifies the initial kernarg SGPR pair. A narrow provenance relation follows decoded immediate scalar loads and register copies, requires lgkmcnt(0) before loaded values are consumed, and refuses writes to unfinished scalar-load destinations. It requires the ready word at kernarg byte 28 to reach s22. Unknown definitions erase provenance; unknown scalar memory definitions are refused. This adds neither a second instruction decoder nor an executable CFG interpreter.

Independent LLVM MessagePack/ELF inspection requires six explicit metadata slots:
pointer at 0 (8 B), length at 8 (8 B), a at 16 (4 B), b at 20 (4 B), c at 24 (4 B), selector at 28 (4 B), plus the exact optimization-specific COV6 hidden roster: 19 entries at O0, or the first 13 entries through hidden_grid_dims at O3. O3 omits six unused runtime-service descriptors; the reserved hidden extent remains 256 bytes and total kernarg size remains 288. Neither optimization can omit, reorder or overlap any explicit argument. The descriptor's total of 288 bytes alone is insufficient. Metadata LDS/private/dynamic stack must be zero; SGPR/VGPR/AGPR counts and spills are recorded as observations, not assumed zero. gfx942 VGPR capacity uses its eight-register allocation granule and four-register architected boundary; SGPR capacity uses the existing pre-gfx10 eight-register encoding. Capacities must cover declared roles, actual metadata counts and each decoded ordinary register component.

Every case runs 19 typed LLVM mutation refusals (22 for the three branch profiles), covering selector absence/type/order, s23 drift, missing memory/scratch clobber, wrong tail/carry/compare/wait/restore, target/launch and extra effect. These are parser/exact-profile refusals, not falsely attributed to native compilation.

Native mutation controls edit actual retained payload bytes, recompute the payload hash, call the unchanged decoder again and compare the resulting relation. They include self/external/backward/mid-instruction branches, a wrong valid branch target where available, arithmetic register, wait and selector drift. The report distinguishes existing-decoder failure from the new native relation refusal and retains bounded diagnostics. Two actual descriptor-capacity mutations re-decode changed bytes before checking exact current descriptor capacity. Three metadata mutations reparse actual changed payloads, including selector offset 28 to 24 and hidden-entry drift. Two additional native selector mutations change the actual load offset and overwrite s22 after its provenance chain; both use changed payload bytes and fresh decoder results.

These checks establish a static compilation/ABI mechanism only if root executes and validates all ten cases. They do not prove source ownership, whole-body source re-admission, dynamic memory safety, races, physical register values/liveness, native kernel outputs or hardware behavior.

## Bounds and evidence

LLVM input is capped before allocation at 16 KiB; generated assembly remains 4 KiB. The worker output allowance is unchanged at 1 MiB, but retained fixture HSACO must fit 64 KiB. Each actual native report is separately capped at 64 KiB by the reused emitter, with no aggregate report inflation. Metadata note input is 16 KiB; one kernel, at most 25 arguments, <=128 sections, <=4096 descriptor symbols; entry trace <=512 instructions and <=128 blocks. These logical/data caps are not an OS RSS quota. Existing LLVM parser/worker internal memory is supervised externally.

Each native invocation has the existing 90-second alarm; the integrator must retain the separate process-group timeout/reap policy, bounded streams and current lease/toolchain/resource limits. This fixture invokes no GPU, runtime debugger, production source owner or protected finalizer.

The [2026-09-23 qualification](../../../../docs/evidence/complete-body-native-abi-20260923.md) records all ten actual cases, the failed initial build/metadata assumptions, corrected checks and exact evidence boundaries. A new build requires fresh qualification.
