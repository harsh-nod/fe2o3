// Included only inside the analyzer's private anonymous namespace and test build.
namespace ct = transport_test_v1;
// Const references to the actual tables/analysis, not const unique_ptr wrappers.
struct TransportMcView {
  const MCRegisterInfo &Registers;
  const MCInstrInfo &Instructions;
  const MCInstrAnalysis &Analysis;
};
constexpr uint16_t TransportNoSlot = UINT16_MAX;
enum class TransportBank : uint8_t { Scalar, Vector, Accumulator, Control };
enum class TransportReady : uint8_t { Unknown, Pending, Ready };
enum class TransportDomain : uint8_t { Unknown, Uniform, Entry, Intersection };
struct TransportFact {
  uint16_t Expression = 0, Generation = 0;
  TransportDomain Domain = TransportDomain::Unknown;
  TransportReady Ready = TransportReady::Unknown;
  uint8_t Origin = 0, Reserved = 0;
  bool operator==(const TransportFact &) const = default;
};
static_assert(sizeof(TransportFact) <= 8);
struct TransportState {
  std::array<TransportFact,64> Facts{};
  uint16_t VccGeneration = 0, NextGeneration = 1;
  TransportDomain Current = TransportDomain::Entry;
  bool LoadIssued = false, LoadReady = false, Compared = false, Masked = false;
  std::array<uint8_t,23> Reserved{};
};
static_assert(sizeof(TransportState) <= 544);
struct TransportRegister {
  uint16_t Id = 0, Tag = 0, Width = 0;
  uint8_t UnitCount = 0, WordCount = 0;
  std::array<uint16_t,16> Units{};
  std::array<uint16_t,8> Words{}; // Real register IDs, never cache indices.
  uint8_t Slot = UINT8_MAX;
  TransportBank Bank = TransportBank::Control;
  uint16_t Index = 0;
};
static_assert(sizeof(TransportRegister) <= 64);
struct TransportWord {
  uint16_t Id = 0, Index = 0;
  TransportBank Bank = TransportBank::Control;
  std::array<uint8_t,11> Reserved{};
};
static_assert(sizeof(TransportWord) <= 16);
struct TransportWorkspace {
  std::array<uint16_t,16384> UnitSlots;
  std::array<TransportRegister,128> Registers{};
  std::array<TransportWord,64> Words{};
  std::array<uint16_t,128> Whole32Classes{};
  std::array<TransportState,9> States{}; // Working + eight exits, no fixpoint.
  std::array<std::array<uint32_t,4>,64> SourceRows{};
  std::array<char,256> Root{}, Helper{};
  std::array<uint8_t,512> Expressions{};
  std::array<uint8_t,128> Domains{};
  std::array<uint8_t,512> CfgScratch{};
  std::array<uint8_t,128> Pending{};
  std::array<uint8_t,1024> SummaryScratch{};
  std::array<uint8_t,4096> InstructionScratch{};
  std::array<uint8_t,8192> DiagnosticScratch{};
};
static_assert(sizeof(TransportWorkspace) + sizeof(ct::Summary) + 1024
              <= ct::StorageBytes, "fixed transport account cannot be enlarged");
struct TransportProjection {
  uint16_t Root = 0, Helper = 0;
  std::array<uint32_t,4> RootParameters{};
  std::array<uint32_t,3> Formals{};
  uint32_t Result = 0, ProgramResult = 0;
};
bool transportNonzero(const std::array<uint8_t,32> &D) {
  return llvm::any_of(D,[](uint8_t B){return B != 0;});
}
// One linear pass over the exact source producer's row grammar. This checks a
// diagnostic relation, not source ownership; no bytes-to-owner conversion exists.
bool transportProjection(const ct::SourceProjection &S,
                         TransportProjection &P) {
  if (S.Root.empty() || S.Root.size()>256 || S.Helper.empty() ||
      S.Helper.size()>256 || S.Root==S.Helper || S.RowCount>64 ||
      !transportNonzero(S.Canonical) || !transportNonzero(S.Llvm) ||
      !transportNonzero(S.SourceReport) || !transportNonzero(S.Descriptor) ||
      (S.Selected!=ct::Profile::ConditionalHelperO0 &&
       S.Selected!=ct::Profile::InlineRootO3) ||
      (S.SelectedProgram!=ct::Program::XorAnd &&
       S.SelectedProgram!=ct::Program::MoveInput2))
    return false;
  size_t At=0;
  const auto take=[&](uint32_t Tag)->const std::array<uint32_t,4>* {
    if (At>=S.RowCount || S.Rows[At][0]!=Tag) return nullptr;
    return &S.Rows[At++];
  };
  const auto exact=[&](uint32_t T,uint32_t A,uint32_t B,uint32_t C) {
    const auto *R=take(T);return R && (*R)[1]==A && (*R)[2]==B && (*R)[3]==C;
  };
  const auto *Root=take(1);
  if (!Root || (*Root)[1]>=2 || (*Root)[2]!=4 || (*Root)[3]!=0) return false;
  P.Root=uint16_t((*Root)[1]);
  for (uint32_t I=0;I<4;++I) {
    const auto *R=take(2);
    if (!R || (*R)[1]!=I || (I==0?((*R)[3]!=2 && (*R)[3]!=3):(*R)[3]!=1)) return false;
    P.RootParameters[I]=(*R)[2];
    for (uint32_t J=0;J<I;++J) if (P.RootParameters[J]==(*R)[2]) return false;
  }
  const auto *Helper=take(3);
  if (!Helper || (*Helper)[1]>=2 || (*Helper)[1]==P.Root ||
      (*Helper)[2]!=3 || (*Helper)[3]!=1) return false;
  P.Helper=uint16_t((*Helper)[1]);
  for (uint32_t I=0;I<3;++I) {
    const auto *R=take(4);
    if (!R || (*R)[1]!=I || (*R)[3]!=1) return false;
    P.Formals[I]=(*R)[2];
    for (uint32_t J=0;J<I;++J) if (P.Formals[J]==(*R)[2]) return false;
  }
  const auto site=[&](uint32_t Role,uint32_t Function) {
    const auto *A=take(5);const auto *B=take(6);
    return A && B && (*A)[1]==Role && (*A)[2]<16 && (*A)[3]<=128 &&
           (*B)[1]==Role && (*B)[3]==Function;
  };
  if (!site(1,P.Root)) return false;
  const auto *Call=take(7);
  if (!Call || (*Call)[1]!=P.Helper || (*Call)[3]!=1) return false;
  P.Result=(*Call)[2];
  for (uint32_t I=0;I<3;++I)
    if (!exact(8,I,P.RootParameters[I+1],P.Formals[I]) ||
        !exact(16,I,P.RootParameters[I+1],P.RootParameters[I+1])) return false;
  if (!site(2,P.Helper) ||
      !exact(11,8,9,S.SelectedProgram==ct::Program::XorAnd?2:1)) return false;
  for (uint32_t I=0;I<3;++I)
    if (!exact(9,I,P.Formals[I],1) || !exact(12,I,10+I,0)) return false;
  if (S.SelectedProgram==ct::Program::XorAnd) {
    if (!exact(13,0,133,0) || !exact(13,1,315,0)) return false;
  } else if (!exact(13,0,40,0)) return false;
  const auto *ProgramResult=take(10);
  if (!ProgramResult || (*ProgramResult)[2]!=1 || (*ProgramResult)[3]!=0)
    return false;
  P.ProgramResult=(*ProgramResult)[1];
  if (!site(3,P.Helper) ||
      !exact(14,P.ProgramResult,P.Result,P.Helper) || !site(4,P.Root)) return false;
  const auto *Store=take(15);
  if (!Store || (*Store)[1]!=P.Result ||
      !exact(16,3,P.Result,P.Result) ||
      !exact(17,0,0,(8U<<16)|8) || !exact(18,0,8,(8U<<16)|8)) return false;
  for (uint32_t I=0;I<3;++I)
    if (!exact(19,I+1,16+4*I,(4U<<16)|4)) return false;
  if(At!=S.RowCount)return false;
  for(size_t I=S.RowCount;I<64;++I)
    for(uint32_t V:S.Rows[I])if(V!=0)return false;
  return true;
}
class TransportAttempt {
  TransportWorkspace W;
  ct::Summary Summary{};
  TransportProjection Projection{};
  uint16_t RegisterCount=0, WordCount=0, ClassCount=0, TupleCount=0;
#ifdef FE2O3_PRIVATE_COMPOSITION_TRANSPORT_CONTROLS_V1
  friend struct TransportControls;
#endif
  uint16_t FunctionCount=0, BlockCount=0, InstructionCount=0, EdgeCount=0;
  uint16_t RootLength=0, HelperLength=0;
  bool Bound=false, SelectedSeen=false, RootSeen=false, HelperSeen=false;
  bool Observing=false, Finished=false;
  ct::Refusal Failure=ct::Refusal::None;
  ct::Profile Profile;
  ct::Program Program;
  bool fail(ct::Refusal R) {
    if(Failure==ct::Refusal::None) Failure=R;
    return false;
  }
  StringRef root() const {return StringRef(W.Root.data(),RootLength);}
  StringRef helper() const {return StringRef(W.Helper.data(),HelperLength);}
  TransportRegister *find(unsigned Id);
  const TransportRegister *find(unsigned Id) const;
  bool cache(unsigned Id,const MCRegisterInfo &MRI);
  bool words(unsigned Id,unsigned Width,const MCRegisterInfo &MRI);
  bool tables(const MCRegisterInfo &MRI);
  bool shape(const DecodedInstruction &,const TransportMcView &);
  bool kill(unsigned Id,TransportState &State);
  TransportFact fact(unsigned Id,const TransportState &State) const;
  bool set(unsigned Id,TransportFact,TransportState &State);
  bool role(unsigned Id,TransportBank B,unsigned Index,unsigned Width=32) const;
  bool evaluate(ArrayRef<DecodedInstruction>,const FunctionCfg &,const TransportMcView &,
                const SymbolRecord &);
public:
  explicit TransportAttempt(const ct::SourceProjection &S)
      : Profile(S.Selected),Program(S.SelectedProgram) {
    // Caller has already reserved the complete fixed logical envelope/work.
    W.UnitSlots.fill(TransportNoSlot);
    if (!transportProjection(S,Projection)) {fail(ct::Refusal::SourceProjection);return;}
    W.SourceRows=S.Rows;
    std::copy(S.Root.begin(),S.Root.end(),W.Root.begin());
    std::copy(S.Helper.begin(),S.Helper.end(),W.Helper.begin());
    RootLength=uint16_t(S.Root.size());HelperLength=uint16_t(S.Helper.size());
    Summary.Selected=Profile;Summary.SelectedProgram=Program;
    Summary.Canonical=S.Canonical;Summary.Llvm=S.Llvm;
    Summary.SourceReport=S.SourceReport;Summary.Descriptor=S.Descriptor;
  }
  TransportAttempt(const TransportAttempt &)=delete;
  TransportAttempt &operator=(const TransportAttempt &)=delete;
  bool good() const {return Failure==ct::Refusal::None;}
  ct::Refusal refusal() const {return Failure;}
  bool bind(const MetadataKernel &,const SymbolRecord &,ArrayRef<uint8_t>);
  bool observe(const SymbolRecord &,ArrayRef<DecodedInstruction>,
               const FunctionCfg &,const TransportMcView &,const AnalyzedFunction &);
  bool finish(const PhysicalMachineEffectEvidence &);
  const ct::Summary &summary() const {return Summary;}
};
