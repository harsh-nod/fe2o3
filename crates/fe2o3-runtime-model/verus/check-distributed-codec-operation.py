#!/usr/bin/env python3
"""Bind the whole operation codec; source calibration is not proof acceptance."""
import ast
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
SRC = Path('crates/fe2o3-runtime-model/src')
V = Path('crates/fe2o3-runtime-model/verus')
OWNER = SRC / 'distributed_publication_contract.rs'
IDENTITY = SRC / 'identity.rs'
NATIVE = SRC / 'distributed_publication_contract/codec_operation.rs'
BODY = SRC / 'distributed_publication_contract/codec_operation_body.rs'
TESTS = SRC / 'distributed_publication_contract/codec_operation_tests.rs'
PRIOR_TESTS = SRC / 'distributed_publication_contract/codec_tests.rs'
PROOF = V / 'distributed_codec_operation_v1.rs'
FIELD_GUARD = V / 'check-distributed-codec-fields.py'
FIELD_GUARD_SHA = '7edd1dd3631ce3e39f0efb3bb48fef176bcebb7cfbdc29d7203f6e4728834483'
SOURCE_FILES = 308
SOURCE_TREE_SHA = '641e9e3c94274ba46410d9312e3259bde6999387c9eca3f4963394f1b31e7d73'
EXPECTED_VERIFIED = 99
DIAGNOSTICS = V / 'distributed_codec_operation_diagnostics_v1.py'
CALIBRATION = V / 'distributed_codec_operation_calibration_v1.json'
MUTATIONS = V / 'distributed_codec_operation_mutations_v1.json'
VERIFICATION_PINS = {
    DIAGNOSTICS: 'fab8cb0d8dcb1bf5de35c04ac692cceed58886921c97dd00add8f1e2325b6cad',
    CALIBRATION: 'cff8f5a4e1c917d1e9988e6a1a67b935bf4a458193546c8b83210027fc2674a3',
    MUTATIONS: 'e89cc9740dc86fbc687e79578f9f0732f9d218b1b8dee31ce35e2bab5345b414',
}
PINS = {
    OWNER: '5801e06a532ee2e8bd5db33f7284957c27e132aeb8f6fa85d75de2077bb0f771',
    IDENTITY: '4df48a80f5bff5ff82481358ee320819de5afc38eee8e7520c50732326fc3564',
    NATIVE: 'dda8ed7fe31bc9035048ced485e231f5cec1fb76bab657091d988632828cde49',
    BODY: '8f5f2345edc0667ac7654229d3f859a742342b770b7c77444573a9506991c4d8',
    TESTS: '46ecc90b8763fdca63cfa634d0686d4ee1acafd1cf3dfbef94cb1af293fce25b',
    PRIOR_TESTS: '854a7f7bd09dbb02ea496ecc86128e691222283a7ca2087ae79997163b1eb0c0',
    PROOF: '9d08f79c29dc5a8d76c9d73686a4df044a2327e875a9f6a6fd0f94944a79b505',
}
KINDS = ('write_digest', 'read_digest', 'encode_operation', 'decode_operation')
SELECTORS = dict(zip(KINDS, ('write_digest', 'read_digest', 'encode', 'decode')))


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


def field_guard():
    path = ROOT / FIELD_GUARD
    need(path.resolve() == path and not path.is_symlink()
         and hashlib.sha256(path.read_bytes()).hexdigest() == FIELD_GUARD_SHA,
         'exact inherited field source guard')
    spec = importlib.util.spec_from_file_location('operation_inherited_fields', path)
    field = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(field)
    return field


FIELD = field_guard()
FILES = [*FIELD.FILES, BODY, PROOF]
PINS = {**{path: digest for path, digest in FIELD.PINS.items() if path != OWNER}, **PINS}


def metadata_checks(calibration, roster):
    need(roster['schema'] == 'fe2o3-operation-mutation-roster-v1'
         and calibration['schema'] == 'fe2o3-operation-diagnostic-calibration-v1'
         and type(roster['expected_full_positive_verified']) is int
         and roster['expected_full_positive_verified'] == EXPECTED_VERIFIED
         and calibration['proof_inputs'] == {str(path): PINS[path] for path in FILES}
         and calibration['classifier_sha256'] == VERIFICATION_PINS[DIAGNOSTICS]
         and calibration['field_span_helper_sha256'] == FIELD.VERIFICATION_PINS[FIELD.DIAGNOSTICS]
         and calibration['capture_prepared_sha256'] == roster['source_capture_prepared_sha256'],
         'measured full99/0, unchanged eleven-input closure and exact reviewed diagnostic helpers')
    for record, keys in (
        (calibration, ('source_capture_accepted_negative_classifications', 'source_capture_qualified_kills',
                       'qualified_kills', 'fresh_verifier_executions', 'new_cpu_executions')),
        (roster, ('negative_count_qualified',)),
    ):
        need(all(type(record[key]) is int and record[key] == 0 for key in keys),
             'historical construction and calibration are not qualified negatives or new executions')
    need(roster['body_sha256'] == PINS[BODY] and roster['compiler_or_solver_executed'] is False
         and set(roster['equivalent_omissions']) == {'finish_omission', 'initial_zero_fill_change'}
         and len(calibration['cases']) == len(roster['logical_candidates']) == 40
         and set(calibration['cases']) == set(roster['logical_candidates']),
         'exact forty-case source-only roster, with equivalent changes excluded from kill coverage')
    for name, case in roster['logical_candidates'].items():
        observed = calibration['cases'][name]
        need(case['family'] in KINDS and case['selector'] == SELECTORS[case['family']]
             and case['negative_classification_accepted'] is case['qualified_kill'] is False
             and all(observed[key] == case[key] for key in ('family', 'selector', 'body_sha256'))
             and observed['observed']['source_spans_validated'] is True
             and observed['observed']['qualified_logical_kill'] is False,
             'same actual-body substitution and calibrated function; selectors are metadata, not verifier flags')


def verification_metadata():
    buffers = {}
    for path, digest in VERIFICATION_PINS.items():
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             'ordinary canonical operation verification metadata')
        data = selected.read_bytes()
        need(hashlib.sha256(data).hexdigest() == digest, 'exact reviewed classifier/calibration/mutation metadata')
        buffers[path] = data
    calibration, roster = [json.loads(buffers[path]) for path in (CALIBRATION, MUTATIONS)]
    metadata_checks(calibration, roster)
    return calibration, roster, buffers


def operation_classifier():
    calibration, roster, buffers = verification_metadata()
    module = types.ModuleType('distributed_codec_operation_diagnostics_v1')
    module.__file__ = str(ROOT / DIAGNOSTICS)
    exec(compile(buffers[DIAGNOSTICS], module.__file__, 'exec'), module.__dict__)
    need(module.FIELD_SHA == calibration['field_span_helper_sha256'] and module.SELECTORS == SELECTORS,
         'same authenticated source-span helper and actual selected function map')
    return module


def construct_mutations(original, roster):
    need(sha(original) == PINS[BODY], 'unchanged actual operation body before mutation')
    result = {}
    for name, case in roster['logical_candidates'].items():
        need(case['family'] in KINDS and case['selector'] == SELECTORS[case['family']]
             and case['before'] and case['after'] and case['before'] != case['after'],
             'known family and nonempty nonidentity substitution')
        marker = 'macro_rules! distributed_codec_' + case['family'] + '_body_v1 {'
        need(original.count(marker) == 1, 'unique selected actual shared macro')
        start = original.index(marker)
        end = original.find('\nmacro_rules! ', start + len(marker))
        end = len(original) if end < 0 else end
        region = original[start:end]
        need(region.count(case['before']) == 1, 'one substitution within the selected actual shared body')
        body = original[:start] + region.replace(case['before'], case['after']) + original[end:]
        need(sha(body) == case['body_sha256'], 'exact calibrated mutant bytes; all other bodies unchanged')
        result[name] = (body, case['selector'])
    need(len(result) == len({body for body, selector in result.values()}) == 40,
         'forty distinct actual-body mutations, never a selected-root proof plan')
    return result


def mutations(original):
    calibration, roster, buffers = verification_metadata()
    return construct_mutations(original, roster)


def snapshot():
    paths = set(FILES)
    paths.update(path.relative_to(ROOT) for path in (ROOT / SRC).rglob('*.rs'))
    sources = {}
    for path in paths:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             'ordinary canonical operation source')
        sources[path] = selected.read_bytes().decode()
    return sources


def compact(text):
    return re.sub(r'\s+', '', re.sub(r'//[^\n]*', '', text))


def structural(sources):
    FIELD.structural(sources)
    owner, native, body, proof, identity = [sources[path] for path in (OWNER, NATIVE, BODY, PROOF, IDENTITY)]
    need(owner.count('mod codec_operation;') == owner.count('mod codec_operation_tests;') == 1,
         'actual native and focused test modules')
    forwarders = (
        'pubfncanonical_description(self)->[u8;DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1]{codec_operation::encode(self)}',
        'pubfndecode_untrusted_description(bytes:&[u8],)->Result<Self,DistributedPublicationContractErrorV1>{codec_operation::decode(bytes)}',
    )
    need(all(compact(owner).count(fragment) == 1 for fragment in forwarders),
         'actual operation methods delegate only to the shared whole-codec helpers')
    native_bodies = (
        'pub(super)fnwrite_digest(bytes:&mut[u8],offset:&mutusize,value:IdentityDigestV1){distributed_codec_write_digest_body_v1!(distributed_codec_operation_expr_v1,bytes,offset,value)}',
        'pub(super)fnread_digest(bytes:&[u8],offset:&mutusize,)->Result<IdentityDigestV1,DistributedPublicationContractErrorV1>{distributed_codec_read_digest_body_v1!(distributed_codec_operation_expr_v1,bytes,offset)}',
        'pub(super)fnencode(binding:ModelDistributedOperationBindingV1,)->[u8;DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1]{distributed_codec_encode_operation_body_v1!(distributed_codec_operation_expr_v1,debug_assert_eq,binding)}',
        'pub(super)fndecode(bytes:&[u8],)->Result<ModelDistributedOperationBindingV1,DistributedPublicationContractErrorV1>{distributed_codec_decode_operation_body_v1!(distributed_codec_operation_expr_v1,bytes)}',
    )
    need(all(compact(native).count(fragment) == 1 for fragment in native_bodies),
         'four exact native signatures and enclosing macro bodies')
    need('use super::codec_fields::{read_header, read_u64, write_header, write_u64};' in native
         and 'use super::codec_primitives::{finish, fixed, put};' in native,
         'native field and primitive imports resolve to the actual lower layer')
    need(native.count('include!("codec_operation_body.rs");') == 1
         and proof.count('include!("distributed_codec_fields_v1.rs");') == 1
         and proof.count('include!("../src/distributed_publication_contract/codec_operation_body.rs");') == 1
         and len(re.findall(r'\binclude!\(', proof)) == 2
         and not re.search(r'\binclude!\(', body), 'closed eleven-input whole-operation proof')
    for kind in KINDS:
        macro = 'distributed_codec_' + kind + '_body_v1'
        need(native.count(macro + '!') == proof.count(macro + '!') == 1
             and body.count('macro_rules! ' + macro + ' {') == 1,
             'same real executable operation macro in both native and proof')
    constants = (
        'constDISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1:&[u8]=b"FE2O3/DISTRIBUTED-OPERATION-DESCRIPTION/V1\\0";',
        'constCOORDINATE_BYTES:usize=11*32+4*8;',
        'constDISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1:usize=DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1.len()+4+COORDINATE_BYTES;',
    )
    domain_bytes = ast.literal_eval(constants[0].split('=', 1)[1].removesuffix(';'))
    need(type(domain_bytes) is bytes, 'decoded exact native byte literal')
    domain_size = len(domain_bytes)
    proof_domain = constants[0].replace(':&[u8]', f":&'static[u8;{domain_size}]")
    proof_size = constants[2].replace('DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1.len()', str(domain_size))
    need(all(compact(owner).count(value) == 1 for value in constants)
         and compact(proof).count(proof_domain) == compact(proof).count(proof_size) == 1
         and compact(proof).count(constants[1]) == 1,
         'exact native literal length binds proof array bound and equivalent size arithmetic; no spec-mode coercion')
    constructor = 'pubconstfnfrom_untrusted_digest(digest:IdentityDigestV1)->Self{Self(digest)}'
    need(compact(owner).count(constructor) == compact(identity).count(constructor) == 1
         and compact(identity).count('pubconstfnas_bytes(&self)->&[u8;IDENTITY_DIGEST_BYTES_V1]{&self.0}') == 1
         and 'pubconstIDENTITY_DIGEST_BYTES_V1:usize=32;' in compact(identity),
         'actual identity constructor and byte getter bodies, not merely signatures')
    need('fnfrom_untrusted_digest(digest:IdentityDigestV1)->(result:Self)ensuresresult.0==digest,{Self(digest)}'
         in compact(proof)
         and 'fnas_bytes(&self)->(result:&[u8;32])ensuresresult@==self.0@,{&self.0}' in compact(proof),
         'proof bridges execute the source-identical native safe expressions')
    need('verus_exec_expr!({assert($left==$right);})' in compact(proof)
         and 'verus_exec_expr,operation_checked_offset_v1,binding)' in compact(proof)
         and '$check!(offset,DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1);' in compact(body),
         'native debug predicate is proved unconditionally without an assumption')
    theorems = (
        'fnoperation_roundtrip(coordinates:UntrustedDistributedOperationCoordinatesV1)'
        '->(result:Result<ModelDistributedOperationBindingV1,E>)'
        'ensuresresult==ifzero_identity(coordinates){Err(E::ZeroIdentity)}'
        'elseifzero_integer(coordinates){Err(E::ZeroEpochOrAttempt)}'
        'else{Ok(ModelDistributedOperationBindingV1{coordinates})},'
        '{letbinding=ModelDistributedOperationBindingV1{coordinates};letbytes=encode(binding);'
        'proof{operation_coordinates_inverse(coordinates);}decode(&bytes)}',
        'fnsuccessful_decode_canonicality(bytes:&[u8])'
        '->(result:Result<[u8;DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1],E>)'
        'ensuresmatchresult{Ok(wire)=>wire@==bytes@&&operation_decision(bytes@).is_ok(),'
        'Err(error)=>operation_decision(bytes@)==Err(error),},'
        '{letbinding=decode(bytes)?;proof{operation_success_wire_inverse(bytes@);}Ok(encode(binding))}',
    )
    need(all(compact(proof).count(value) == 1 for value in theorems),
         'actual composed codec calls prove both inverses without validity or success entry premises')
    stripped = re.sub(r'//[^\n]*', '', proof)
    need(not re.search(r'\b(?:assume|admit|assume_specification|uninterp)\b|verifier::external|#\!?\[allow', stripped),
         'no new trusted shim or suppressed diagnostics')
    need(not re.search(r'\b(?:Vec|Box|unsafe|alloc)\b|std::', body + native),
         'safe fixed-storage no_std operation codec')
    need(sources[TESTS].count('#[test]') == 5, 'five focused native differential test families')


def audit(sources):
    implementation = {path: text for path, text in sources.items() if path.is_relative_to(SRC)}
    need(len(FILES) == len(set(FILES)) == 11
         and set(sources) == set(implementation) | set(FILES), 'exact source and proof-input membership')
    need(len(implementation) == SOURCE_FILES and FIELD.tree_hash(implementation) == SOURCE_TREE_SHA,
         'complete reviewed native model source roster')
    need(all(sha(sources[path]) == digest for path, digest in PINS.items()),
         'exact operation draft and unchanged field proof/native dependencies')
    structural(sources)


if __name__ == '__main__':
    audit(snapshot())
    verification_metadata()
    print('PASS: operation codec source binding (four actual bodies, eleven proof inputs, forty calibrated mutants; no compiler, proof, qualification or parity claim)')
