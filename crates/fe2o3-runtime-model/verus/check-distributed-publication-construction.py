#!/usr/bin/env python3
"""Qualify shared descriptive constructors/trailer, never whole-wire authority."""
import hashlib
import json
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path('crates/fe2o3-runtime-model/verus')
SRC = Path('crates/fe2o3-runtime-model/src')
OWNER = SRC / 'distributed_publication_contract.rs'
DECLARATIONS = SRC / 'distributed_publication_contract/declarations.rs'
OLD_BODY = SRC / 'distributed_publication_contract/classifier_body.rs'
BODY = SRC / 'distributed_publication_contract/construction_body.rs'
PROOF = V / 'distributed_publication_construction_v1.rs'
OLD_PROOF = V / 'distributed_publication_contract_v1.rs'
IDENTITY = SRC / 'identity.rs'
BRIDGE = V / 'check-distributed-publication-contract.py'
BRIDGE_SHA = 'e29345c409f6cc4e47a85a9e52e8581fefb5b4575feaea71e6726834c182dc48'
FILES = [PROOF, OLD_PROOF, DECLARATIONS, OLD_BODY, BODY]
SOURCE_FILES = 308
SOURCE_TREE_SHA = '641e9e3c94274ba46410d9312e3259bde6999387c9eca3f4963394f1b31e7d73'
PROOF_SHA = 'b0955242b7c9e8868ba917be3145451d8e26af0ddf3a49ddf7379b5155d24790'
UNCHANGED = {
    OLD_PROOF: '57edf7747e4c86c6e73b0311e59e06b7fb2dbf276fba5988e526dfd4fcacd853',
    DECLARATIONS: '39a5e23849a75df9af224b2a4193cd037b5bb39cbddc85c3de07ada299015363',
    OLD_BODY: 'a9523b717e47851e50163e41a9cc4ff2ec31b10e814a90521644aaaa0158f18d',
    IDENTITY: '4df48a80f5bff5ff82481358ee320819de5afc38eee8e7520c50732326fc3564',
}
EXPECTED_VERIFIED = 37
SELECTION_NOTES = {
    '*ModelDistributedOperationBindingV1::from_untrusted_coordinates': frozenset(('verifying root module (selected functions)',)),
    '*UntrustedDistributedPublicationReceiptV1::new': frozenset(('verifying root module (selected functions)',)),
    '*decode_outcome_trailer': frozenset(('verifying root module (selected functions)',)),
}
SELECTORS = {
    'binding': '*ModelDistributedOperationBindingV1::from_untrusted_coordinates',
    'receipt': '*UntrustedDistributedPublicationReceiptV1::new',
    'trailer': '*decode_outcome_trailer',
}
DIGEST_FIELDS = ('runtime_instance', 'participant', 'coordinator', 'membership', 'run', 'operation',
                 'artifact', 'execution_plan', 'placement_plan', 'target', 'runtime_model')
INTEGER_FIELDS = ('participant_incarnation', 'coordinator_epoch', 'membership_epoch', 'attempt')
MACROS = {'binding': 'distributed_binding_constructor_body_v1',
          'receipt': 'distributed_receipt_constructor_body_v1', 'trailer': 'distributed_outcome_trailer_body_v1'}
MUTANT_COUNT = 30

def need(value, message):
    if not value:
        raise ValueError(message)

def invocation_guard(flags=None):
    flags = sys.flags if flags is None else flags
    need(getattr(flags, 'isolated', 0) and getattr(flags, 'dont_write_bytecode', 0)
         and not getattr(flags, 'optimize', 1), 'use isolated python3 -I -B without optimization')

invocation_guard()

def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()

def tree_hash(sources):
    return hashlib.sha256(json.dumps({str(p): sha(text) for p, text in sources.items()}, sort_keys=True, separators=(',', ':')).encode()).hexdigest()

def bridge():
    data = (ROOT / BRIDGE).read_bytes()
    need(hashlib.sha256(data).hexdigest() == BRIDGE_SHA, 'authenticated unchanged classifier checker helpers')
    module = types.ModuleType('distributed_construction_helpers')
    module.__file__ = str(ROOT / BRIDGE)
    sys.modules[module.__name__] = module
    exec(compile(data, module.__file__, 'exec'), module.__dict__)
    return module

def snapshot():
    names = {PROOF, OLD_PROOF} | {p.relative_to(ROOT) for p in (ROOT / SRC).rglob('*.rs')}
    result = {}
    for p in names:
        selected = ROOT / p
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected, 'ordinary exact source')
        result[p] = selected.read_bytes().decode()
    return result

def audit(sources):
    implementation = {p: text for p, text in sources.items() if p.is_relative_to(SRC)}
    need(set(sources) == set(implementation) | {PROOF, OLD_PROOF}
         and len(implementation) == SOURCE_FILES and tree_hash(implementation) == SOURCE_TREE_SHA, 'reviewed complete model source roster')
    need(sha(sources[PROOF]) == PROOF_SHA and all(sha(sources[p]) == digest for p, digest in UNCHANGED.items()), 'new proof and unchanged older executable closure')
    owner, proof, identity = (sources[p] for p in (OWNER, PROOF, IDENTITY))
    compact = re.sub(r'\s+', '', owner)
    need(owner.count('mod construction_body;') == 1
         and compact.count('letoutcome=decode_outcome_trailer(r.fixed()?)?;r.finish()?;Self::new(binding,sequence,outcome)') == 1,
         'actual trailer is called at unchanged decoder validation point')
    for macro in MACROS.values():
        need(owner.count(macro + '!') == proof.count(macro + '!') == 1, 'same three actual shared body invocations')
    need(proof.count('include!("distributed_publication_contract_v1.rs");') == 1
         and proof.count('include!("../src/distributed_publication_contract/construction_body.rs");') == 1
         and len(re.findall(r'\binclude!\(', proof)) == 2 and not re.search(r'\binclude!\(', sources[BODY]), 'exact closed five-file nested proof closure')
    compact_identity = re.sub(r'\s+', '', identity)
    need('pubconstfnfrom_untrusted_bytes(bytes:[u8;IDENTITY_DIGEST_BYTES_V1])->Self{Self(bytes)}' in compact_identity
         and 'pubconstfndigest(self)->IdentityDigestV1{self.0}' in compact_identity
         and 'pubconstfndigest(self)->IdentityDigestV1{self.0}' in compact, 'real full-payload identity constructor and projections')
    need(all(proof.count('c.' + field + '.0.0 == vstd::array::spec_array_fill_for_copy_type::<u8, 32>(0)') == 1 for field in DIGEST_FIELDS)
         and all(proof.count('c.' + field + ' == 0') == 1 for field in INTEGER_FIELDS), 'all eleven full payloads and four integers explicitly specified')
    stripped = re.sub(r'//[^\n]*', '', proof)
    need(not re.search(r'\b(?:assume|admit|assume_specification|requires|uninterp)\b|verifier::external|#\!?\[allow', stripped), 'no fabricated premises, trusted executable adapters or diagnostic allowances')
    need(proof.count('ensures result == self.0,') == 1 and proof.count('ensures result.0 == bytes,') == 1,
         'accessor bridge proves exact complete projection, not a validity Boolean')
    bridge()

def mutations(body):
    inherited = bridge()
    rows = []
    for field in DIGEST_FIELDS:
        rows.append(('omit-' + field.replace('_', '-'), 'binding', '$coordinates.' + field + '.digest() == zero', 'false'))
    for field in INTEGER_FIELDS:
        rows.append(('omit-' + field.replace('_', '-'), 'binding', '$coordinates.' + field + ' == 0', 'false'))
    rows += [
        ('identity-error-priority', 'binding', 'return Err(DistributedPublicationContractErrorV1::ZeroIdentity);',
         'return Err(DistributedPublicationContractErrorV1::ZeroEpochOrAttempt);'),
        ('zero-integer-accepted', 'binding', 'return Err(DistributedPublicationContractErrorV1::ZeroEpochOrAttempt);',
         'return Ok(Self { coordinates: $coordinates });'),
        ('binding-attempt-substituted', 'binding', 'Ok(Self {\n            coordinates: $coordinates,\n        })',
         '{ let mut c = $coordinates; c.attempt = 1; Ok(Self { coordinates: c }) }'),
        ('zero-sequence-accepted', 'receipt', 'if $sequence == 0 {', 'if false {'),
        ('receipt-sequence-substituted', 'receipt', 'sequence: $sequence,', 'sequence: 1,'),
        ('receipt-binding-substituted', 'receipt', 'binding: $binding,',
         'binding: { let mut c = $binding.coordinates; c.attempt = 1; ModelDistributedOperationBindingV1 { coordinates: c } },'),
        ('receipt-outcome-substituted', 'receipt', 'outcome: $outcome',
         'outcome: { let _ = &$outcome; ReportedDistributedPublicationV1::Published }'),
        ('reserved-trailer-accepted', 'trailer', 'if a != 0 || b != 0 || c != 0 {', 'if { let _ = (a, b, c); false } {'),
    ]
    for tag, outcome in enumerate(('DefinitelyNotPublished', 'Published', 'Completed', 'FailedBeforePublication', 'FailedMayStillExecute', 'ParticipantLostWithUnknownPublication'), 1):
        other = 'Published' if outcome == 'Completed' else 'Completed'
        rows.append((f'tag-{tag}-substituted', 'trailer', f'{tag} => P::{outcome},', f'{tag} => P::{other},'))
    rows.append(('unknown-tag-accepted', 'trailer', '_ => return Err(DistributedPublicationContractErrorV1::InvalidOutcome),', '_ => P::Published,'))
    result = {name: (inherited.change(body, MACROS[kind], before, after), SELECTORS[kind]) for name, kind, before, after in rows}
    need(len(result) == len(set(result.values())) == MUTANT_COUNT and all(value != body for value, _ in result.values()), 'thirty distinct actual-body mutants')
    return result

def selection_notes(leaf, focus=None):
    need(focus in SELECTORS.values() and isinstance(SELECTION_NOTES, dict) and set(SELECTION_NOTES) == set(SELECTORS.values()), 'exact measured selector inventory required')
    notes = SELECTION_NOTES[focus]
    need(isinstance(notes, frozenset) and notes == frozenset(('verifying root module (selected functions)',)), 'exact measured singleton selection note required')
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES=notes)

def campaign():
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0, 'full positive count remains unmeasured')
    for focus in SELECTORS.values():
        selection_notes(types.SimpleNamespace(LOGICAL_ERRORS=set()), focus)
    module = bridge().inherited_controller()
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.mutations, module.selection_notes = mutations, selection_notes
    return module

if __name__ == '__main__':
    runpy.run_path(str(ROOT / V / 'test-distributed-publication-construction.py'))
    campaign().main()
