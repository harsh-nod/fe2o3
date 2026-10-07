#!/usr/bin/env python3
"""Candidate developer qualification for constructor-origin owner traces.

This lane is not registered in the global proof inventory. Missing reviewed
pins or whole-root measurements deliberately prevent qualification or probe
execution. Source-policy controls are distinct from solver negative controls.
"""
import argparse
import copy
import hashlib
import importlib.util
import json
import os
import re
import signal
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True
CRATE = Path('crates/fe2o3-runtime-model')
VERUS_DIR = CRATE / 'verus'
PREVIOUS = VERUS_DIR / 'check-owner-inspection.py'
CHECK = VERUS_DIR / 'check-owner-lifecycle.py'
FROZEN = 'dcebed7d2850700142e6c2477c61e2cf4a115d88'
INHERITED_INPUT_COUNT = 437
HELPER_PINS = {
    Path('crates/fe2o3-runtime-model/verus/check-journal-issuance.py'): '36d5e0641300bf2568884ec4c440c668477a4b24a07ee95273a91eab34659480',
    Path('crates/fe2o3-runtime-model/verus/check-owner-constructor.py'): 'c97879b598f65b9075675d8d22525b3aeb948605d27e648e10a233841c6ad99c',
    Path('crates/fe2o3-runtime-model/verus/check-owner-disposal.py'): 'aea7140f1c536184b9fe61186d0256eb6038189de787cdfd124fcb5c96e0050c',
    Path('crates/fe2o3-runtime-model/verus/check-owner-enrollment.py'): 'a2207602e1ca7940440d2dccd305663f0a0954c5549c6b50487519a8a581ec00',
    Path('crates/fe2o3-runtime-model/verus/check-owner-inspection.py'): 'ceaa3d04cbc5376982c150e53620c3dfbb1b41f80ae7d23c6c8740f74bb9ca0c',
    Path('crates/fe2o3-runtime-model/verus/check-owner-retirement.py'): '59cd68815d8e633b21c21045957e1862f23e71e081beb2b5ba1c552035091f84',
    Path('crates/fe2o3-runtime-model/verus/check-owner-scalar-enrollment.py'): '3a182afac2d30a7c2417ba0a980581bc9faf0d0837c225552379ae152306e959',
    Path('crates/fe2o3-runtime-model/verus/check-owner-settlement-historical.py'): '97a4049377aa94e3ebebf63f4b10ac89d5edfaaf8c23cdc26843c9035e7bb9d3',
    Path('crates/fe2o3-runtime-model/verus/check-owner-settlement.py'): '25daee4c258828db09d683c8ab9e39d26c2d5c91babfb73c0240e713a3d6c7b3',
    Path('crates/fe2o3-runtime-model/verus/check-owner-unknown.py'): '6960312d5625f433e420739d4276a395c16ee57ca3fa9c727fa4c829e58ef60d',
    Path('crates/fe2o3-runtime-model/verus/check-owner-writer.py'): '5993861ef8bc2a7ae8ede74d9d03e7697a1e411237ea148d34a60568c6270e3f',
    Path('docs/evidence/dev-producer-stable-2026-09-22/audit.py'): '0ca2c61dde9905f06df41218276fb6626f84bf60a5dbb4e7f611a6d139bbdf6f',
    Path('docs/evidence/dev-producer-stable-2026-09-22/check.py'): '983524d61d773bfdc5bfacc77a8c41b9b084f8c1f70b433b35ea65e22180149a',
    Path('docs/evidence/dev-producer-stable-2026-09-22/performance.py'): '3b94430dad0ca4a9b31ade908e7d8a36569809fa9503d476a86fc184a23cc053',
    Path('examples/wave64_collectives_v1/check-proof-source.py'): 'a3071ad8f0025a59d0c70dcf2427c1eb43b0c02fe484474a84d0377d99ccb887',
}
BEGIN_CUSTODY = VERUS_DIR / 'context_version_journal_begin_custody_v1.rs'
RELEASE_WITNESSES = VERUS_DIR / 'context_stable_release_witnesses_v1.rs'
INHERITED_ORIGINALS = {
    BEGIN_CUSTODY: ['begin_new_chain_v1', 'begin_surviving_chain_v1'],
    RELEASE_WITNESSES: ['stable_release_live_witness_v1'],
}
BEGIN_HELPERS = [
    'begin_new_chain_link_details_v1', 'begin_new_chain_members_v1', 'begin_new_chain_links_v1',
    'begin_surviving_chain_links_v1', 'begin_surviving_chain_member_details_v1',
    'begin_surviving_chain_selected_v1', 'begin_surviving_chain_members_v1',
]
INHERITED_PINS = {
    BEGIN_CUSTODY: 'f0b984eccdc0b849af719a74c66a4e5b04c54febfb6cdb9ce6c09cfaa4dff747',
    RELEASE_WITNESSES: 'f3746f456acc43222d433ca6497a20ab9fd3fcf9bfcaac2c862e7f69ce04af55',
}
INHERITED_BODY_PINS = {
    (BEGIN_CUSTODY, 'begin_new_chain_v1'): '70db4fea08b24590c37c4404b23e02f9c2870dd35d4aa86951b40ef59e7d7a98',
    (BEGIN_CUSTODY, 'begin_surviving_chain_v1'): 'bcc685bf517d0e588a7ee28573ae28e3c4e8e0383f9ed7b6a1c160b5df1ba549',
    (RELEASE_WITNESSES, 'stable_release_live_witness_v1'): '6fa01b12425803f89a19c429f8efcb20dd09b8fa20a7251b27e174d2dff1aa5f',
    (BEGIN_CUSTODY, 'begin_new_chain_link_details_v1'): '7b03f5e1cddaea5e0aafcc194a24d09f91ba819cbe3a3a7fc47da5d1906ce66c',
    (BEGIN_CUSTODY, 'begin_new_chain_members_v1'): '731725ecec67f4f70f145e9f46228d4ae2bc9918e0e5382d7f5083d11b7e17fb',
    (BEGIN_CUSTODY, 'begin_new_chain_links_v1'): '6f5ef3e4468c3d427cb6a413d19bf41cb494859c862acae627969e36fd09a345',
    (BEGIN_CUSTODY, 'begin_surviving_chain_links_v1'): 'deb2d1ecf9845f2b404518cacc0a3866ec3d490b5b102eb2430978c81642c8be',
    (BEGIN_CUSTODY, 'begin_surviving_chain_member_details_v1'): '40ac3d0e23944cf39243517fb031058e4e0732f7ada5bee4e5c79d42b02f0d93',
    (BEGIN_CUSTODY, 'begin_surviving_chain_selected_v1'): 'ebe79391fec7d58f47a8cf1ab684339103c71751930088c59b60e4e5d32a44bf',
    (BEGIN_CUSTODY, 'begin_surviving_chain_members_v1'): '89fa31c952991ec80cb13bb204d85eea83eaedee432a453a996114bf263dd65b',
}

def source(name):
    return VERUS_DIR / ('context_owner_lifecycle_' + name + '_v1.rs')


ROOT = source('paired')
ADAPTERS, MODEL, HISTORY, PRESERVATION = map(source, ('adapters', 'model', 'history', 'preservation'))
BRIDGE, OBSERVATION, TRACE, DOMAIN = map(source, ('correspondence', 'observation', 'trace', 'domain'))
WITNESSES, READER_WITNESSES = map(source, ('witnesses', 'reader_witnesses'))
LOGICAL = [ADAPTERS, MODEL, HISTORY, PRESERVATION]
PRODUCTION = [BRIDGE, OBSERVATION, TRACE, DOMAIN, WITNESSES, READER_WITNESSES]
NEW = [ROOT, *LOGICAL, *PRODUCTION]
SOURCE_COUNT = INHERITED_INPUT_COUNT + len(NEW) + 1
SCOPED_TIMEOUT_SECONDS = 600
WHOLE_TIMEOUT_SECONDS = 900

# Counts come from whole-root measurements; no automatic pin or count update.
PAIRED_VERIFIED = 1240
INSPECTION_VERIFIED = 1179
RAW_VERIFIED = 384
CONSTRUCTOR_VERIFIED = 1156
CPU_TEST_RESULTS = [(1021, 0, 18, 0, 0), (27, 0, 0, 0, 0)]
NEW_PINS = {
    ROOT: '327bc74aee5e2f9cf8b0594da7f67e4342499f673e0bd1d62ba10e3aa6aee223',
    ADAPTERS: '5b0ebd34d89e87cb9d81b69e4517a3ea00a9f22d5121c780353c0f297aa80b87',
    MODEL: '3383a92429df7e7423029aba0d5646cdc1cc5de5c8c56a584165191ab8c1afee',
    HISTORY: 'f9454845582cd0db4595cb02718b407b9f4ef22c8ebb930320c318f07bc8b518',
    PRESERVATION: '33974692b6bb09e4a4607a08942cd6c19cb3c1199f2768b44e75dec47bab4bf7',
    BRIDGE: '8d6b6d164f42b6bd5f7fbc6ee7b1c17a4248c5d5e61cb9d8e21971eb2f6aa7b3',
    OBSERVATION: '80b3247bfe5a2fae3e468592eb05c9764e6b09dd455691d5d38f6d6f5fb92c3f',
    TRACE: 'cbdaeca0d2212978045f618d8d750673e3cec1dfd7e526df1466015cc18eb490',
    DOMAIN: '847c339fa13921417d6960cb2156bc7d74eafacced63647893ffd4d9f69ebd70',
    WITNESSES: 'fc19aaa33c7e2d6a06f3a903e56740fc4e53db2f0f53ae0a0608256d74b8d50e',
    READER_WITNESSES: 'f704baade4cdb9a0240c6b98d37a0b53c866275ae563f2ddbde6a1bc8a007252',
}
READER_WITNESS_NAMES = [
    'lifecycle_reader_expected_event_v1', 'lifecycle_reader_extension_v1', 'lifecycle_reader_trace_v1',
    'lifecycle_reader_reached_v1', 'lifecycle_reader_append_v1', 'lifecycle_reader_contents_v1',
    'lifecycle_reader_origin_v1', 'lifecycle_reader_enroll_v1', 'lifecycle_reader_register_v1',
    'lifecycle_reader_begin_v1', 'lifecycle_reader_acquire_producer_v1', 'lifecycle_reader_settle_v1',
    'lifecycle_reader_reused_producer_query_v1', 'lifecycle_reader_acquire_stable_v1',
    'lifecycle_reader_release_producer_v1', 'lifecycle_reader_release_stable_v1',
    'lifecycle_reader_abort_v1', 'lifecycle_reader_complete_v1', 'lifecycle_reader_mixed_reuse_witness_v1',
]

MUTATING_FAMILIES = [
    'EnrollScalar', 'EnrollBatch', 'Retire', 'Register', 'Abort', 'Begin',
    'AcquireStable', 'ReleaseStable', 'AcquireProducer', 'ReleaseProducer',
    'SettleSuccess', 'SettleNoEffect', 'Unknown', 'Dispose',
]
GETTERS = [
    'ContextGeneration', 'AllocationCapacity', 'WriterCapacity', 'RegistrationWatermark',
    'RemainingWriterSlots', 'ReservedWriterCount', 'RemainingAllocationSlots',
    'StableRemainingReadSlots', 'StableRetainedReadCount',
    'ProducerRemainingReadSlots', 'ProducerRetainedReadCount', 'ProducerTotalReadCount',
]
QUERIES = [
    'Getter', 'InspectJournal', 'InspectStable', 'InspectProducer', 'LookupReserved',
    'LookupWriter', 'LookupAllocation', 'ReaderCount', 'StableCapacity',
    'CombinedStableCapacity', 'ProducerCapacity', 'ValidateRead', 'ValidateProducer',
    'LookupRead', 'LookupProducer', 'ProducerStatus', 'ValidateRetirement', 'ValidateDisposal',
]
ANSWERS = ['Scalar', 'Journal', 'Owner', 'Reserved', 'Writer', 'Allocation', 'Count',
           'Unit', 'StableRead', 'ProducerRead', 'Status']
STAGE_NAMES = ['lifecycle_register_fixture_step_v1', 'lifecycle_inspection_fixture_step_v1',
               'lifecycle_abort_fixture_step_v1']

# These lock reviewed admission/answer/history definitions separately from file
# pins. Rehashing a modified file cannot hide a circular or weakened contract.
DEFINITION_NAMES = {
    ADAPTERS: ['lifecycle_begin_relation_v1'],
    MODEL: ['lifecycle_register_relation_v1', 'lifecycle_abort_relation_v1',
            'lifecycle_settlement_relation_v1', 'lifecycle_step_relation_v1',
            'lifecycle_writer_history_v1', 'lifecycle_stable_minted_v1', 'lifecycle_producer_minted_v1',
            'lifecycle_next_history_v1', 'lifecycle_stable_history_v1', 'lifecycle_producer_history_v1',
            'lifecycle_invariant_v1'],
    BRIDGE: ['lifecycle_actual_step_relation_v1', 'lifecycle_step_inputs_v1', 'lifecycle_step_answers_v1'],
    OBSERVATION: ['lifecycle_getter_actual_v1', 'lifecycle_getter_model_v1',
                  'lifecycle_writer_lookup_model_v1', 'lifecycle_query_actual_v1', 'lifecycle_query_model_v1'],
    TRACE: ['lifecycle_origin_relation_v1', 'lifecycle_event_relation_v1', 'lifecycle_event_answers_v1',
            'lifecycle_event_history_v1', 'lifecycle_paired_history_v1', 'lifecycle_paired_trace_v1'],
    DOMAIN: ['lifecycle_origin_input_shape_v1', 'lifecycle_mutation_input_shape_v1',
             'lifecycle_mutation_domain_v1', 'lifecycle_query_input_shape_v1', 'lifecycle_query_domain_v1',
             'lifecycle_event_input_shape_v1', 'lifecycle_event_domain_v1'],
    WITNESSES: ['lifecycle_constructor_origin_fixture_v1', *STAGE_NAMES,
                'lifecycle_mixed_trace_witness_v1', 'lifecycle_failed_constructor_trace_witness_v1'],
    READER_WITNESSES: READER_WITNESS_NAMES,
}
DEFINITION_PINS = {
    (ADAPTERS, 'lifecycle_begin_relation_v1'): 'bcbddc2e7cfeee51aa55be239646a74a40725015e3db5a2e4505d3863102cf68',
    (MODEL, 'lifecycle_register_relation_v1'): 'a55c4dce9fb79747641a4ef32b66595e4f43d18632effa527b6dfb29860eeb7f',
    (MODEL, 'lifecycle_abort_relation_v1'): 'c0fdb9843eaac612246286941c93bf3cc30d16327e635dd7398a2f5d6e53ec2c',
    (MODEL, 'lifecycle_settlement_relation_v1'): '5e822c2c5a2fb4586c1e80ff3e45d6dc8f31e48020d509272b9808c953e43ee4',
    (MODEL, 'lifecycle_step_relation_v1'): '7f59f8daf96b745ebedf825166c12d3957462a984c8941ae5b7da6c8314257ba',
    (MODEL, 'lifecycle_writer_history_v1'): '1929d8ff974562b5f6582308ac715596ea49a742c1a4b1a781aa0dae1c777eca',
    (MODEL, 'lifecycle_stable_minted_v1'): 'ddb3c307c41aeabfe029af1269049435d01341d846c0ef3611b43c9b703bbe5a',
    (MODEL, 'lifecycle_producer_minted_v1'): '00ae280a427319ac3641712978fd495616f88462009c16bb8d02ecd49b99a6ff',
    (MODEL, 'lifecycle_next_history_v1'): '27c69deebdd2a2ac71f0a02c16fc1e2149d82c70f1b375e7d32c424c0ac74f10',
    (MODEL, 'lifecycle_stable_history_v1'): '30e6282da78fe3afe652654b350974d449d0ad859e80887ac9415e7728e1cea7',
    (MODEL, 'lifecycle_producer_history_v1'): '91d0f9c065c04e2d9954bdfcab176070173980bb44c4f2c5dca739381a6632a8',
    (MODEL, 'lifecycle_invariant_v1'): '350df3d4e2621851d93b9a4205be8621894a15209dc2baa3cc333a2d89be9007',
    (BRIDGE, 'lifecycle_actual_step_relation_v1'): 'bb783e2a394de18388f4a48e91ec198104b5015b2cddb0a92a157fecc417615d',
    (BRIDGE, 'lifecycle_step_inputs_v1'): '3b7302b387657fa9b0ac01b948c6267125568f1996405c8179ed13d337ef6ffa',
    (BRIDGE, 'lifecycle_step_answers_v1'): '2cfd6f8a3f07b5fe8f325afcf08e96b393f52a17a060ed16d045a69ab2411120',
    (OBSERVATION, 'lifecycle_getter_actual_v1'): '22f84f844622fa43b2ab77041666cbb7a49468f1b45c421846090665770e60cb',
    (OBSERVATION, 'lifecycle_getter_model_v1'): '9ab7e2fc9a7b0a9a6e5c20c7d0c06b1626593ed5358a7da3909ef14508472a15',
    (OBSERVATION, 'lifecycle_writer_lookup_model_v1'): '3f69227bfa8afea0244aa37662c1aad8cc97d1ef42086fed1d8048aa0425afdb',
    (OBSERVATION, 'lifecycle_query_actual_v1'): '84c74e8f61d5022e64badd46647c1c34d6858b12149a6a7ff3fcfdd39106f39c',
    (OBSERVATION, 'lifecycle_query_model_v1'): '17b85b1300d72ca9b3bac3f11912d5a3eaafa0a5074639e2f8057e797e3017bc',
    (TRACE, 'lifecycle_origin_relation_v1'): '8be4a093fb812014b340a071500840e4433c2665ab4c1ae8b1b4d2dec1b281c2',
    (TRACE, 'lifecycle_event_relation_v1'): '2e420d1af3e50206a283b14bae368796926bf4cefa2d8817b171a189157fa683',
    (TRACE, 'lifecycle_event_answers_v1'): 'd8cc1d048e811e268441be76b6c6c98f57098d1d25de3bf0b39f9ca1edf2a1a8',
    (TRACE, 'lifecycle_event_history_v1'): 'aa4d7baa8670f3b3487bd4fd9633335a47dc844e22731ee022851b0a0bde245e',
    (TRACE, 'lifecycle_paired_history_v1'): 'c44bfd067a4c0f0adaaf7ff5faa125dbf253b1a2caac1138004f887aa00afe63',
    (TRACE, 'lifecycle_paired_trace_v1'): 'e80ddc6592329b084debfd53ceb5782beb3cbb1123e8210de43691351c555a26',
    (DOMAIN, 'lifecycle_origin_input_shape_v1'): '103fbb072512495f46a64183c6551c601d3926ede92be1df63d5562263c0efbb',
    (DOMAIN, 'lifecycle_mutation_input_shape_v1'): 'd143812a527f8ce5ab9f2bb8fec182ad17373e0b32d9f693de94338ef8d3c3a9',
    (DOMAIN, 'lifecycle_mutation_domain_v1'): '185e909a026e753cf32b767dfaf6b4e9af75551a821ac657bd0aa1435f3f48a8',
    (DOMAIN, 'lifecycle_query_input_shape_v1'): 'dbb2059b6ac0f5f7341c048dd5fa7e5beed892aacb67a8b41b72a461bffec443',
    (DOMAIN, 'lifecycle_query_domain_v1'): 'd5f0e29196f03eda2ade991f391df31d8201157499e42d02cb13cdaf23444e48',
    (DOMAIN, 'lifecycle_event_input_shape_v1'): 'c25f65a09448a2e306152fc78c969025c9df28b2926b7ae636a2994fdee3fb12',
    (DOMAIN, 'lifecycle_event_domain_v1'): '38e6a699749b8b1c2ce2f4ac1a9d56a31e75fd2867f4cabd37205df306e1cfd1',
    (WITNESSES, 'lifecycle_constructor_origin_fixture_v1'): '2c6062d11c4c5b978e4de71c39fc4221ae13a3ece2f8749f5cd63d914805ec37',
    (WITNESSES, 'lifecycle_register_fixture_step_v1'): '0e05ffb88fe5903c80af8fe78fac1b6e1565dd877e4eb7c376f283aea88c3db8',
    (WITNESSES, 'lifecycle_inspection_fixture_step_v1'): '9a2a19e364f0e6885090a412274d51dc38510d9a8dcc7761ed07c2abf45fe741',
    (WITNESSES, 'lifecycle_abort_fixture_step_v1'): 'd3b21002b50b7a69baf04c01a5ac7683634835d68118a2ec7e118e9d93fd04a1',
    (WITNESSES, 'lifecycle_mixed_trace_witness_v1'): 'fb2195368598122387dbc5248e8ba40cab33619307bd2043c91a655a1c62e86a',
    (WITNESSES, 'lifecycle_failed_constructor_trace_witness_v1'): '3f6c93a9e8c8a1b45207afbc8a41758cffa922b33097a1b454ac0e57ecaf56c8',
    (READER_WITNESSES, 'lifecycle_reader_expected_event_v1'): 'b36950879920e1e04cf12a947012b4909599ecb25bcc88dd023db8694cb63d4a',
    (READER_WITNESSES, 'lifecycle_reader_extension_v1'): '667a263e6a12602f2e3065c9b6f58efad25c6d12d6a1d7e3fec93490ca11897c',
    (READER_WITNESSES, 'lifecycle_reader_trace_v1'): '7b23cdf84df5ddd66dd19aeb5f1b1f7a7466c5fa191d0408ad4297dee97ec812',
    (READER_WITNESSES, 'lifecycle_reader_reached_v1'): 'b42368e4aa6c977e0694e9070852d1428d27f7eaa6479c382e560aa32c026219',
    (READER_WITNESSES, 'lifecycle_reader_append_v1'): '90f9964dd614a5153b0be7502a4b46a2af813f689fb2d53c3acffee1ef3e851f',
    (READER_WITNESSES, 'lifecycle_reader_contents_v1'): '76b6b56c08ba4036e697e2a39177cb3f9eb29b66ebeb4c828696b7e95802ebf3',
    (READER_WITNESSES, 'lifecycle_reader_origin_v1'): 'a4b8ebd0d8f0a75942def78b516f6c8fb72c55e62b5fc692c45779f84451a6c0',
    (READER_WITNESSES, 'lifecycle_reader_enroll_v1'): 'a96d189bdb800ae90f5b13667ba9dc6b7bb3a7924bd2b2c45a5f597621e419e3',
    (READER_WITNESSES, 'lifecycle_reader_register_v1'): '037b07ba2eee28e61c169fa8c05c0f7be8ef9da5956532f5bca74bdf56583585',
    (READER_WITNESSES, 'lifecycle_reader_begin_v1'): 'eeaff6c8b6be5871dcb4bbad41ce7ab2770feaab688e10d6fcadf31f9b50d311',
    (READER_WITNESSES, 'lifecycle_reader_acquire_producer_v1'): '2266bc86ce85b1239bbc2fa3a0ea801105745020a196c74bdf9aed8f0df62369',
    (READER_WITNESSES, 'lifecycle_reader_settle_v1'): '97e4f7905de32432590a252bd1db83f0530f2ac57c0b8aba4eea9e4ac820ae01',
    (READER_WITNESSES, 'lifecycle_reader_reused_producer_query_v1'): '0fc4d6a1e72ae605a187452a8ced75b41fa915cc946c1059f85f73ebb6b1af1e',
    (READER_WITNESSES, 'lifecycle_reader_acquire_stable_v1'): 'a6df2d74809b39564dded0e3004d63c8411ce959b715b3dd7b2d381493a8a5e9',
    (READER_WITNESSES, 'lifecycle_reader_release_producer_v1'): 'adf9c9ff606a55cb312611595ab0e06fbb45ea691606e4956a09d57d7cf0f155',
    (READER_WITNESSES, 'lifecycle_reader_release_stable_v1'): 'cbfe2159685f881be120370f63ac51095bac590e7b8bee5397d3568be500d8b9',
    (READER_WITNESSES, 'lifecycle_reader_abort_v1'): '4567eb53bf0a1d30999e09da565c06a298d0fb111b06432c14d4d44d3483700f',
    (READER_WITNESSES, 'lifecycle_reader_complete_v1'): '9b9ee37ea2227d5ed0661e840da54412c3291d1b550abd697ea4a427598ab29a',
    (READER_WITNESSES, 'lifecycle_reader_mixed_reuse_witness_v1'): '73f14d904246b474d5d87f7bf92248e60af7732655169fd65268e73a8e711ffd',
}
CONTRACT_NAMES = {
    BRIDGE: ['lifecycle_mutation_correspondence_v1'],
    TRACE: ['lifecycle_origin_correspondence_v1', 'lifecycle_event_preserves_v1',
            'lifecycle_paired_prefix_v1', 'lifecycle_constructor_origin_trace_v1',
            'lifecycle_paired_history_nonreissue_v1'],
    DOMAIN: ['lifecycle_mutation_domain_from_reached_v1', 'lifecycle_query_domain_from_reached_v1',
             'lifecycle_reached_operation_domains_v1'],
    WITNESSES: ['lifecycle_constructor_origin_fixture_v1', 'lifecycle_three_event_trace_v1',
                *STAGE_NAMES, 'lifecycle_mixed_trace_witness_v1', 'lifecycle_failed_constructor_trace_witness_v1'],
}
CONTRACT_PINS = {
    (BRIDGE, 'lifecycle_mutation_correspondence_v1'): 'db418aedabe755a0a5fc9341c5dc81aa23dec050d41eb7ed66e59f6e9bf225ce',
    (TRACE, 'lifecycle_origin_correspondence_v1'): '6e1359edc9418bcd70c951ebb3f5e08673a934bb046b0eb269d029f2f9fcab87',
    (TRACE, 'lifecycle_event_preserves_v1'): '4d6ce5aacc1fee31f72e63748664430e2dc687321c138670cc4cc039faf39c47',
    (TRACE, 'lifecycle_paired_prefix_v1'): 'ed3cd42a39b2f942871cf3c767f1dd915635e3f1f9beb7758886085b37befc65',
    (TRACE, 'lifecycle_constructor_origin_trace_v1'): '7f9c0a98e1d4f9823cba50878397e8141f3ed2227890f9bfe5dfb63d7fb19fb1',
    (TRACE, 'lifecycle_paired_history_nonreissue_v1'): '47ed29d1202979c4398bdd9d0bff4d95fb6a59263f1ecfcca27a166e660d1bbe',
    (DOMAIN, 'lifecycle_mutation_domain_from_reached_v1'): '51f90d7293948a60a65981a7a936568f8088d8fb1611cb8025b287bd8cafc948',
    (DOMAIN, 'lifecycle_query_domain_from_reached_v1'): '5a66297ad6f61e552afdacd8fef33b0cca12569c5cad2292813fa601e542a121',
    (DOMAIN, 'lifecycle_reached_operation_domains_v1'): '91329ecbe3a6eb5cf962cab57ceab87ca762a5118e1710a8a3baab5c3b677acf',
    (WITNESSES, 'lifecycle_constructor_origin_fixture_v1'): 'cc957fb83a4f9d38a4be726ff9168492717c796e6ae388b5f5051f6ea9350812',
    (WITNESSES, 'lifecycle_three_event_trace_v1'): 'cc1156677d6f7a82540836b1aff6912c29fe844a84ea005228dcea8fdd71d89a',
    (WITNESSES, 'lifecycle_register_fixture_step_v1'): '87677d57bc84d0504a9bdbac8aaabf81613429fee249f6226908c5a10db64d7b',
    (WITNESSES, 'lifecycle_inspection_fixture_step_v1'): '63e09d473c2760285af66f7b133cae48372ff1299669851eb7004a9b29134da7',
    (WITNESSES, 'lifecycle_abort_fixture_step_v1'): '5ad4adb4dd8cb75b422313e320d0d00007481d765e6dec67b0344b50b5005859',
    (WITNESSES, 'lifecycle_mixed_trace_witness_v1'): '77a93079800fc993bb16fbaa0e6c25c0aab1e01fe1195ca48d617274f9b51f97',
    (WITNESSES, 'lifecycle_failed_constructor_trace_witness_v1'): '6836c48f00149df29592c3da31f7ebf71f42ddc6b45426137bcfb914982269ef',
}
# Reviewed reader contracts include exact event and prefix extensions.
READER_SPEC_NAMES = ['lifecycle_reader_expected_event_v1', 'lifecycle_reader_extension_v1',
                     'lifecycle_reader_trace_v1', 'lifecycle_reader_contents_v1']
READER_CONTRACT_PINS = {
    'lifecycle_reader_reached_v1': '91feac80b10a32bfd14af0251f861a4c258e42c341ee8dc02489081af9aad386',
    'lifecycle_reader_append_v1': '340d271280b726a9c0378f0841f9cea0611ae8123f6724305b9e660d57bec0bc',
    'lifecycle_reader_origin_v1': '41d484c4078fa465ddeb34cc2ad038bdb2d620b727e7acc59e5ea57a4812d536',
    'lifecycle_reader_enroll_v1': '29f0213b69b1de658d340c4ff10b132900447de2d892d684742ee0a842c8d46b',
    'lifecycle_reader_register_v1': '828c5cda1166e47d013757c0ddca2ea069b9809d13b637afa1b0a9f62e1eee29',
    'lifecycle_reader_begin_v1': '4e78eca3b38226e228822df2c0773221810d7ae775a7021e6ef3203a34bec1cc',
    'lifecycle_reader_acquire_producer_v1': 'f65336994e68260646fa5fd95e4a0f0e43c53ffbaaca69dc710e02da123560ca',
    'lifecycle_reader_settle_v1': 'a3caa40caaf1a8abe80448bee677c554cf0f7af201b8d6ea3a776906de186fd9',
    'lifecycle_reader_reused_producer_query_v1': '35dfc726f5f5ee0f81fda3a14004c9cfdade774b7ca22639233784f8391fbcc9',
    'lifecycle_reader_acquire_stable_v1': 'e2ddb9e80bedbdb0ae4150b7ebcb1d5565ce8801c4e1339ed171d3879c89f567',
    'lifecycle_reader_release_producer_v1': '5272e1966b24d8edd56bd6ebdeec44e2f9ef7208faea134e7605c8b867a7b11b',
    'lifecycle_reader_release_stable_v1': '79dc2018a3ac4004c2c04a3ca672bbb89ac24e7fcfe3093b35a57bed91133e06',
    'lifecycle_reader_abort_v1': 'e2e580f1758ab678ba5967e761d8197d6be03135164da5300eb3bbb680d94000',
    'lifecycle_reader_complete_v1': '6b88912845eae3212658ac78bc367f17679b036da98b5bc07f2b8496846c84fc',
    'lifecycle_reader_mixed_reuse_witness_v1': 'f1a622853c9f8e7d14f3a05d95c619db8faa968f8a77ef1155a13be9ee0d950c',
}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def git_environment():
    return {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'GIT_NO_REPLACE_OBJECTS': '1',
            'GIT_OPTIONAL_LOCKS': '0', 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': os.devnull}


def git_output(repo, *arguments, text=False):
    return subprocess.check_output(['/usr/bin/git', '-C', str(repo), *arguments],
                                   env=git_environment(), text=text)


def authenticate_helper(repo, path):
    need(path in HELPER_PINS and not path.is_absolute() and '..' not in path.parts,
         'registered lifecycle Python helper')
    need(not any((repo / prefix).is_symlink() for prefix in (path, *path.parents)),
         'no lifecycle Python helper links: ' + str(path))
    need(stat.S_ISREG((repo / path).lstat().st_mode), 'ordinary lifecycle Python helper: ' + str(path))
    need(hashlib.sha256((repo / path).read_bytes()).hexdigest() == HELPER_PINS[path],
         'exact lifecycle Python helper: ' + str(path))


def authenticate_helpers(repo):
    need(len(HELPER_PINS) == 15 and all(re.fullmatch(r'[0-9a-f]{64}', pin) for pin in HELPER_PINS.values()),
         'closed reviewed Python helper pins')
    for path in HELPER_PINS:
        authenticate_helper(repo, path)


def module(repo, path, name):
    authenticate_helper(repo, path)
    spec = importlib.util.spec_from_file_location(name, repo / path)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


authenticate_helpers(Path(__file__).resolve().parents[3])
INSPECTION = module(Path(__file__).resolve().parents[3], PREVIOUS, 'lifecycle_inspection')
check_negative = INSPECTION.check_negative


def dependencies(repo):
    authenticate_helpers(repo)
    result = INSPECTION.dependencies(repo)
    result['inspection'] = INSPECTION
    return result


def reviewed_sources_configuration():
    need(SCOPED_TIMEOUT_SECONDS == INSPECTION.SCOPED_TIMEOUT_SECONDS
         and WHOLE_TIMEOUT_SECONDS == INSPECTION.WHOLE_TIMEOUT_SECONDS,
         'unchanged inherited lifecycle command budgets')
    need(READER_WITNESS_NAMES is not None and len(READER_WITNESS_NAMES) == len(set(READER_WITNESS_NAMES))
         and 'lifecycle_reader_mixed_reuse_witness_v1' in READER_WITNESS_NAMES, 'reviewed reader witness roster required')
    need(type(READER_CONTRACT_PINS) is dict and READER_CONTRACT_PINS, 'reviewed reader trace contracts required')
    need(set(NEW_PINS) == set(NEW), 'closed lifecycle source pins')
    need(set(DEFINITION_PINS) == {(path, name) for path, names in DEFINITION_NAMES.items() for name in names},
         'closed lifecycle definition pins')
    need(set(CONTRACT_PINS) == {(path, name) for path, names in CONTRACT_NAMES.items() for name in names},
         'closed lifecycle contract pins')
    need(set(READER_CONTRACT_PINS) == set(READER_WITNESS_NAMES) - set(READER_SPEC_NAMES),
         'exact reader contract roster')
    need(set(INHERITED_PINS) == set(INHERITED_ORIGINALS) == {BEGIN_CUSTODY, RELEASE_WITNESSES},
         'exact inherited proof edit paths')
    need(len(BEGIN_HELPERS) == len(set(BEGIN_HELPERS)) == 7, 'seven Begin proof helpers')
    expected_bodies = {(path, name) for path, names in INHERITED_ORIGINALS.items() for name in names}
    expected_bodies |= {(BEGIN_CUSTODY, name) for name in BEGIN_HELPERS}
    need(set(INHERITED_BODY_PINS) == expected_bodies and len(expected_bodies) == 10,
         'exact inherited proof body roster')
    for pins in (NEW_PINS, DEFINITION_PINS, CONTRACT_PINS, READER_CONTRACT_PINS,
                 INHERITED_PINS, INHERITED_BODY_PINS):
        need(all(type(pin) is str and re.fullmatch(r'[0-9a-f]{64}', pin) for pin in pins.values()),
             'final reviewed lifecycle pins required')


def configuration():
    reviewed_sources_configuration()
    for name, value in [('PAIRED_VERIFIED', PAIRED_VERIFIED), ('INSPECTION_VERIFIED', INSPECTION_VERIFIED),
                        ('RAW_VERIFIED', RAW_VERIFIED), ('CONSTRUCTOR_VERIFIED', CONSTRUCTOR_VERIFIED)]:
        need(type(value) is int and value > 0, 'final measured count required: ' + name)
    need(CPU_TEST_RESULTS == INSPECTION.CPU_TEST_RESULTS, 'unchanged CPU qualification inventory')


def inherited_sources(repo, base):
    # The inspection packet is not required to exist: reconstruct its exact
    # signed input closure from its frozen checker and inherited packet.
    frozen_checker = git_output(repo, 'show', FROZEN + ':' + str(PREVIOUS))
    need((repo / PREVIOUS).read_bytes() == frozen_checker, 'frozen inspection checker')
    frozen_inputs = git_output(repo, 'show', INSPECTION.FROZEN + ':' + str(INSPECTION.FROZEN_INPUTS), text=True)
    inherited = {Path(path) for path in base.unique_json(frozen_inputs)['inputs']}
    inherited |= {INSPECTION.FROZEN_INPUTS, PREVIOUS} | set(INSPECTION.NEW)
    need(len(inherited) == INHERITED_INPUT_COUNT, 'exact inherited inspection roster')
    need({path for path in inherited if path.suffix == '.py'} == set(HELPER_PINS), 'exact inherited Python closure')
    return inherited


def definition(data, name, policy):
    code = policy.code_only(data)
    pattern = r'^(?:pub\s+)?(?:(?:open|closed)\s+)?(?:(?:proof|spec)\s+)?fn\s+' + re.escape(name) + r'(?:<[^\n]*>)?\('
    matches = list(re.finditer(pattern, code, re.M))
    need(len(matches) == 1, 'unique lifecycle declaration: ' + name)
    start = matches[0].start()
    end = code.find('\n}\n', start)
    need(end >= start, 'top-level lifecycle closing brace: ' + name)
    return code[start:end + 2]


def contract(data, name, policy):
    block = definition(data, name, policy)
    need('\n{\n' in block, 'explicit contract/body boundary: ' + name)
    return block.split('\n{\n', 1)[0]


def raw_definition(data, name, policy):
    pattern = r'^(?:pub\s+)?(?:(?:open|closed)\s+)?(?:(?:proof|spec)\s+)?fn\s+' + re.escape(name) + r'(?:<[^\n]*>)?\('
    matches = list(re.finditer(pattern, data, re.M))
    need(len(matches) == 1, 'unique unprojected lifecycle declaration: ' + name)
    start = matches[0].start()
    end = data.find('\n}\n', start)
    need(end >= start, 'unprojected lifecycle closing brace: ' + name)
    result = data[start:end + 2]
    need(policy.code_only(result) == definition(data, name, policy), 'exact source/code projection: ' + name)
    return result


def enum_variants(data, name, policy):
    code = policy.code_only(data)
    anchors = list(re.finditer(r'\benum\s+' + re.escape(name) + r'\s*\{', code))
    need(len(anchors) == 1, 'unique lifecycle enum: ' + name)
    depth, parens, expect, result = 0, 0, True, []
    for token in re.findall(r'[A-Za-z_][A-Za-z_0-9]*|[{}(),]', code[anchors[0].end():]):
        if token == '}' and depth == 0:
            return result
        if expect and re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*', token):
            result.append(token)
            expect = False
        if token == '{':
            depth += 1
        elif token == '}':
            depth -= 1
        elif token == '(':
            parens += 1
        elif token == ')':
            parens -= 1
        elif token == ',' and depth == parens == 0:
            expect = True
    raise ValueError('unterminated lifecycle enum: ' + name)


def exact_members(block, enum, expected):
    actual = re.findall(r'\b' + re.escape(enum) + r'::([A-Za-z_0-9]+)', block)
    need(sorted(actual) == sorted(expected), 'complete lifecycle arms: ' + enum)


def source_policy(repo, deps):
    reviewed_sources_configuration()
    scalar, policy = deps['scalar'], deps['policy']
    data = {path: (repo / path).read_text() for path in NEW}
    for path in LOGICAL:
        INSPECTION.independent_model(data[path], policy)
    for path, name, expected in [(MODEL, 'LifecycleStepV1', MUTATING_FAMILIES),
                                 (BRIDGE, 'LifecycleActualStepV1', MUTATING_FAMILIES),
                                 (OBSERVATION, 'LifecycleGetterV1', GETTERS),
                                 (OBSERVATION, 'LifecycleQueryV1', QUERIES),
                                 (OBSERVATION, 'LifecycleReceiverV1', ['Journal', 'Stable', 'Producer']),
                                 (OBSERVATION, 'LifecycleAnswerV1', ANSWERS),
                                 (TRACE, 'LifecyclePairedEventV1', ['Mutation', 'Observe'])]:
        need(enum_variants(data[path], name, policy) == expected, 'closed lifecycle alphabet: ' + name)
    for path, name, enum, expected in [
        (MODEL, 'lifecycle_step_relation_v1', 'LifecycleStepV1', MUTATING_FAMILIES),
        (BRIDGE, 'lifecycle_actual_step_relation_v1', 'LifecycleActualStepV1', MUTATING_FAMILIES),
        (BRIDGE, 'lifecycle_step_inputs_v1', 'LifecycleActualStepV1', MUTATING_FAMILIES),
        (BRIDGE, 'lifecycle_step_inputs_v1', 'logical::LifecycleStepV1', MUTATING_FAMILIES),
        (BRIDGE, 'lifecycle_step_answers_v1', 'LifecycleActualStepV1', MUTATING_FAMILIES),
        (OBSERVATION, 'lifecycle_query_actual_v1', 'LifecycleQueryV1', QUERIES),
        (OBSERVATION, 'lifecycle_query_model_v1', 'LifecycleQueryV1', QUERIES),
        (OBSERVATION, 'lifecycle_getter_actual_v1', 'LifecycleGetterV1', GETTERS),
        (OBSERVATION, 'lifecycle_getter_model_v1', 'LifecycleGetterV1', GETTERS),
    ]:
        exact_members(definition(data[path], name, policy), enum, expected)
    inputs = definition(data[BRIDGE], 'lifecycle_step_inputs_v1', policy)
    need(re.search(r'\b(?:result|output)\b|\.is_(?:ok|err)\s*\(', inputs) is None,
         'input matching cannot assume answers or final buffers')
    for path, name in [(TRACE, 'lifecycle_origin_relation_v1'), (TRACE, 'lifecycle_event_relation_v1'),
                       (TRACE, 'lifecycle_paired_trace_v1'), (MODEL, 'lifecycle_step_relation_v1')]:
        block = definition(data[path], name, policy)
        need(re.search(r'\b(?:producer_represents|producer_invariant_v1|lifecycle_invariant_v1|issued_producer_v1|'
                       r'storage_admission_v1|lifecycle_event_answers_v1|lifecycle_step_answers_v1|'
                       r'constructor_producer_results_match_v1)\b', block) is None,
             'raw admission cannot assume invariant or matched answers: ' + name)
    for name in ('lifecycle_getter_model_v1', 'lifecycle_writer_lookup_model_v1', 'lifecycle_query_model_v1'):
        block = definition(data[OBSERVATION], name, policy)
        need(re.search(r'\bproduction\s*::|\blifecycle_(?:getter|query)_actual_v1\s*\(', block) is None,
             'independent observation computation: ' + name)
    for pins, extractor in ((DEFINITION_PINS, definition), (CONTRACT_PINS, contract)):
        for (path, name), pin in pins.items():
            need(type(pin) is str and scalar.digest(extractor(data[path], name, policy).encode()) == pin,
                 'reviewed lifecycle admission/contract: ' + name)
    need(type(READER_CONTRACT_PINS) is dict and READER_CONTRACT_PINS, 'reviewed reader trace contracts required')
    for name, pin in READER_CONTRACT_PINS.items():
        need(scalar.digest(contract(data[READER_WITNESSES], name, policy).encode()) == pin,
             'reviewed reader trace contract: ' + name)
    reader_names = re.findall(r'^(?:(?:proof|spec) )?fn (lifecycle_reader_[A-Za-z_0-9]+)',
                              policy.code_only(data[READER_WITNESSES]), re.M)
    need(reader_names == READER_WITNESS_NAMES, 'exact reader witness function roster')
    for path, name in [(WITNESSES, 'lifecycle_mixed_trace_witness_v1'),
                       (WITNESSES, 'lifecycle_failed_constructor_trace_witness_v1'),
                       (READER_WITNESSES, 'lifecycle_reader_mixed_reuse_witness_v1')]:
        need(re.search(r'\brequires\b', contract(data[path], name, policy)) is None, 'no-precondition witness: ' + name)
    stages = re.findall(r'^fn (lifecycle_[A-Za-z_0-9]+_fixture_step_v1)\(', data[WITNESSES], re.M)
    need(stages == STAGE_NAMES, 'exact three executable fixture stages')


def source_roster(repo, inherited):
    INSPECTION.source_roster(repo, inherited)
    discovered = {p.relative_to(repo) for p in (repo / VERUS_DIR).glob('context_owner_lifecycle_*_v1.rs')}
    need(discovered == set(NEW), 'closed lifecycle source roster')
    need(not any((repo / path).is_symlink() for path in inherited | set(NEW) | {CHECK}), 'normal lifecycle source files')


def authenticate_proof_edit(path, original, current, deps):
    policy, scalar = deps['policy'], deps['scalar']
    need(path in INHERITED_ORIGINALS, 'registered inherited proof edit')
    for name in INHERITED_ORIGINALS[path]:
        need(contract(current, name, policy) == contract(original, name, policy),
             'unchanged inherited proof contract: ' + name)
    for (actual_path, name), pin in INHERITED_BODY_PINS.items():
        if path == actual_path:
            need(scalar.digest(definition(current, name, policy).encode()) == pin,
                 'reviewed inherited proof body: ' + name)
    restored = current
    if path == BEGIN_CUSTODY:
        for name in BEGIN_HELPERS:
            block = '#[verifier::spinoff_prover]\n' + raw_definition(restored, name, policy) + '\n\n'
            restored = INSPECTION.replace_once(restored, block, '')
    for name in INHERITED_ORIGINALS[path]:
        restored = INSPECTION.replace_once(restored, raw_definition(restored, name, policy),
                                           raw_definition(original, name, policy))
    need(restored == original, 'unchanged inherited proof surroundings: ' + str(path))
    need(scalar.digest(current.encode()) == INHERITED_PINS[path], 'exact inherited proof source: ' + str(path))


def authenticate(repo, inherited, deps, git_repo=None):
    git_repo = git_repo or repo
    need((repo / CHECK).read_bytes() == Path(__file__).read_bytes(), 'exact running lifecycle checker')
    for path in sorted(inherited):
        expected = git_output(git_repo, 'show', FROZEN + ':' + str(path))
        current = (repo / path).read_bytes()
        if path in INHERITED_ORIGINALS:
            authenticate_proof_edit(path, expected.decode(), current.decode(), deps)
        else:
            need(current == expected, 'unchanged inspection input: ' + str(path))
    expected = git_output(git_repo, 'show', FROZEN + ':' + str(INSPECTION.ROOT), text=True)
    for name in ('owner_enrollment_invariant', 'owner_settlement_historical_bodies',
                 'owner_scalar_enrollment_model', 'owner_retirement_model', 'owner_retirement_invariant',
                 'owner_disposal_model', 'owner_disposal_invariant', 'owner_constructor_model'):
        expected = INSPECTION.replace_once(expected, 'use ' + name + '::*;\n\n#[path',
                                           'use ' + name + '::*;\n#[path')
    addition = ''.join('\n#[path = "' + path.name + '"]\nmod owner_lifecycle_' + name + ';\nuse owner_lifecycle_' + name + '::*;'
                       for path, name in zip(LOGICAL, ('adapters', 'model', 'history', 'preservation')))
    expected = INSPECTION.replace_once(expected, 'use owner_inspection_model::*;', 'use owner_inspection_model::*;' + addition)
    anchor = '    include!("context_owner_inspection_witnesses_v1.rs");'
    expected = INSPECTION.replace_once(expected, anchor, anchor + ''.join('\n    include!("' + path.name + '");' for path in PRODUCTION))
    need((repo / ROOT).read_text() == expected, 'exact lifecycle paired root extension')
    source_roster(repo, inherited)
    source_policy(repo, deps)
    for path, pin in NEW_PINS.items():
        need(type(pin) is str and deps['scalar'].digest((repo / path).read_bytes()) == pin,
             'exact reviewed lifecycle source: ' + str(path))


def scan_sources(stage, deps):
    INSPECTION.scan_sources(stage, deps)
    for path in NEW:
        if path != ROOT:
            deps['disposal'].scan_leaf(stage, path, deps['policy'])
    for path in LOGICAL:
        INSPECTION.independent_model((stage / path).read_text(), deps['policy'])


def mutations(repo, policy):
    rows = []
    def add(name, path, declaration, module_name, function_name, before, after):
        block = definition((repo / path).read_text(), declaration, policy)
        need(block.count(before) == 1 and before != after, 'unique lifecycle mutation: ' + name)
        anchor = re.search(r'^(?:pub\s+)?(?:(?:open|closed)\s+)?(?:(?:proof|spec)\s+)?fn\s+' + re.escape(declaration) + r'\(', block)[0]
        rows.append((name, path, anchor, module_name, function_name, before, after, 'logical'))
    for path, declaration, enum, module_name, target, prefix in [
        (BRIDGE, 'lifecycle_actual_step_relation_v1', 'LifecycleActualStepV1', 'production', 'lifecycle_mutation_correspondence_v1', 'actual'),
        (MODEL, 'lifecycle_step_relation_v1', 'LifecycleStepV1', 'owner_lifecycle_preservation', 'lifecycle_step_issued_v1', 'model')]:
        block = definition((repo / path).read_text(), declaration, policy)
        for family in MUTATING_FAMILIES:
            pattern = r'(' + enum + '::' + family + r'\s*\{[^}]*\}\s*=>\s*)(.*?)(?=,\n        ' + enum + r'::|,\n    \})'
            matches = list(re.finditer(pattern, block, re.S))
            need(len(matches) == 1, 'one mutation family arm: ' + family)
            before = matches[0][0]
            add(prefix + '_' + family.lower(), path, declaration, module_name, target, before, matches[0][1] + 'true')
    block = definition((repo / OBSERVATION).read_text(), 'lifecycle_getter_model_v1', policy)
    for getter in GETTERS:
        matches = list(re.finditer(r'(LifecycleGetterV1::' + getter + r' => )([^\n]+),', block))
        need(len(matches) == 1, 'one getter mutation arm: ' + getter)
        match = matches[0]
        add('getter_' + getter.lower(), OBSERVATION, 'lifecycle_getter_model_v1', 'production',
            'lifecycle_query_correspondence_v1', match[0], match[1] + '(' + match[2] + ') + 1,')
    add('origin_actual', TRACE, 'lifecycle_origin_relation_v1', 'production', 'lifecycle_origin_correspondence_v1',
        '    &&& constructor_producer_relation_v1(origin.actual_before, origin.actual_after, origin.context,\n'
        '        origin.allocations, origin.writers, origin.reads, origin.actual_result)\n', '')
    add('origin_model', TRACE, 'lifecycle_origin_relation_v1', 'production', 'lifecycle_origin_correspondence_v1',
        '    &&& logical::constructor_producer_relation_v1(origin.model_before, origin.model_after, origin.context,\n'
        '        origin.allocations, origin.writers, origin.reads, origin.model_result)\n', '')
    add('register_input', BRIDGE, 'lifecycle_step_inputs_v1', 'production', 'lifecycle_mutation_correspondence_v1',
        '(LifecycleActualStepV1::Register { key, .. }, logical::LifecycleStepV1::Register { key: m, .. }) => writer_key_view(key) == m,',
        '(LifecycleActualStepV1::Register { key, .. }, logical::LifecycleStepV1::Register { key: m, .. }) => true,')
    add('stable_original', BRIDGE, 'lifecycle_step_inputs_v1', 'production', 'lifecycle_mutation_correspondence_v1',
        ' && stable_output_view(original) == o', '')
    add('registration_history', MODEL, 'lifecycle_writer_history_v1', 'owner_lifecycle_preservation', 'lifecycle_step_issued_v1',
        'registration_history_v1(history, result)', 'history')
    for flavor in ('stable', 'producer'):
        add(flavor + '_minted_index', MODEL, 'lifecycle_' + flavor + '_minted_v1', 'owner_lifecycle_preservation',
            'lifecycle_step_histories_v1', 'output[i].unwrap()', 'output[0].unwrap()')
    add('reached_representation', DOMAIN, 'lifecycle_mutation_domain_from_reached_v1', 'production',
        'lifecycle_mutation_domain_from_reached_v1', 'producer_represents(actual, model), ', '')
    add('failed_constructor_grammar', TRACE, 'lifecycle_paired_trace_v1', 'production', 'lifecycle_constructor_origin_trace_v1',
        'Err(_) => actual_states.len() == 0 && events.len() == 0,',
        'Err(_) => actual_states.len() <= 1 && events.len() == 0,')
    need(len(rows) == len({row[0] for row in rows}), 'distinct lifecycle negative names')
    return rows


# Source-policy checks never count as logical negative controls. Some strengthen
# premises or erase conclusions and could still pass the solver vacuously.
POLICY_CASES = [
    ('missing_actual_family', BRIDGE, None,
     '    Unknown { writer: WriterReferenceV1, result: Result<(), ReadErrorV1> },\n', '',
     'closed lifecycle alphabet: LifecycleActualStepV1'),
    ('missing_logical_family', MODEL, None,
     '    Unknown { writer: WriterReferenceV1, result: Result<(), ReadErrorV1> },\n', '',
     'closed lifecycle alphabet: LifecycleStepV1'),
    ('missing_query', OBSERVATION, None, '    ProducerStatus(ContextProducerReadReferenceV1),\n', '',
     'closed lifecycle alphabet: LifecycleQueryV1'),
    ('missing_getter', OBSERVATION, None, '    ContextGeneration, AllocationCapacity,', '    AllocationCapacity,',
     'closed lifecycle alphabet: LifecycleGetterV1'),
    ('missing_receiver', OBSERVATION, None, 'enum LifecycleReceiverV1 { Journal, Stable, Producer }',
     'enum LifecycleReceiverV1 { Journal, Producer }', 'closed lifecycle alphabet: LifecycleReceiverV1'),
    ('matched_answers', BRIDGE, 'lifecycle_step_inputs_v1', '    match (actual, model) {',
     '    lifecycle_step_answers_v1(actual, model) && match (actual, model) {',
     'reviewed lifecycle admission/contract: lifecycle_step_inputs_v1'),
    ('post_representation', TRACE, 'lifecycle_event_relation_v1', '            &&& lifecycle_step_inputs_v1(actual, model)',
     '            &&& producer_represents(after, model_after)\n            &&& lifecycle_step_inputs_v1(actual, model)',
     'raw admission cannot assume invariant or matched answers: lifecycle_event_relation_v1'),
    ('post_invariant', TRACE, 'lifecycle_event_relation_v1', '            &&& lifecycle_step_inputs_v1(actual, model)',
     '            &&& logical::producer_invariant_v1(model_after)\n            &&& lifecycle_step_inputs_v1(actual, model)',
     'raw admission cannot assume invariant or matched answers: lifecycle_event_relation_v1'),
    ('pre_matched_constructor', TRACE, 'lifecycle_origin_relation_v1', '    &&& constructor_observations_match_v1(origin.actual_before, origin.model_before)',
     '    &&& constructor_producer_results_match_v1(origin.actual_result, origin.model_result)\n'
     '    &&& constructor_observations_match_v1(origin.actual_before, origin.model_before)',
     'raw admission cannot assume invariant or matched answers: lifecycle_origin_relation_v1'),
    ('erase_answer_claim', TRACE, 'lifecycle_event_answers_v1', 'actual_answer == model_answer', 'true',
     'reviewed lifecycle admission/contract: lifecycle_event_answers_v1'),
    ('independent_observation', OBSERVATION, 'lifecycle_query_model_v1', '    match query {',
     '    lifecycle_query_actual_v1(owner, query)\n    /* substituted actual computation */ match query {',
     'independent observation computation: lifecycle_query_model_v1'),
    ('failed_grammar_policy', TRACE, 'lifecycle_paired_trace_v1', 'Err(_) => actual_states.len() == 0 && events.len() == 0,',
     'Err(_) => actual_states.len() <= 1 && events.len() == 0,',
     'reviewed lifecycle admission/contract: lifecycle_paired_trace_v1'),
    ('reader_event_prefix', READER_WITNESSES, 'lifecycle_reader_extension_v1',
     'after.events == before.events.push(lifecycle_reader_expected_event_v1(phase))',
     'after.events.len() == before.events.len() + 1',
     'reviewed lifecycle admission/contract: lifecycle_reader_extension_v1'),
    ('reader_actual_prefix', READER_WITNESSES, 'lifecycle_reader_extension_v1',
     'after.actual == before.actual.push(actual)', 'after.actual.len() == before.actual.len() + 1',
     'reviewed lifecycle admission/contract: lifecycle_reader_extension_v1'),
    ('reader_model_prefix', READER_WITNESSES, 'lifecycle_reader_extension_v1',
     'after.model == before.model.push(model)', 'after.model.len() == before.model.len() + 1',
     'reviewed lifecycle admission/contract: lifecycle_reader_extension_v1'),
    ('reader_origin', READER_WITNESSES, 'lifecycle_reader_extension_v1', 'after.origin == before.origin', 'true',
     'reviewed lifecycle admission/contract: lifecycle_reader_extension_v1'),
    ('reader_helper_extension', READER_WITNESSES, 'lifecycle_reader_enroll_v1',
     '        lifecycle_reader_extension_v1(trace, next@, *final(actual), *final(model), 1),\n', '',
     'reviewed lifecycle admission/contract: lifecycle_reader_enroll_v1'),
    ('skip_actual_register', WITNESSES, 'lifecycle_register_fixture_step_v1',
     'let registered = owner_register_historical_exec_v1(', 'let registered = synthetic_register_v1(',
     'reviewed lifecycle admission/contract: lifecycle_register_fixture_step_v1'),
    ('presumed_register_answer', WITNESSES, 'lifecycle_register_fixture_step_v1',
     'result: registered.0', 'result: Ok(WriterReferenceV1 { slot: 0, key })',
     'reviewed lifecycle admission/contract: lifecycle_register_fixture_step_v1'),
]


command = INSPECTION.command
proof_timeout = INSPECTION.proof_timeout


def bootstrap_selftest(repo):
    with tempfile.TemporaryDirectory(prefix='fe2o3-lifecycle-bootstrap-') as temporary:
        stage = Path(temporary)
        for path in HELPER_PINS:
            target = stage / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((repo / path).read_bytes())
        authenticate_helpers(stage)

        def rejected(path, message):
            try:
                authenticate_helper(stage, path)
            except ValueError as error:
                need(str(error) == message + str(path), 'intended helper bootstrap rejection')
            else:
                raise ValueError('accepted changed lifecycle Python helper')

        for path in HELPER_PINS:
            target = stage / path
            original = target.read_bytes()
            target.write_bytes(original + b'\n')
            try:
                rejected(path, 'exact lifecycle Python helper: ')
            finally:
                target.write_bytes(original)
        target = stage / PREVIOUS
        original = target.read_bytes()
        target.unlink()
        target.symlink_to(repo / PREVIOUS)
        try:
            rejected(PREVIOUS, 'no lifecycle Python helper links: ')
        finally:
            target.unlink()
            target.write_bytes(original)
        target.unlink()
        target.mkdir()
        try:
            rejected(PREVIOUS, 'ordinary lifecycle Python helper: ')
        finally:
            target.rmdir()
            target.write_bytes(original)
        (stage / 'crates').rename(stage / 'saved-crates')
        (stage / 'crates').symlink_to(repo / 'crates', target_is_directory=True)
        try:
            rejected(PREVIOUS, 'no lifecycle Python helper links: ')
        finally:
            (stage / 'crates').unlink()
            (stage / 'saved-crates').rename(stage / 'crates')
        authenticate_helpers(stage)

        controls = [
            {'GIT_DIR': str(stage / 'missing'), 'GIT_WORK_TREE': str(stage), 'GIT_COMMON_DIR': str(stage)},
            {'GIT_OBJECT_DIRECTORY': str(stage), 'GIT_ALTERNATE_OBJECT_DIRECTORIES': str(stage / 'missing')},
            {'GIT_CONFIG_COUNT': '1', 'GIT_CONFIG_KEY_0': 'core.repositoryformatversion', 'GIT_CONFIG_VALUE_0': '999'},
            {'PATH': str(stage), 'GIT_CONFIG_GLOBAL': str(stage / 'missing'), 'GIT_CONFIG_SYSTEM': str(stage / 'missing')},
            {'GIT_NO_REPLACE_OBJECTS': '0', 'GIT_REPLACE_REF_BASE': 'refs/unreviewed/'},
        ]
        saved = os.environ.copy()
        try:
            for changes in controls:
                os.environ.update(changes)
                need(git_output(repo, 'show', FROZEN + ':' + str(PREVIOUS)) == original,
                     'ambient Git state cannot substitute frozen sources')
                os.environ.clear()
                os.environ.update(saved)
        finally:
            os.environ.clear()
            os.environ.update(saved)
        fixture = stage / 'git-fixture'
        fixture.mkdir()
        git_output(fixture, 'init', '--quiet')
        (fixture / 'original').write_bytes(b'original frozen blob\n')
        (fixture / 'replacement').write_bytes(b'replacement blob\n')
        old = git_output(fixture, 'hash-object', '-w', 'original', text=True).strip()
        new = git_output(fixture, 'hash-object', '-w', 'replacement', text=True).strip()
        git_output(fixture, 'replace', old, new)
        replacement_env = git_environment()
        del replacement_env['GIT_NO_REPLACE_OBJECTS']
        replaced = subprocess.check_output(['/usr/bin/git', '-C', str(fixture), 'cat-file', 'blob', old],
                                           env=replacement_env)
        need(replaced == b'replacement blob\n', 'effective private Git replacement control')
        need(git_output(fixture, 'cat-file', 'blob', old) == b'original frozen blob\n',
             'authoritative Git reads ignore replacement refs')
    print('PASS: lifecycle bootstrap; 18 helper controls, 5 ambient Git controls, replacement-ref isolation')


def frozen_inspection_selftest(repo, inherited):
    need(len(inherited) == INHERITED_INPUT_COUNT, 'exact frozen recorder source roster')
    need(all(not path.is_absolute() and '..' not in path.parts for path in inherited), 'relative frozen recorder paths')
    git_dir = git_output(repo, 'rev-parse', '--absolute-git-dir', text=True).strip()
    tree = git_output(repo, 'ls-tree', '-rz', FROZEN, '--', *[str(path) for path in sorted(inherited)])
    entries = {}
    for entry in tree.split(b'\0'):
        if not entry:
            continue
        header, name = entry.split(b'\t', 1)
        mode, kind, object_id = header.split()
        path = Path(name.decode())
        need(path not in entries and mode in (b'100644', b'100755') and kind == b'blob',
             'unique ordinary frozen recorder blob')
        entries[path] = (mode, object_id.decode())
    need(set(entries) == inherited, 'exact frozen recorder Git tree')
    with tempfile.TemporaryDirectory(prefix='fe2o3-lifecycle-frozen-recorder-') as temporary:
        base = Path(temporary)
        stage, home, scratch = (base / name for name in ('sources', 'home', 'tmp'))
        for directory in (stage, home, scratch):
            directory.mkdir()
        before = {}
        for path, (mode, object_id) in entries.items():
            data = git_output(repo, 'cat-file', 'blob', object_id)
            target = stage / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            target.chmod(0o755 if mode == b'100755' else 0o644)
            before[path] = data
        expected_checker = git_output(repo, 'show', FROZEN + ':' + str(PREVIOUS))
        need(before[PREVIOUS] == expected_checker, 'exact frozen inspection checker')
        authenticate_helpers(stage)
        env = {**git_environment(), 'HOME': str(home), 'TMPDIR': str(scratch),
               'GIT_DIR': git_dir, 'GIT_WORK_TREE': str(stage)}
        # Stay in the outer recorder's owned group, including every git child.
        child = subprocess.Popen([sys.executable, '-I', '-B', str(stage / PREVIOUS), '--selftest'],
                                 cwd=stage, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                 start_new_session=False)
        try:
            need(os.getpgid(child.pid) == os.getpgrp(), 'frozen recorder stays in outer owned group')
            stdout, stderr = child.communicate(timeout=300)
        except subprocess.TimeoutExpired as error:
            if error.stdout:
                sys.stdout.buffer.write(error.stdout)
            if error.stderr:
                sys.stderr.buffer.write(error.stderr)
            raise
        finally:
            if child.poll() is None:
                child.kill()
            child.wait(timeout=5)
            child.stdout.close()
            child.stderr.close()
        sys.stdout.write(stdout.decode())
        sys.stderr.write(stderr.decode())
        need(child.returncode == 0 and stdout == INSPECTION.recorder_transcript().encode() and not stderr,
             'exact frozen inspection selftest result')
        nodes = list(stage.rglob('*'))
        need(not any(path.is_symlink() for path in nodes), 'no frozen recorder source links')
        need({path.relative_to(stage) for path in nodes if path.is_file()} == inherited,
             'unchanged frozen recorder file roster')
        need(all((stage / path).read_bytes() == data for path, data in before.items()), 'unchanged frozen recorder bytes')
    print('PASS: frozen inspection recorder; exact signed-parent sources and transcript; no process-group escape')


def proof_edit_selftest(repo, deps):
    policy = deps['policy']
    originals = {path: git_output(repo, 'show', FROZEN + ':' + str(path), text=True)
                 for path in INHERITED_ORIGINALS}
    current = {path: (repo / path).read_text() for path in INHERITED_ORIGINALS}
    cases = []
    for path, names in INHERITED_ORIGINALS.items():
        authenticate_proof_edit(path, originals[path], current[path], deps)
        for name in names:
            block = raw_definition(current[path], name, policy)
            if '\n    requires ' in block:
                weakened = INSPECTION.replace_once(block, '\n    requires ', '\n    requires false, ')
            else:
                weakened = INSPECTION.replace_once(block, '\n    ensures ', '\n    requires false,\n    ensures ')
            changed = INSPECTION.replace_once(current[path], block, weakened)
            cases.append((path, changed, 'unchanged inherited proof contract: ' + name))
            changed_body = INSPECTION.replace_once(block, '\n{\n', '\n{\n    assert(true);\n')
            changed = INSPECTION.replace_once(current[path], block, changed_body)
            cases.append((path, changed, 'reviewed inherited proof body: ' + name))
        cases.append((path, current[path] + '\n', 'unchanged inherited proof surroundings: ' + str(path)))
    for name in BEGIN_HELPERS:
        block = raw_definition(current[BEGIN_CUSTODY], name, policy)
        changed_block = INSPECTION.replace_once(block, '\n    requires ', '\n    requires false, ')
        changed = INSPECTION.replace_once(current[BEGIN_CUSTODY], block, changed_block)
        cases.append((BEGIN_CUSTODY, changed, 'reviewed inherited proof body: ' + name))
    need(len(cases) == 15, 'exact inherited proof-edit controls')
    for path, changed, expected_error in cases:
        need(changed != current[path], 'effective inherited proof-edit control')
        saved = INHERITED_PINS[path]
        INHERITED_PINS[path] = deps['scalar'].digest(changed.encode())
        try:
            try:
                authenticate_proof_edit(path, originals[path], changed, deps)
            except ValueError as error:
                need(str(error) == expected_error, 'intended inherited proof-edit rejection: ' + expected_error)
            else:
                raise ValueError('accepted inherited proof edit: ' + expected_error)
        finally:
            INHERITED_PINS[path] = saved
    print('PASS: lifecycle inherited proof edits; 15 rehashed changes rejected; original contracts unchanged')


def recorder_transcript(negative_count):
    return ('PASS: lifecycle bootstrap; 18 helper controls, 5 ambient Git controls, replacement-ref isolation\n'
            + INSPECTION.recorder_transcript()
            + 'PASS: frozen inspection recorder; exact signed-parent sources and transcript; no process-group escape\n'
            + 'PASS: lifecycle inherited proof edits; 15 rehashed changes rejected; original contracts unchanged\n'
            + 'PASS: lifecycle source-policy controls; ' + str(len(POLICY_CASES)) + ' rehashed changes rejected\n'
            + 'PASS: lifecycle inherited/root/checker/roster authentication; 6 changes rejected\n'
            + 'PASS: lifecycle logical mutation anchors; ' + str(negative_count) + ' scoped controls authenticated\n')


def recorder_selftest():
    reviewed_sources_configuration()
    repo = Path(__file__).resolve().parents[3]
    deps = dependencies(repo)
    inherited = inherited_sources(repo, deps['base'])
    authenticate(repo, inherited, deps)
    bootstrap_selftest(repo)
    frozen_inspection_selftest(repo, inherited)
    proof_edit_selftest(repo, deps)
    saved = copy.deepcopy((NEW_PINS, DEFINITION_PINS, CONTRACT_PINS, READER_CONTRACT_PINS))
    try:
        with tempfile.TemporaryDirectory(prefix='fe2o3-lifecycle-source-selftest-') as temporary:
            stage = Path(temporary)
            for path in inherited | set(NEW) | {CHECK}:
                target = stage / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((repo / path).read_bytes())
            authenticate(stage, inherited, deps, git_repo=repo)
            for label, path, name, before, after, message in POLICY_CASES:
                original = (stage / path).read_text()
                block = raw_definition(original, name, deps['policy']) if name is not None else original
                need(block.count(before) == 1, 'policy mutation anchor: ' + label)
                modified = original.replace(block, block.replace(before, after, 1), 1)
                need(modified != original, 'applied policy mutation: ' + label)
                (stage / path).write_text(modified)
                pin = NEW_PINS[path]
                NEW_PINS[path] = deps['scalar'].digest(modified.encode())
                try:
                    need(deps['scalar'].digest((stage / path).read_bytes()) == NEW_PINS[path],
                         'explicit rehashed source-policy mutation: ' + label)
                    try:
                        source_policy(stage, deps)
                    except ValueError as error:
                        need(str(error) == message, 'intended source-policy rejection: ' + label)
                    else:
                        raise ValueError('source policy accepted: ' + label)
                finally:
                    (stage / path).write_text(original)
                    NEW_PINS[path] = pin
            cases = [
                ('inherited', INSPECTION.MODEL, '\n// unexpected inherited change\n', 'unchanged inspection input:'),
                ('document', next(iter(INSPECTION.DOCUMENT_PINS)), '\nUnexpected document change.\n', 'unchanged inspection input:'),
                ('root', ROOT, '\n// unexpected root change\n', 'exact lifecycle paired root extension'),
                ('checker', CHECK, '\n# unexpected checker change\n', 'exact running lifecycle checker'),
            ]
            for label, path, suffix, message in cases:
                original = (stage / path).read_text()
                (stage / path).write_text(original + suffix)
                try:
                    try:
                        authenticate(stage, inherited, deps, git_repo=repo)
                    except ValueError as error:
                        need(message in str(error), 'intended source-auth rejection: ' + label)
                    else:
                        raise ValueError('source authentication accepted: ' + label)
                finally:
                    (stage / path).write_text(original)
            unexpected = stage / CRATE / 'src/lifecycle_unregistered.rs'
            unexpected.write_text('// unregistered runtime source\n')
            try:
                try:
                    source_roster(stage, inherited)
                except ValueError as error:
                    need('closed runtime source roster' in str(error), 'intended runtime roster rejection')
                else:
                    raise ValueError('unregistered runtime source accepted')
            finally:
                unexpected.unlink()
            unexpected = stage / VERUS_DIR / 'context_owner_lifecycle_unregistered_v1.rs'
            unexpected.write_text('// unregistered lifecycle source\n')
            try:
                try:
                    source_roster(stage, inherited)
                except ValueError as error:
                    need(str(error) == 'closed lifecycle source roster', 'intended lifecycle roster rejection')
                else:
                    raise ValueError('unregistered lifecycle source accepted')
            finally:
                unexpected.unlink()
        rows = mutations(repo, deps['policy'])
        for row in rows:
            deps['legacy'].mutated((repo / row[1]).read_bytes(), row)
        print('PASS: lifecycle source-policy controls; ' + str(len(POLICY_CASES)) + ' rehashed changes rejected')
        print('PASS: lifecycle inherited/root/checker/roster authentication; 6 changes rejected')
        print('PASS: lifecycle logical mutation anchors; ' + str(len(rows)) + ' scoped controls authenticated')
    finally:
        for current, previous in zip((NEW_PINS, DEFINITION_PINS, CONTRACT_PINS, READER_CONTRACT_PINS), saved):
            current.clear()
            current.update(previous)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--verus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target', type=Path, required=True)
    parser.add_argument('--probe', action='store_true')
    parser.add_argument('--resume', action='store_true')
    parser.add_argument('--stop-after')
    args = parser.parse_args()
    configuration()
    repo = args.repo.resolve()
    need(Path(__file__).resolve() == repo / CHECK, 'checker belongs to the captured repository')
    os.chdir(repo)
    deps = dependencies(repo)
    base, scalar, legacy = (deps[name] for name in ('base', 'scalar', 'legacy'))
    inherited = inherited_sources(repo, base)
    authenticate(repo, inherited, deps)
    rows = mutations(repo, deps['policy'])
    sources = sorted(inherited | set(NEW))
    inputs = set(sources) | {CHECK}
    need(len(inputs) == SOURCE_COUNT, 'complete lifecycle source roster')
    before = {str(path): scalar.digest((repo / path).read_bytes()) for path in sorted(inputs)}
    commit = git_output(repo, 'rev-parse', 'HEAD', text=True).strip()
    if not args.probe:
        for path, pin in before.items():
            expected = git_output(repo, 'show', commit + ':' + path)
            need(scalar.digest(expected) == pin, 'source commit mismatch: ' + path)
    for path, pin in legacy.PINS.items():
        need(scalar.digest((repo / path).read_bytes()) == pin, 'source pin: ' + str(path))
    output, verus = args.output.resolve(), args.verus.resolve(strict=True)
    need(verus.name == 'verus', 'named pinned Verus executable')
    output.parent.mkdir(parents=True, exist_ok=True)
    lock = scalar.campaign_lock(output)
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    identity = {'commit': commit, 'probe': args.probe, 'inputs': before}
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'CARGO_HOME': '/home/harsh/.cargo', 'RUSTUP_HOME': '/home/harsh/.rustup',
           'VERUS_Z3_PATH': str(verus.parent / 'z3'), 'CARGO_TARGET_DIR': str(args.target.resolve()),
           'TMPDIR': str(output)}
    cases = [('positive-before', None), *[(row[0], row) for row in rows], ('positive-after', None),
             ('raw-regression', None), ('inspection-regression', None), ('constructor-regression', None)]
    cpu = [
        ('recorder-test', ['python3', '-B', str(Path(__file__).resolve()), '--selftest']),
        ('compiler', ['rustc', '+1.97.1', '-Vv']),
        ('format', ['cargo', '+1.97.1', 'fmt', '--check', '-p', 'fe2o3-runtime-model']),
        ('test', ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model']),
        ('clippy', ['cargo', '+1.97.1', 'clippy', '--locked', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings']),
        ('release-build', ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model', '--release', '--no-run']),
    ]
    names = ['closure-before', *[name for name, _ in cases], 'closure-after', *[name for name, _ in cpu]]
    need(args.stop_after is None or args.stop_after in names, 'known checkpoint name')
    closure_command = ['/bin/sh', str(repo / legacy.CLOSURE), str(verus.parent), str(repo / legacy.MANIFEST)]
    cpu_commands, changes = dict(cpu), dict(cases)
    cpu_parser = module(repo, deps['settlement'].CPU_PARSER, 'lifecycle_cpu_parser').test_results
    roots = {'raw-regression': INSPECTION.RAW, 'inspection-regression': INSPECTION.ROOT,
             'constructor-regression': INSPECTION.CONSTRUCTOR.ROOT}
    counts = {'raw-regression': RAW_VERIFIED, 'inspection-regression': INSPECTION_VERIFIED,
              'constructor-regression': CONSTRUCTOR_VERIFIED}

    def validate(name, row, stdout, stderr):
        if name in ('closure-before', 'closure-after'):
            need(row['command'] == closure_command and row['status'] == 0 and not stderr and stdout ==
                 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n',
                 'pinned Verus distribution')
        elif name in cpu_commands:
            need(row['command'] == cpu_commands[name] and row['status'] == 0, 'CPU check: ' + name)
            if name == 'test':
                need(cpu_parser(stdout) == CPU_TEST_RESULTS, 'exact CPU test inventory')
            if name == 'recorder-test':
                need(stdout == recorder_transcript(len(rows)) and not stderr, 'exact lifecycle recorder inventory')
        else:
            root, change = roots.get(name, ROOT), changes[name]
            recorded = Path(row['command'][-1])
            need(recorded.is_absolute() and str(recorded).endswith('/' + str(root)), 'recorded lifecycle proof root')
            stage = Path(str(recorded)[:-len(str(root)) - 1])
            need(stage.parent == output and stage.name.startswith('sources-'), 'owned lifecycle source stage')
            need(row['command'] == command(legacy, verus, stage / root, change), 'exact lifecycle proof command')
            observed = legacy.normalized(base, stdout, stderr, stage)
            scalar.check_verifier(observed['verus'])
            if change:
                check_negative(scalar, name, row['status'], observed)
            else:
                need(row['status'] == 0 and not observed['diagnostics'], 'clean whole-root terminal positive')
                need(legacy.same(observed['result'], {'encountered-error': False, 'encountered-vir-error': False,
                     'success': True, 'verified': counts.get(name, PAIRED_VERIFIED), 'errors': 0,
                     'is-verifying-entire-crate': True}), 'exact whole-root lifecycle/regression inventory')
            return observed

    campaign = scalar.Campaign(output, identity, names, base, legacy.same, validate, args.resume)

    def unchanged():
        source_roster(repo, inherited)
        need(git_output(repo, 'rev-parse', 'HEAD', text=True).strip() == commit,
             'lifecycle source HEAD drift')
        need(before == {str(path): scalar.digest((repo / path).read_bytes()) for path in sorted(inputs)},
             'lifecycle source input drift')

    def stop(name):
        if name != args.stop_after or name == 'release-build':
            return False
        unchanged()
        print('CHECKPOINT: ' + name + ' checked; lifecycle campaign incomplete', flush=True)
        return True

    for phase in ('before', 'after'):
        campaign.run('closure-' + phase, closure_command, 120, env)
        if stop('closure-' + phase):
            return
        if phase == 'after':
            break
        for name, change in cases:
            root = roots.get(name, ROOT)
            if name in campaign.rows:
                print(name, campaign.proofs[name]['result'], '(reused)', flush=True)
            else:
                with tempfile.TemporaryDirectory(prefix='sources-', dir=output) as temporary:
                    stage = Path(temporary)
                    for path in sources:
                        data = (repo / path).read_bytes()
                        need(scalar.digest(data) == before[str(path)], 'staging source drift: ' + str(path))
                        target = stage / path
                        target.parent.mkdir(parents=True, exist_ok=True)
                        target.write_bytes(legacy.mutated(data, change) if change and path == change[1] else data)
                    scan_sources(stage, deps)
                    campaign.run(name, command(legacy, verus, stage / root, change),
                                 proof_timeout(change), env)
                    print(name, campaign.proofs[name]['result'], flush=True)
            if stop(name):
                return
        need(legacy.same(campaign.proofs['positive-before'], campaign.proofs['positive-after']),
             'matching source-bound lifecycle whole-root positives')
    for name, cpu_command in cpu:
        campaign.run(name, cpu_command, 600, env)
        print(name, 'passed', flush=True)
        if stop(name):
            return
    unchanged()
    (output / 'inputs-after.json').write_text(json.dumps(before, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(campaign.proofs, indent=2) + '\n')
    lock.close()


if __name__ == '__main__':
    if sys.argv[1:] == ['--selftest']:
        recorder_selftest()
    else:
        main()
