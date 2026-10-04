// Pure relation controls only. No worker/compiler/native/file input is executed.
#define FE2O3_TRANSPORT_OBSERVER_NO_MAIN
#include "TransportNativeObservation.cpp"
int main() {
  (void)&transportRecord;(void)&transportRun;(void)&helperAnalyze;(void)&compositionRecord;
  constexpr StringLiteral Text=R"({"canonical_identity":"c","canonical_sha256":"b","llvm_sha256":"l",
    "descriptor_sha256":"d","handoff_sha256":"h","semantic_identity":"s","source_profile":"copy",
    "entry":"root","entry_symbol":"root","root_symbol":"root",
    "cpu":{"cases":32,"view_offset":8,"edited_intent":false,"canaries_and_initialization":true,
    "native_or_physical_execution":false,"output_sha256":"o"}})";
  auto V=transportJson(ArrayRef<uint8_t>(reinterpret_cast<const uint8_t*>(Text.data()),Text.size()));
  const auto &N=compositionRow(V);size_t Count=0;
  const auto Check=[&](bool X){require(X,"transport relation control");++Count;};
  Check(transportRoleJoin(N,N));
  for(StringRef K:{"canonical_identity","canonical_sha256","llvm_sha256","descriptor_sha256",
      "handoff_sha256","semantic_identity","source_profile"}) {
    auto C=V;(*C.getAsObject())[K]="foreign";Check(!transportRoleJoin(*C.getAsObject(),N));
    C=V;C.getAsObject()->erase(K);Check(!transportRoleJoin(*C.getAsObject(),N));
  }
  for(StringRef K:{"cases","view_offset","edited_intent","canaries_and_initialization",
      "native_or_physical_execution","output_sha256"}) {
    auto C=V;(*C.getAsObject()->getObject("cpu"))[K]="foreign";
    Check(!transportRoleJoin(*C.getAsObject(),N));
    C=V;C.getAsObject()->getObject("cpu")->erase(K);Check(!transportRoleJoin(*C.getAsObject(),N));
  }
  for(StringRef K:{"entry_symbol","root_symbol"}) {
    auto C=V;(*C.getAsObject())[K]="foreign";Check(!transportRoleJoin(*C.getAsObject(),N));
  }
  auto Empty=V;Empty.getAsObject()->erase("cpu");Check(!transportRoleJoin(*Empty.getAsObject(),N));
  json::Object S{{"invocation","actual"},{"source","same"}};
  Check(transportSessionJoin(S,S));
  for(StringRef K:{"invocation","source"}) {
    json::Object C{{"invocation","actual"},{"source","same"}};
    C[K]="foreign";Check(!transportSessionJoin(C,S));C.erase(K);Check(!transportSessionJoin(C,S));
  }
  require(Count==35,"transport source relation control census");
  outs()<<"35 pure transport source relation controls\n";return 0;
}
