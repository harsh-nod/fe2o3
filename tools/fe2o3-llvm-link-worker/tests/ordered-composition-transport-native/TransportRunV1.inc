// New diagnostic consumer, retaining the old worker/inspection report unchanged.
PhysicalMachineEffectRequest transportRequest(const CompositionRecord &R,ArrayRef<uint8_t> B) {
  const auto Id=physicalMachineEffectIdentities();PhysicalMachineEffectRequest Q;
  Q.AnalyzerIdentity=Id.Analyzer;Q.ToolchainIdentity=Id.Toolchain;
  Q.ExecutionChallenge.fill(0x70);Q.RequestIdentity.fill(0x71);Q.RequestBytes=4096;
  Q.Payload.assign(B.begin(),B.end());Q.PayloadDigest=SHA256::hash(B);Q.PayloadBytes=B.size();
  Q.Entries={{R.Input.Entry,{32,16,8,8,8}}};return Q;
}
void transportTimely(std::chrono::steady_clock::time_point Started) {
  require(std::chrono::steady_clock::now()-Started<std::chrono::seconds(110),
    "transport original-start deadline");
}
json::Object transportSummary(const transport::Summary &S) {
  return json::Object{
    {"profile",S.Selected==transport::Profile::ConditionalHelperO0?
      "conditional-ordinary-C-helper-data":"inline-root-kernarg-to-store-data"},
    {"program",S.SelectedProgram==transport::Program::XorAnd?"xor-and":"move-input2"},
    {"cached_registers",S.CachedRegisters},{"whole_words",S.WholeWords},{"whole32_classes",S.Whole32Classes},
    {"functions",S.Functions},{"blocks",S.Blocks},{"instructions",S.Instructions},
    {"selected_offset",S.SelectedOffset},{"selected_bytes",S.SelectedBytes},
    {"program_offset",S.ProgramOffset},{"data_use_offset",S.DataUseOffset},
    {"canonical_identity",hex(S.Canonical)},{"llvm_sha256",hex(S.Llvm)},
    {"source_report_sha256",hex(S.SourceReport)},{"source_descriptor_sha256",hex(S.Descriptor)},
    {"native_descriptor_sha256",hex(S.NativeDescriptor)}};
}
int transportRun(TransportInputV1 &T,StringRef Level,StringRef Output,
                 std::chrono::steady_clock::time_point Started) {
  auto &R=T.Normal.Record;
  require(compositionRun(R,Level,Output,Started,
    "fe2o3-promoted-composition-native-observation-v1",[&]{
      promotedRecheck(T.Normal);helperRecheck(T.Extra);})==0,"ordinary observation completion");
  transportTimely(Started);
  const auto BaselineBytes=helperRead(Output.str()+"/observation.json",2*1024*1024);
  const auto Baseline=transportJson(BaselineBytes);const auto &Old=compositionRow(Baseline);
  helperBool(Old,"ordinary_worker_completed",true);helperBool(Old,"unchanged_normal_llvm",true);
  helperBool(Old,"authored_interval_observation_complete",true);
  const auto B=helperRead(Output.str()+"/output.hsaco",1024*1024);
  require(B.size()==helperInteger(Old,"hsaco_bytes") &&
    hex(SHA256::hash(B))==helperString(Old,"hsaco_sha256"),"same retained ordinary worker artifact");
  T.Projection.Root=R.Input.Entry;T.Projection.Helper=R.Input.Helpers.at(0);
  auto Q=transportRequest(R,B);
  auto Default=unwrap(analyzeGfx942PhysicalMachineEffects(Q));transportTimely(Started);
  auto DefaultWire=unwrap(encodePhysicalMachineAnalysisBundle(Default));
  require(DefaultWire.size()<=1024*1024,"bounded diagnostic effect/trace bundle");
  constexpr uint64_t Floor=37,PriorWork=11;
  transport::Account A(Floor+transport::StorageBytes,PriorWork+transport::WorkUnits,Floor,PriorWork);
  std::optional<transport::Result> Typed;
  Typed.emplace(unwrap(transport::analyze(Q,T.Projection,A)));transportTimely(Started);
    require(A.storage()==Floor+transport::StorageBytes && A.work()==PriorWork+transport::WorkUnits &&
      A.peak()==Floor+transport::StorageBytes && A.denials()==0,"same prepaid result lifetime");
  auto TypedWire=unwrap(encodePhysicalMachineAnalysisBundle(Typed->effects()));
    require(TypedWire==DefaultWire,"default and typed effect/trace bundles must remain byte identical");
    require(Typed->summary().NativeDescriptor==Default.Entries.at(0).DescriptorIdentity,
      "actual descriptor remains joined");
  auto Summary=transportSummary(Typed->summary());
  // Keep the reservation alive through serialization and every output write.
  // Real late aggregate refusal: selected graph analysis runs before the
  // original reachable-effect totals are checked; denying every global write
  // must never publish the pending typed summary.
  auto Late=Q;Late.Entries.at(0).Budget.GlobalWrites=0;
  transport::Account L(Floor+transport::StorageBytes,PriorWork+transport::WorkUnits,Floor,PriorWork);
  auto Rejected=transport::analyze(Late,T.Projection,L);
  require(!Rejected,"late original effect budget bypass");
  const auto LateReason=toString(Rejected.takeError());
  require(LateReason=="gfx942 physical machine-effect: effect expansion exceeds request for "+R.Input.Entry,
    "exact original late aggregate-budget refusal");
  require(LateReason.size()<=8192 && L.storage()==Floor &&
    L.work()==PriorWork+transport::WorkUnits,"late failure preserves floor and debit");
  promotedRecheck(T.Normal);helperRecheck(T.Extra);transportTimely(Started);
  // Separate fixed diagnostic output envelope: two encoded bundles <=1 MiB
  // each and JSON <=64 KiB; existing parser/worker resources remain their own
  // bounded domain. This is not added to source-ledger or whole-process RSS.
  json::Object Report{{"schema","fe2o3-test-composition-transport-native-v1"},
    {"variant",R.Input.Label},{"optimization",Level.str()},
    {"source_report_sha256",T.ReportHash},{"baseline_report_sha256",hex(SHA256::hash(BaselineBytes))},
    {"hsaco_sha256",hex(SHA256::hash(B))},{"hsaco_bytes",B.size()},
    {"llvm_sha256",hex(R.Input.Llvm.Digest)},{"summary",std::move(Summary)},
    {"ordinary_worker_completed",true},{"unchanged_normal_llvm",true},
    {"default_typed_effect_trace_byte_identical",true},{"effect_trace_bytes",DefaultWire.size()},
    {"effect_trace_sha256",hex(SHA256::hash(DefaultWire))},{"late_aggregate_refused",true},
    {"late_refusal",LateReason},
    {"transport_storage_reserved",transport::StorageBytes},{"transport_work_debited",transport::WorkUnits},
    {"incoming_storage",Floor},{"storage_at_serialization",A.storage()},{"peak_storage",A.peak()},
    {"incoming_work",PriorWork},{"work_at_serialization",A.work()},
    {"synthetic_worker_request_identity",true},{"source_custody_from_files",false},
    {"full_O0_caller_transport_proved",false},{"address_or_predicate_proved",false},
    {"runtime_conditions_discharged",false},{"hardware_observed",false},{"protected_authority",false},
    {"full_machine_abi_equivalence_proved",false},{"functional_equivalence_proved",false},
    {"milestone_completion",false}};
  std::string Encoded;raw_string_ostream Stream(Encoded);
  Stream<<formatv("{0:2}",json::Value(std::move(Report)))<<"\n";Stream.flush();
  require(Encoded.size()<=65536,"transport diagnostic output envelope");transportTimely(Started);
  // Positive filename is not published in the child. The root runner can only
  // accept this pending observation after successful timely exit and rechecks.
  const int FD=open(Output.str().c_str(),O_RDONLY|O_DIRECTORY|O_CLOEXEC|O_NOFOLLOW);
  require(FD>=0,"existing output directory");
  helperWriteAt(FD,"transport.pending.json",ArrayRef<uint8_t>(
    reinterpret_cast<const uint8_t*>(Encoded.data()),Encoded.size()));
  helperWriteAt(FD,"default-analysis.bin",DefaultWire);
  helperWriteAt(FD,"typed-analysis.bin",TypedWire);
  Typed.reset();
  require(A.storage()==Floor && A.work()==PriorWork+transport::WorkUnits,
    "result storage floor restored once after output consumption");
  auto Again=transport::analyze(Q,T.Projection,A);
  require(!Again,"cumulative work reset");consumeError(Again.takeError());
  require(A.storage()==Floor && A.work()==PriorWork+transport::WorkUnits && A.denials()==1 &&
    A.last_denial()==transport::Refusal::ResourceWork,"cumulative denial state");
  static constexpr char Terminal[]=
    "{\"schema\":\"fe2o3-test-composition-transport-teardown-v1\","
    "\"final_storage\":37,\"final_work\":280587,\"denials\":1,"
    "\"cumulative_work_refused\":true,\"result_released_after_output\":true}\n";
  helperWriteAt(FD,"transport-terminal.json",ArrayRef<uint8_t>(
    reinterpret_cast<const uint8_t*>(Terminal),sizeof(Terminal)-1));
  require(fsync(FD)==0 && close(FD)==0,"transport output close");
  promotedRecheck(T.Normal);helperRecheck(T.Extra);
  transportTimely(Started);
  outs()<<"{\"transport_pending\":true,\"schema\":\"fe2o3-test-composition-transport-native-v1\"}\n";
  outs().flush();require(!outs().has_error(),"transport stdout failure");
  transportTimely(Started);return 0;
}
