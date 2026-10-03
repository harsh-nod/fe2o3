# Local gfx942 Integer Refinement

This component connects shared executable integer instruction models to retained
compiler inputs. It is a prerequisite for application-kernel verification, not
a complete compiler proof or permission to load or launch a kernel.

## Instruction Model

`Gfx942SAddU32V1` and `Gfx942SMovB32V1` decode actual instruction bytes and cross-check the
retained trace's opcode, operands, definitions, SCC effects and absence of
memory/control effects. The closed profile supports ordinary SGPR0 through
SGPR101, integer inline constants and a shared literal word. Unsupported
registers, encodings and trace disagreements reject.

ADD follows sections 12.1 and 13.1.1, and MOV sections 12.3 and 13.1.3, of the
[AMD MI300 ISA Reference Guide](https://www.amd.com/content/dam/amd/en/documents/instinct-tech-docs/instruction-set-architectures/amd-instinct-mi300-cdna3-instruction-set-architecture.pdf),
5 August 2025, pinned by SHA-256
`0cec4237cd93ce7dd76ee8502771429eb2308ab47dfa389f5bcc34f5903a6e2a`.
Its projected state contains 102 ordinary scalar registers and SCC, not PC,
memory, scheduling state or a complete GPU wave. Sources are read before the
destination is written, including when registers alias. ADD replaces SCC;
MOV preserves it and changes only its destination register.

## Arithmetic And State Proofs

Rust execution and Verus include the same arithmetic macro. For every pair of
`u32` inputs, the theorem establishes:

- The result is the sum modulo `2^32`.
- SCC is true exactly when the mathematical sum is at least `2^32`.
- Result and carry reconstruct the mathematical sum exactly.

The production `execute` method and Verus also include the same register-state
transition macro. Given an instruction with valid ordinary-register indices,
the theorem establishes that both operands use the incoming state, including
all alias cases; the returned value and carry equal the arithmetic result;
only the destination register changes; and SCC is replaced by the new carry.
The proof checks the same 102-register array, source variants, result fields and
instruction fields as production. Constant operands range over all `u32` values.

The runner checks the pinned Verus release closure, unchanged source inputs,
structured solver results and owned-process cleanup. Three arithmetic mutants
and five ADD-state mutants are retained, alongside five MOV-state mutants.
Primitive mutants must fail their intended postconditions. The bounded
composition below adds origin/execution-fold mutants, which must fail declared
postconditions or loop invariants at the exact target and shared-body location.
Compilation errors, overflow and timeouts do not count as successful negative
controls. Source controls reject schema drift, alternate includes, disconnected
production/proof forwarding, altered getters and nonempty runtime annotations.

These proofs cover the shared arithmetic and projected-state execute bodies
under valid-index preconditions. They do not prove that decoding establishes
those preconditions, ISA conformance, compiler correspondence or hardware
execution. CPU tests separately exercise decoding, every destination register,
register/literal aliases, incoming SCC and instruction identity.

## Bounded Composition

`Gfx942MovPrefixAddU32V1` selects a complete contiguous same-function, same-block
trace interval containing at most 64 instructions: zero or more MOVs followed
by exactly one ADD. It scans retained instruction order without sorting or
filtering out unsupported instructions. Missing or split boundaries, cross-block
intervals, unsupported instructions and over-limit spans reject. The model owns
only the bounded decoded span; it does not carry mutable caller-supplied origins.

One shared iterative body propagates `EntrySgpr` or `Constant` origins, reading
each source before updating its destination. A second shared body executes the
actual MOV methods followed by the actual ADD method. The proof relates both
folds to one origin recurrence over the incoming register state. It establishes
the final modular value, carry, SCC and complete register frame, including
preservation of registers outside the set of written destinations. This holds
for valid instruction indices and the bounded length, not as a theorem about
the decoder or the authenticity of a machine trace.

Origin calculation costs O(102 + n) and execution O(n), with constant-size
origin scratch and at most 63 decoded MOVs. No recursion or repeated whole-prefix
scan occurs inside either loop. The source checker binds the primitive getters,
loop bodies, proof-only annotations and actual production forwarding.

## Compiler Obligation

`check_gfx942_local_checked_u32_add_v1` borrows the exact
`ValidatedCompilerProofInputsV4` and authenticated analyzer-execution owners.
It extracts an anchored MIR/KIR checked addition, its literal, types and value
and overflow coordinates, then checks a matching encoded scalar addition and
its physical reaching definitions. Unsupported operand shapes and ambiguous
definitions reject. The initial profile accepts a semantic local and a unique
KIR `u32` block parameter plus a literal. Existing bounded CFG analysis must
establish that the parameter's block dominates the checked addition, including
when pruned SSA reuses a loop-header parameter in its body. The exact source
statement span remains just the literal and checked addition; operand
normalization and arbitrary arithmetic programs remain unsupported.

The returned obligation explicitly leaves two input equalities unresolved:
the semantic local equals the KIR SSA value, and that value equals the machine
register at the selected instruction. Physical reaching definitions do not
establish either equality. Result correspondence is likewise conditional, not
a verified continuation into the remaining program.

`check_gfx942_local_mov_prefix_checked_u32_add_v1` is a separate borrowed API;
the original single-ADD checker is unchanged. It retains the same exact input
owners and MIR/KIR anchor restrictions, then checks the bounded physical span.
Its terminal operands must resolve to one entry register and the exact KIR
literal, including when a MOV carries that literal. The derived register must
have one supported reaching definition at the first instruction, not merely at
the terminal ADD. External MOV/ADD definitions must define that register and
dominate the span entry; ambiguous or unsupported definitions reject.

The returned entry coordinates now name the span start. MIR-local/KIR-value and
KIR-value/entry-SGPR equality, reaching that entry with those values, and result
continuation beyond the ADD remain unresolved. Internal physical copies and
their modeled arithmetic are composed; no source-to-machine authority follows.

Compiler origin, exact source-to-payload lineage, retained local-to-SSA
correspondence, LLVM lowering, entry ABI, CFG composition, address and memory
effects, numerical semantics and publication currentness remain required for a
Worker V3 application refinement backend. This API creates no such authority.

## Validation

The [initial qualification record](evidence/dev-gfx942-integer-refinement-2026-10-02/README.md)
separates arithmetic proof, CPU fixtures, authenticated **test-worker** custody
and real LLVM-MC compatibility checks. The
[state-transition checkpoint](evidence/dev-gfx942-state-transition-2026-10-03/README.md)
adds the shared execute theorem and requalifies the complete selected analysis
and verifier suites. The
[MOV-composition checkpoint](evidence/dev-gfx942-mov-composition-2026-10-03/README.md)
adds MOV and bounded shared-body origin/execution composition, passing 832 CPU
tests, 30 doctests, both ADD/MOV checks on LLVM 18 and 22, and 21 required logical
mutants. Thirteen tests are explicitly ignored in the ordinary CPU run, including
the separately selected native assembler checks.
LLVM assembly/disassembly is not native GPU execution;
its synthetic trace envelope is not production analyzer evidence.

Run the pinned proof and required negative controls with an absolute `VERUS`
path:

```sh
bash crates/fe2o3-kernel-analysis/verus/run-gfx942-add-u32.sh
```

The existing runtime-model workflow and `scripts/ci-local.sh` Verus gate invoke
this campaign. Native LLVM-MC tests are explicitly ignored by default and must
be selected with `--ignored` and `FE2O3_GFX942_LLVM_MC` set; a missing configured
tool fails rather than silently passing.
