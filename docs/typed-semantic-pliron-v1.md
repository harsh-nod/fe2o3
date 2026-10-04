# Typed semantic expressions in PLIRON

Production `SemanticExpression` recipes materialize as workload-neutral SSA,
not as digest-only assertions. The closed node set is:

- typed symbols and constants;
- unary and binary operators with explicit overflow behavior;
- comparisons, selects, and casts;
- a root with an exact numerical-policy schema and SHA-256 commitment.

For example, an unsigned wrapping `s7 + 4` is represented by two leaf
operations, a typed binary operation, and `kernel.semantic_typed_root`. The
root selects exact bitvector operator congruence. Float-bearing expressions
must instead select exact IEEE-754 operator congruence with nearest-ties-even
rounding and exact exceptional-value bits.

The mandatory semantic-refinement pass reconstructs the AST from SSA and
checks all of the following before lowering:

1. node arity, scalar widths, operand/result types, operator domains, overflow
   modes, casts, and operation definedness;
2. the expression node/depth bounds of 8,192 and 128;
3. that the numerical policy matches the complete expression, including casts;
4. that the root commitment equals the canonical expression-and-policy
   transcript;
5. that the verified root commitments exactly match the independent retained
   production recipe, in order.

Collective semantic contracts consume only verified typed roots. Fold and
recurrence values, identities, and transitions must agree on scalar type and
the complete numerical contract. Permutation maps and inverses must use one
integer bitvector contract. A collective's declared value policy must match
its actual and expected roots before ownership or proof evidence can discharge
the contract.

The legacy `kernel.semantic_expression_commitment` operation remains an opaque
compatibility form. It is compared only by byte identity and gains no typed
authority.

## Proof boundary

This representation supports exact operator-identity and congruence claims at
the authenticated safe-reference-MIR to kernel-MIR boundary. It does not prove
the mathematical interpretation of an operator, target-instruction IEEE-754
behavior, source-to-ISA refinement, artifact identity, launch correctness, or
runtime execution. Those require separate evidence and joins.
