// Independent native V5/KIR V10 census join; never repacks the workgroup export as V6.
import { demand,same,digest,uint,site,validateWorkgroupAttribution,LIMITS }
  from './debug-runtime-observations-source-v1-data.mjs';
export function validateNativeWorkgroupInspection(report,inspection,fileIdentity,source) {
  demand(inspection?.schema==='task-runtime-workgroup-native-v5-inspection-v1'&&inspection.status==='passed'
    &&inspection.bundle_version===5&&inspection.canonical_kir_version===10,'native V5 inspection mode');
  demand(inspection.source_authenticated===false&&inspection.hardware_observed===false
    &&inspection.compiler_resume_authority===false,'native inspection authority');
  for(const field of ['bundle_sha256','bundle_identity','canonical_kir_digest','canonical_kir_sha256']) {
    digest(inspection[field]);demand(inspection[field]===report[field],'native exact artifact join: '+field);
  }
  for(const field of ['source_map_identity','source_map_sha256','bundle_subject_identity'])digest(inspection[field]);
  demand(inspection.canonical_kir_bytes===report.canonical_kir_bytes,'native exact canonical length');
  uint(inspection.operation_count,LIMITS.operations,'native operation count');
  demand(inspection.operation_count===report.topology.operations&&Array.isArray(inspection.operations)
    &&inspection.operations.length===inspection.operation_count,'complete native operation census');
  validateWorkgroupAttribution(report,inspection,inspection.operations);
  demand(Array.isArray(inspection.files)&&inspection.files.length>0&&inspection.files.length<=32,'native source file roster');
  const files=new Map();
  for(const file of inspection.files) {
    digest(file.identity);uint(file.bytes,LIMITS.bundle,'source file byte extent');
    demand(file.bytes>0&&!files.has(file.identity)&&typeof file.display_path==='string'
      &&file.display_path.length>0&&file.display_path.length<=4096,'unique bounded native source file');
    files.set(file.identity,file);
  }
  demand(files.get(fileIdentity)?.bytes===source.length,'same-run census file identity and exact source bytes');
  const selected=report.topology.lds.authoring_coordinate.join(':');
  let lds=null;
  for(const operation of inspection.operations) {
    site(operation.runtime_site);
    demand(operation.runtime_site[0]===operation.coordinate.function
      &&operation.runtime_site[2]===operation.coordinate.operation,'native coordinate domains');
    demand(typeof operation.function_name==='string'&&operation.function_name.length>0
      &&operation.function_name.length<=4096,'native actual function name');
    const key=[operation.coordinate.function,operation.coordinate.block,operation.coordinate.operation].join(':');
    if(key===selected)lds=operation;
    demand(Array.isArray(operation.source_spans)&&operation.source_spans.length<=16,'native source span roster');
    for(const span of operation.source_spans) {
      digest(span.file_identity);const file=files.get(span.file_identity);demand(file,'native span file exists');
      for(const key of ['byte_start','byte_end'])demand(typeof span[key]==='string'&&/^(0|[1-9][0-9]{0,8})$/.test(span[key]),'native span decimal');
      const start=Number(span.byte_start),end=Number(span.byte_end);
      demand(start<=end&&end<=file.bytes,'native span bounds');
      if(span.file_identity===fileIdentity)for(const offset of [start,end])
        demand(offset===source.length||(source[offset]&0xc0)!==0x80,'native main source UTF8 boundary');
    }
  }
  demand(lds&&lds.kind==='workgroup_memory','native source LDS declaration');
  same(lds.runtime_site,report.topology.lds.runtime_site,'actual source LDS runtime BlockId');
  demand(lds.source_spans.some(span=>span.file_identity===fileIdentity),'actual LDS source span joins same-run source census');
  return {bundle_version:5,canonical_kir_version:10,operation_count:inspection.operation_count,
    source_file_identity:fileIdentity,source_map_identity:inspection.source_map_identity,
    lds_runtime_site:lds.runtime_site,lds_authoring_coordinate:report.topology.lds.authoring_coordinate,
    source_authenticated:false,hardware_observed:false,compiler_resume_authority:false};
}
