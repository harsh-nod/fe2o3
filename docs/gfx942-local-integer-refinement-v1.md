# Local gfx942 Integer Refinement

This component connects a shared executable unsigned-add model to retained
compiler inputs. It is a prerequisite for application-kernel verification, not
a complete compiler proof or permission to load or launch a kernel.

## Instruction Model

`Gfx942SAddU32V1` decodes the actual instruction bytes and cross-checks the
retained trace's opcode, operands, definitions, SCC effects and absence of
memory/control effects. The closed profile supports ordinary SGPR0 through
SGPR101, integer inline constants and a shared literal word. Unsupported
registers, encodings and trace disagreements reject.

The model follows sections 12.1 and 13.1.1 of the
[AMD MI300 ISA Reference Guide](https://www.amd.com/content/dam/amd/en/documents/instinct-tech-docs/instruction-set-architectures/amd-instinct-mi300-cdna3-instruction-set-architecture.pdf),
5 August 2025, pinned by SHA-256
`0cec4237cd93ce7dd76ee8502771429eb2308ab47dfa389f5bcc34f5903a6e2a`.
Its projected state contains 102 ordinary scalar registers and SCC, not PC,
memory, scheduling state or a complete GPU wave. Both operands are read before
the destination is written, including when registers alias.

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
discard carry, narrow the result incorrectly or substitute an operand. Five
additional mutants challenge the register-state transition. Each must fail
the intended postcondition; compilation errors and timeouts do not count as
successful negative controls. Source controls reject schema drift, alternate
includes and disconnected production/proof forwarding.

These proofs cover the shared arithmetic and projected-state execute bodies
under valid-index preconditions. They do not prove that decoding establishes
those preconditions, ISA conformance, compiler correspondence or hardware
execution. CPU tests separately exercise decoding, every destination register,
register/literal aliases, incoming SCC and instruction identity.

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
and verifier suites. LLVM assembly/disassembly is not native GPU execution;
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
