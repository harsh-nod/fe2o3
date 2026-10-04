// Pure bounded report syntax/identity checks, not source or runtime authority.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {open,constants} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';

export const mutationRoster=Object.freeze([
  ...Array.from({length:16},(_,i)=>[`row_${i}_removed`,`row_${i}_operand`]).flat(),
  'load_wait_lgkm_pending','store_wait_vm_pending','trap_abort_id2','trap_breakpoint_id1',
  'extra_early_trap','second_trap_at_end','pointer_kernarg_offset','sentinel_s12_literal',
  'sentinel_s13_literal','scale_64bit_immediate','store_data_register','store_offset_nonzero',
  'prefetch_padding_changed',
  ...['group_segment','private_segment','kernarg_total','reserved12','entry_offset','reserved24',
    'reserved40','rsrc3_extra','vgpr_short','sgpr_short','privilege','debug_mode','workgroup_x_missing',
    'user_sgpr_extra','workitem_y','scratch','trap_enable_descriptor','kernarg_pointer_missing',
    'kernarg_preload','reserved60'].map(v=>'descriptor_'+v),
  ...['wave','sgpr','vgpr','agpr','sgpr_spill','vgpr_spill','kernarg_total','kernarg_align',
    'workgroup_cap','grid_cap','output_offset','hidden_offset','hidden_width'].map(v=>'metadata_'+v),
  'elf_target','elf_machine','elf_cov','elf_truncated','artifact_one_over'
]);
export function checkRoster(rows) {
  assert(Array.isArray(rows)); assert.equal(rows.length,83);
  assert.deepEqual(rows.map(v=>v.mutation),mutationRoster);
  for(const row of rows) {
    assert.deepEqual(Object.keys(row).sort(),['accepted','bytes','mutation','reason','sha256']);
    assert.equal(row.accepted,false);
    assert.equal(typeof row.reason,'string'); assert(row.reason.length>0 && row.reason.length<=4096);
    assert.match(row.sha256,/^[0-9a-f]{64}$/);
    assert(Number.isSafeInteger(row.bytes) && row.bytes>0 && row.bytes<=1048577);
  }
}
export function validateReport(v) {
  assert.equal(v?.schema,'diagnostic-gfx950-one-stop-fixture-v1');
  assert(['O0','O3'].includes(v.optimization)); assert.equal(v.target,'gfx950:xnack-');
  assert.equal(v.wave_width,64); assert.equal(v.code_object_version,6);
  assert.equal(v.kernel_name,'fe2o3_gfx950_one_stop_fixture');
  assert.deepEqual(v.workgroup,[64,1,1]); assert.deepEqual(v.grid,[64,1,1]);
  for(const k of ['llvm','artifact','generated_object']) {
    assert(Number.isSafeInteger(v[k+'_bytes']) && v[k+'_bytes']>0 &&
      v[k+'_bytes']<=(k==='llvm'?16384:1048576));
    assert.match(v[k+'_sha256'],/^[0-9a-f]{64}$/);
  }
  assert.match(v.derivation_sha256,/^[0-9a-f]{64}$/);
  assert.equal(v.entry.bytes,84); assert.equal(v.entry.instructions,16);
  assert.equal(v.executable_section.bytes,1152);
  assert.equal(v.executable_section.prefetch_padding_bytes,1068);
  assert.equal(v.executable_section.prefetch_padding_word,'bf800000');
  assert.equal(v.descriptor.bytes,64); assert.equal(v.descriptor.rsrc2,132);
  assert.match(v.descriptor.sha256,/^[0-9a-f]{64}$/);
  assert.equal(v.metadata.kernarg_bytes,264); assert.equal(v.metadata.kernarg_alignment,8);
  for(const k of ['group_bytes','private_bytes','agprs','sgpr_spills','vgpr_spills'])
    assert.equal(v.metadata[k],0);
  const args=v.metadata.arguments;
  assert.equal(args.length,14);
  assert.deepEqual(args.map(a=>a.offset),[0,8,12,16,20,22,24,26,28,30,48,56,64,72]);
  assert.deepEqual(args.map(a=>a.size),[8,4,4,4,2,2,2,2,2,2,8,8,8,2]);
  assert.equal(args[0].name,'out'); assert.equal(args[0].kind,'global_buffer');
  assert.equal(args[0].address_space,'global');
  assert.deepEqual(args.slice(1).map(a=>a.kind),[
    'hidden_block_count_x','hidden_block_count_y','hidden_block_count_z',
    'hidden_group_size_x','hidden_group_size_y','hidden_group_size_z',
    'hidden_remainder_x','hidden_remainder_y','hidden_remainder_z',
    'hidden_global_offset_x','hidden_global_offset_y','hidden_global_offset_z','hidden_grid_dims']);
  assert.deepEqual(v.trap,{id:3,entry_offset:76,posttrap_entry_offset:80,
    future_query_pc_source_hypothesis_offset:80,observed_wave_info_pc:null,native_expected_pc_authorized:false});
  assert.equal(v.trace.length,16);
  assert.deepEqual(v.trace.map(t=>t.offset),[0,8,12,20,28,32,36,44,48,52,56,60,64,72,76,80]);
  assert.deepEqual(v.trace.map(t=>t.bytes),[8,4,8,8,4,4,8,4,4,4,4,4,8,4,4,4]);
  for(const t of v.trace) assert.match(t.encoding,new RegExp('^[0-9a-f]{'+(t.bytes*2)+'}$'));
  assert.equal(v.trace[14].opcode,'S_TRAP_vi'); assert.equal(v.trace[14].encoding,'030092bf');
  assert.equal(v.trace[15].opcode,'S_ENDPGM_vi'); assert.equal(v.trace[15].encoding,'000081bf');
  const out=v.future_output_contract;
  assert.equal(out.backing_bytes,4096); assert.equal(out.logical_bytes,272);
  assert.equal(out.payload_offset,8); assert.equal(out.payload_bytes,256);
  assert.equal(out.leading_canary_hex,'0123456789abcdef'); assert.equal(out.trailing_canary_hex,'fedcba9876543210');
  assert.deepEqual(out.expected_u32_words,Array.from({length:64},(_,i)=>(0x13579bdf^i)>>>0));
  assert.equal(out.allocated_or_observed,false);
  assert.deepEqual(v.input_refusals,[{mutation:'missing_launch',accepted:false},{mutation:'wrong_cpu',accepted:false}]);
  assert.equal(v.synthetic_worker_identity_fields,true);
  for(const k of ['source_authentication','protected_publication','runtime_authority','gpu_execution',
    'stopped_wave_observed','milestone_complete']) assert.equal(v[k],false);
  checkRoster(v.mutations);
}
async function readBounded(path,cap) {
  const h=await open(path,constants.O_RDONLY|constants.O_NOFOLLOW);
  try {
    const s=await h.stat({bigint:true});
    assert(s.isFile() && s.nlink===1n && s.size>0n && s.size<=BigInt(cap));
    const b=Buffer.alloc(Number(s.size)); const n=await h.read(b,0,b.length,0);
    assert.equal(n.bytesRead,b.length); const after=await h.stat({bigint:true});
    for(const key of ['size','mtimeNs','ctimeNs','dev','ino','nlink']) assert.equal(after[key],s[key]);
    return b;
  } finally { await h.close(); }
}
export async function checkDirectory(directory) {
  assert.equal(typeof directory,'string'); assert(directory.startsWith('/') && directory.length<=4000);
  const report=await readBounded(directory+'/report.json',262144);
  const v=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(report)); validateReport(v);
  for(const [name,key,cap] of [['input.ll','llvm',16384],['output.hsaco','artifact',1048576]]) {
    const b=await readBounded(directory+'/'+name,cap);
    assert.equal(b.length,v[key+'_bytes']);
    assert.equal(createHash('sha256').update(b).digest('hex'),v[key+'_sha256']);
  }
  return {schema:'diagnostic-gfx950-one-stop-report-check-v1',syntax_and_retained_file_hashes:true,
    mutation_count:v.mutations.length,completed_root_receipt_checked:false,
    source_authority:false,runtime_authority:false};
}
if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  assert.equal(process.argv.length,3,'usage: node check-report.mjs ABS_OUTPUT_DIRECTORY');
  console.log(JSON.stringify(await checkDirectory(process.argv[2])));
}
