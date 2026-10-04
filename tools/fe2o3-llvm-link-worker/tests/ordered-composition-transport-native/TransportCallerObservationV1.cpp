// Separate explicit profile/domain. The legacy six-case observer is unchanged.
#define FE2O3_TRANSPORT_OBSERVER_NO_MAIN
#include "TransportNativeObservation.cpp"
int main(int argc,char **argv) {
  (void)&helperAnalyze;(void)&compositionRecord;
  const auto Started=std::chrono::steady_clock::now();
  require(argc==7,"usage: caller observer ABS_SOURCE_REPORT SHA ABS_REPOSITORY copy|preserve|edit O0 ABS_FRESH_OUTPUT");
  require(StringRef(argv[5])=="O0","finite caller profile is O0 only");
  auto T=transportRecord(argv[1],argv[2],argv[3],argv[4],argv[5]);
  require(T.Projection.Selected==transport::Profile::ConditionalHelperO0,
    "exact existing O0 source projection");
  T.Projection.Selected=transport::Profile::CallerToHelperO0;
  const StringRef Out(argv[6]);const auto Slash=Out.rfind('/');
  require(Slash!=StringRef::npos && Out.substr(Slash+1)==std::string(argv[4])+"-O0",
    "fixed caller case output leaf");
  const auto Parent=Out.substr(0,Slash);
  constexpr StringLiteral Prefix="/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/phase28-composition-caller-transport-native-";
  require(Parent.starts_with(Prefix) && !Parent.drop_front(Prefix.size()).contains('/') &&
    !Parent.drop_front(Prefix.size()).empty(),"private caller output parent");
  char Canonical[PATH_MAX];
  require(realpath(Parent.str().c_str(),Canonical) && Parent==Canonical,
    "prevalidated caller output parent");
  transportTimely(Started);
  return transportRun(T,argv[5],argv[6],Started);
}
