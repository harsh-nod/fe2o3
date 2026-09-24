// LLVM is observed intact. This validator does not replay source or perform lowering.
std::string compositionAssembly(size_t Kind) {
  if(Kind==0)return "v_xor_b32_e32 v32, $1, $2\n\tv_and_b32_e32 $0, v32, $3";
  if(Kind==1)return "v_mov_b32_e32 $0, $1";
  if(Kind==2)return "v_mov_b32_e32 $0, $2";
  fail("closed region kind");
}
bool compositionLlvmMatches(const Module &M,const CompositionRecord &R) {
  const auto &In=R.Input;
  if(M.getTargetTriple().str()!=TripleName || !M.getModuleInlineAsm().empty() ||
     !M.global_empty() || !M.alias_empty() || !M.ifunc_empty() ||
     M.getDataLayoutStr().empty())return false;
  const Function *Kernel=M.getFunction(In.Entry);
  if(!Kernel || Kernel->isDeclaration() || !Kernel->hasExternalLinkage() ||
     Kernel->getCallingConv()!=CallingConv::AMDGPU_KERNEL || Kernel->isVarArg() ||
     !Kernel->getReturnType()->isVoidTy() || Kernel->arg_size()!=5)return false;
  const auto *Pointer=dyn_cast<PointerType>(Kernel->getArg(0)->getType());
  if(!Pointer || Pointer->getAddressSpace()!=1 ||
     !Kernel->getArg(1)->getType()->isIntegerTy(64))return false;
  for(unsigned I=2;I<5;++I)if(!Kernel->getArg(I)->getType()->isIntegerTy(32))return false;
  const MDNode *WG=Kernel->getMetadata("reqd_work_group_size");
  if(!WG || WG->getNumOperands()!=3)return false;
  for(unsigned I=0;I<3;++I) {
    const auto *N=mdconst::dyn_extract<ConstantInt>(WG->getOperand(I));
    if(!N || N->getZExtValue()!=(I==0?64:1))return false;
  }
  if(Kernel->getFnAttribute("amdgpu-flat-work-group-size").getValueAsString()!="64,64")return false;
  size_t Functions=0,Defined=0,Blocks=0,Instructions=0;
  std::vector<std::pair<std::string,size_t>> Regions;
  std::vector<std::string> Calls;
  for(const auto &F:M) {
    if(++Functions>16)return false;
    if(F.isDeclaration()) {if(!F.isIntrinsic())return false;continue;}
    ++Defined;
    const bool Root=&F==Kernel;
    if(!Root && !std::binary_search(In.Helpers.begin(),In.Helpers.end(),F.getName().str()))return false;
    if(F.getFnAttribute("target-cpu").getValueAsString()!="gfx942" ||
       !F.hasFnAttribute(Attribute::NoUnwind))return false;
    const auto Features=F.getFnAttribute("target-features").getValueAsString();
    if(Features!="-wavefrontsize32,+wavefrontsize64,-xnack" &&
       Features!="-xnack,-wavefrontsize32,+wavefrontsize64")return false;
    if(!Root && (!F.hasInternalLinkage() || F.getCallingConv()!=CallingConv::C ||
        F.isVarArg() || F.arg_size()!=3 || !F.getReturnType()->isIntegerTy(32) ||
        !llvm::all_of(F.args(),[](const Argument &A){return A.getType()->isIntegerTy(32) && A.getType()->isFirstClassType();})))
      return false;
    for(const auto &B:F) {
      if(++Blocks>128)return false;
      for(const auto &I:B) {
        if(++Instructions>1024)return false;
        const auto *Call=dyn_cast<CallBase>(&I);if(!Call)continue;
        if(!isa<CallInst>(Call))return false;
        if(const auto *Asm=dyn_cast<InlineAsm>(Call->getCalledOperand())) {
          if(!Asm->hasSideEffects() || Asm->isAlignStack() || Asm->canThrow() ||
             Asm->getDialect()!=InlineAsm::AD_ATT || Asm->getConstraintString()!=Constraints ||
             Call->arg_size()!=3 || !Call->getType()->isIntegerTy(32) ||
             Call->getFunctionType()!=Asm->getFunctionType() ||
             !llvm::all_of(Call->args(),[](const Use &A){return A->getType()->isIntegerTy(32);}))return false;
          size_t Kind=0;
          for(;Kind<3 && Asm->getAsmString()!=compositionAssembly(Kind);++Kind){}
          if(Kind==3 || Regions.size()>=8)return false;
          Regions.emplace_back(F.getName().str(),Kind);
        } else {
          const auto *Callee=Call->getCalledFunction();
          if(!Callee)return false;
          if(Callee->isIntrinsic())continue;
          if(!Root || !std::binary_search(In.Helpers.begin(),In.Helpers.end(),Callee->getName().str()) ||
             Call->getCallingConv()!=CallingConv::C || Call->arg_size()!=3 ||
             !Call->getType()->isIntegerTy(32) || Calls.size()>=8 ||
             !llvm::all_of(Call->args(),[](const Use &A){return A->getType()->isIntegerTy(32);}))return false;
          Calls.push_back(Callee->getName().str());
        }
      }
    }
  }
  if(Defined!=In.Helpers.size()+1 || Regions.size()!=R.Definitions.size())return false;
  std::vector<std::pair<std::string,size_t>> Expected;
  for(const auto &D:R.Definitions)Expected.emplace_back(D.Symbol,compositionKind(D.Descriptors));
  std::sort(Regions.begin(),Regions.end());std::sort(Expected.begin(),Expected.end());
  auto ExpectedCalls=R.Calls;std::sort(Calls.begin(),Calls.end());std::sort(ExpectedCalls.begin(),ExpectedCalls.end());
  return Regions==Expected && Calls==ExpectedCalls;
}
json::Object compositionLlvm(CompositionRecord &R) {
  LLVMContext Context;SMDiagnostic Diagnostic;
  const std::string Text(R.Input.Llvm.Bytes.begin(),R.Input.Llvm.Bytes.end());
  auto M=parseAssemblyString(Text,Diagnostic,Context);
  require(M && !verifyModule(*M,&errs()) && compositionLlvmMatches(*M,R),"unchanged exact composition LLVM closure/ABI/regions");
  auto Machine=createMachine();
  require(M->getDataLayoutStr()==Machine->createDataLayout().getStringRepresentation(),"pinned worker data layout");
  const auto *Root=M->getFunction(R.Input.Entry);
  json::Array Arguments,Helpers;
  for(size_t I=0;I<5;++I) {
    const auto Name=Root->getArg(I)->getName().str();
    require(!Name.empty() && Name.size()<=128,"actual LLVM argument name bound");
    R.ArgumentNames[I]=Name;Arguments.emplace_back(Name);
  }
  for(const auto &Name:R.Input.Helpers) {
    const auto *F=M->getFunction(Name);
    Helpers.emplace_back(json::Object{{"symbol",Name},{"calling_convention","C"},
      {"u32_arguments",3},{"u32_results",1},{"llvm_noinline",F->hasFnAttribute(Attribute::NoInline)},
      {"llvm_alwaysinline",F->hasFnAttribute(Attribute::AlwaysInline)}});
  }
  // Mutation tests alter only an independent parser copy/retained expectation.
  size_t Refusals=0;
  const auto Reject=[&](CompositionRecord Changed) {
    require(!compositionLlvmMatches(*M,Changed),"LLVM negative accidentally admitted");++Refusals;
  };
  auto Changed=R;Changed.Input.Entry+="_wrong";Reject(std::move(Changed));
  Changed=R;Changed.Definitions[0].Symbol+="_wrong";Reject(std::move(Changed));
  Changed=R;Changed.Definitions.pop_back();Reject(std::move(Changed));
  if(!R.Calls.empty()) {Changed=R;Changed.Calls[0]+="_wrong";Reject(std::move(Changed));}
  if(!R.Input.Helpers.empty()) {Changed=R;Changed.Input.Helpers[0]+="_wrong";Reject(std::move(Changed));}
  auto Original=Root->getCallingConv();
  M->getFunction(R.Input.Entry)->setCallingConv(CallingConv::C);
  require(!compositionLlvmMatches(*M,R),"wrong kernel calling convention admitted");++Refusals;
  M->getFunction(R.Input.Entry)->setCallingConv(Original);
  require(compositionLlvmMatches(*M,R),"LLVM mutation baseline restoration");
  return json::Object{{"kernel_native_components",5},{"argument_names",std::move(Arguments)},
    {"helpers",std::move(Helpers)},{"canonical_defined_regions",R.Definitions.size()},
    {"canonical_direct_calls",R.Calls.size()},{"llvm_negative_controls",Refusals},
    {"source_replayed_here",false},{"native_argument_result_transport_proved",false}};
}
