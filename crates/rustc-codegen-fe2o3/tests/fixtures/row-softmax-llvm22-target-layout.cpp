#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/CodeGen.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"
#include "llvm/TargetParser/Triple.h"

#include <memory>
#include <string>

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must name the measured upstream LLVM build"
#endif

int main() {
  static constexpr char ExpectedDataLayout[] =
      "e-m:e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-"
      "p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-"
      "i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-"
      "v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9";
  LLVMInitializeAMDGPUTargetInfo();
  LLVMInitializeAMDGPUTarget();
  LLVMInitializeAMDGPUTargetMC();

  llvm::Triple Triple("amdgcn-amd-amdhsa");
  std::string LookupError;
  const llvm::Target *Target =
      llvm::TargetRegistry::lookupTarget("amdgcn", Triple, LookupError);
  if (!Target) {
    llvm::errs() << "AMDGPU target lookup failed: " << LookupError << '\n';
    return 1;
  }

  llvm::TargetOptions Options;
  std::unique_ptr<llvm::TargetMachine> Machine(Target->createTargetMachine(
      Triple, "gfx942", "-xnack", Options, llvm::Reloc::PIC_,
      llvm::CodeModel::Small, llvm::CodeGenOptLevel::None));
  if (!Machine) {
    llvm::errs() << "AMDGPU target-machine creation failed\n";
    return 1;
  }

  std::string ObservedDataLayout =
      Machine->createDataLayout().getStringRepresentation();
  if (ObservedDataLayout != ExpectedDataLayout) {
    llvm::errs() << "row-softmax LLVM 22 layout drift: expected="
                 << ExpectedDataLayout << " observed=" << ObservedDataLayout
                 << '\n';
    return 1;
  }

  llvm::outs() << "llvm-build-identity=" FE2O3_LLVM_BUILD_ID "\n"
               << "target-triple=" << Triple.getTriple() << "\n"
               << "target-cpu=gfx942\n"
               << "target-features=-xnack\n"
               << "relocation-model=pic\n"
               << "code-model=small\n"
               << "codegen-opt-level=none\n"
               << "data-layout=" << ObservedDataLayout << "\n";
  return 0;
}
