// No source authority or new worker route. Same default observer plus typed DATA evidence.
#define PROMOTED_COMPOSITION_NO_OBSERVER_MAIN
#include "promoted-composition-native/PromotedNativeObservation.cpp"
#include "ordered-composition-transport/TransportTestV1.h"
namespace {
#include "TransportRecordV1.inc"
#include "TransportRunV1.inc"
}
#ifndef FE2O3_TRANSPORT_OBSERVER_NO_MAIN
int main(int argc,char **argv) {
  (void)&helperAnalyze;(void)&compositionRecord;
  const auto Started=std::chrono::steady_clock::now();
  require(argc==7,"usage: transport observer ABS_SOURCE_REPORT SHA ABS_REPOSITORY copy|preserve|edit O0|O3 ABS_FRESH_OUTPUT");
  auto T=transportRecord(argv[1],argv[2],argv[3],argv[4],argv[5]);
  const StringRef Out(argv[6]);const auto Slash=Out.rfind('/');
  require(Slash!=StringRef::npos && Out.substr(Slash+1)==std::string(argv[4])+"-"+argv[5],
    "fixed case output leaf");
  const auto Parent=Out.substr(0,Slash);
  constexpr StringLiteral Prefix="/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/phase28-composition-transport-native-";
  require(Parent.starts_with(Prefix) && !Parent.drop_front(Prefix.size()).contains('/') &&
    !Parent.drop_front(Prefix.size()).empty(),"private transport output parent");
  char Canonical[PATH_MAX];
  require(realpath(Parent.str().c_str(),Canonical)&&Parent==Canonical,"prevalidated output parent");
  transportTimely(Started);
  return transportRun(T,argv[5],argv[6],Started);
}
#endif
