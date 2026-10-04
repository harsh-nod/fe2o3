void writeFresh(StringRef Path,ArrayRef<uint8_t> Bytes,size_t Cap) {
  require(Path.starts_with("/") && Path.size()<=4096 && !Bytes.empty() && Bytes.size()<=Cap &&
      llvm::none_of(Path,[](char C){return static_cast<unsigned char>(C)<32 || C==127;}),"bounded fresh output");
  int FD=open(Path.str().c_str(),O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC|O_NOFOLLOW,0600);
  require(FD>=0,"exclusive output open");
  const ssize_t Written=write(FD,Bytes.data(),Bytes.size()); struct stat S{};
  const bool Good=Written==static_cast<ssize_t>(Bytes.size()) && fstat(FD,&S)==0 &&
      S_ISREG(S.st_mode) && S.st_nlink==1 && S.st_size==static_cast<off_t>(Bytes.size());
  const int Closed=close(FD);
  require(Good && Closed==0,"finite output write failed; partial is not qualified");
}
int run(int Count,char **Arguments) {
  const auto Start=std::chrono::steady_clock::now(); alarm(90);
  auto Timely=[&] { return std::chrono::steady_clock::now()-Start<std::chrono::seconds(90); };
  require(Count==3,"usage: gfx950-one-stop-fixture O0|O3 ABS_FRESH_DIRECTORY");
  const StringRef LevelName(Arguments[1]),Directory(Arguments[2]);
  require(LevelName=="O0" || LevelName=="O3","closed optimization");
  require(Directory.starts_with("/") && Directory.size()<=4000 &&
      llvm::none_of(Directory,[](char C){return static_cast<unsigned char>(C)<32 || C==127;}),
      "absolute bounded output directory");
  require(mkdir(Directory.str().c_str(),0700)==0,"fresh output directory");
  initialize(); Mc M;
  const auto Level=LevelName=="O0"?OptimizationLevel::O0:OptimizationLevel::O3;
  const auto InputValue=fixedInput(); const auto R=requestFor(InputValue,Level);
  writeFresh(Directory.str()+"/input.ll",InputValue.Bytes,16384);
  auto Inputs=inputControls(Level); require(Timely(),"input work deadline");
  const auto Reply=execute(R);
  if (!Reply.LinkedOutput) {
    size_t Remaining=4096;
    for (const auto &D:Reply.Diagnostics) { auto P=StringRef(D).take_front(Remaining); errs()<<P<<'\n'; Remaining-=P.size(); if (!Remaining) break; }
    fail("ordinary worker refused fixed fixture");
  }
  const auto &OutputValue=*Reply.LinkedOutput;
  require(Timely() && Reply.FailureStage==Stage::Complete && Reply.Protocol==R.Protocol &&
      Reply.RequestId==R.RequestId && Reply.RequestIdentity==R.Identity &&
      Reply.CompilerEnvelopeIdentity==R.CompilerEnvelopeIdentity &&
      Reply.WorkerBuildIdentity==R.WorkerBuildIdentity && Reply.Derivation &&
      !Reply.DeviceLibraryProvider && !OutputValue.Bytes.empty() &&
      OutputValue.Bytes.size()<=ArtifactCap && OutputValue.Digest==SHA256::hash(OutputValue.Bytes),
      "ordinary response identity/deadline");
  const auto &D=*Reply.Derivation; auto Id=calculateDerivationEvidenceIdentity(D);
  require(static_cast<bool>(Id),"derivation identity calculation");
  require(D.EvidenceIdentity==*Id && D.Hsaco.Digest==OutputValue.Digest &&
      D.Hsaco.ByteLength==OutputValue.Bytes.size() && D.GeneratedObject.ByteLength<=ArtifactCap &&
      D.NativeLinkInputs.size()==1 && D.NativeLinkInputs[0].Source==NativeLinkInputSource::GeneratedObject &&
      D.NativeLinkInputs[0].Content==D.GeneratedObject,"ordinary generated-object/native-link relation");
  // Retain the exact identity-joined ordinary worker artifact before inspection.
  // The file alone is unqualified; checker refusal must leave it for diagnosis.
  writeFresh(Directory.str()+"/output.hsaco",OutputValue.Bytes,ArtifactCap);
  auto A=inspect(OutputValue.Bytes,R,M);
  if (!A) fail(toString(A.takeError()));
  auto Mutations=controls(OutputValue,*A,R,M); require(Timely(),"inspection deadline");
  json::Array ExpectedPayload;
  for (uint32_t Lane=0;Lane<64;++Lane) ExpectedPayload.emplace_back(uint64_t{0x13579bdfU^Lane});
  json::Object Report{
    {"schema","diagnostic-gfx950-one-stop-fixture-v1"},{"optimization",LevelName.str()},
    {"target","gfx950:xnack-"},{"wave_width",64},{"code_object_version",6},
    {"kernel_name",Name.str()},{"workgroup",json::Array{64,1,1}},{"grid",json::Array{64,1,1}},
    {"llvm_bytes",InputValue.Bytes.size()},{"llvm_sha256",hex(InputValue.Digest)},
    {"artifact_bytes",OutputValue.Bytes.size()},{"artifact_sha256",hex(OutputValue.Digest)},
    {"generated_object_bytes",D.GeneratedObject.ByteLength},{"generated_object_sha256",hex(D.GeneratedObject.Digest)},
    {"derivation_sha256",hex(D.EvidenceIdentity)},
    {"entry",json::Object{{"file_offset",A->EntryOffset},{"elf_va",A->EntryAddress},{"bytes",84},{"instructions",16}}},
    {"executable_section",json::Object{{"file_offset",A->TextOffset},{"bytes",A->TextBytes},
      {"prefetch_padding_bytes",1068},{"prefetch_padding_word","bf800000"}}},
    {"descriptor",json::Object{{"file_offset",A->DescriptorOffset},{"elf_va",A->DescriptorAddress},
      {"bytes",64},{"sha256",hex(A->DescriptorDigest)},{"rsrc1",A->Rsrc1},{"rsrc2",A->Rsrc2},
      {"rsrc3",A->Rsrc3},{"vgpr_capacity",A->Vgprs},{"sgpr_capacity",A->Sgprs},
      {"architectural_vgpr_boundary",A->Boundary},{"fp_mode_fields",(A->Rsrc1>>12)&255},
      {"dx10_clamp",bool(A->Rsrc1&(1U<<21))},{"ieee_mode",bool(A->Rsrc1&(1U<<23))}}},
    {"metadata",json::Object{{"kernarg_bytes",264},{"kernarg_alignment",8},{"group_bytes",0},
      {"private_bytes",0},{"sgprs",A->Metadata.Sgprs},{"vgprs",A->Metadata.Vgprs},
      {"agprs",A->Metadata.Agprs},{"sgpr_spills",A->Metadata.SgprSpills},{"vgpr_spills",A->Metadata.VgprSpills},
      {"arguments",fixtureArgumentsJson(A->Metadata)}}},
    {"trap",json::Object{{"id",3},{"entry_offset",76},{"posttrap_entry_offset",80},
      {"future_query_pc_source_hypothesis_offset",80},{"observed_wave_info_pc",nullptr},
      {"native_expected_pc_authorized",false}}},
    {"future_output_contract",json::Object{{"backing_bytes",4096},{"logical_bytes",272},
      {"payload_offset",8},{"payload_bytes",256},{"leading_canary_hex","0123456789abcdef"},
      {"trailing_canary_hex","fedcba9876543210"},{"expected_u32_words",std::move(ExpectedPayload)},
      {"allocated_or_observed",false}}},
    {"trace",traceJson(*A)},{"input_refusals",std::move(Inputs)},{"mutations",std::move(Mutations)},
    {"synthetic_worker_identity_fields",true},{"source_authentication",false},
    {"protected_publication",false},{"runtime_authority",false},{"gpu_execution",false},
    {"stopped_wave_observed",false},{"milestone_complete",false}};
  std::string Text; raw_string_ostream Stream(Text); Stream<<json::Value(std::move(Report))<<'\n'; Stream.flush();
  require(Text.size()<=ReportCap && Timely(),"final report cap/deadline");
  writeFresh(Directory.str()+"/report.json",arrayRefFromStringRef(Text),ReportCap);
  // A positive report is usable only with process exit0 and completed root receipt.
  require(Timely(),"final report write deadline");
  outs()<<Text; outs().flush(); require(!outs().has_error() && Timely(),"stdout completion/deadline");
  alarm(0); return 0;
}
