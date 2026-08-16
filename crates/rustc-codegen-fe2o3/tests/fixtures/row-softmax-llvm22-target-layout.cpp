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

  llvm::outs() << "llvm-build-identity=" FE2O3_LLVM_BUILD_ID "\n"
               << "target-triple=" << Triple.getTriple() << "\n"
               << "target-cpu=gfx942\n"
               << "target-features=-xnack\n"
               << "relocation-model=pic\n"
               << "code-model=small\n"
               << "codegen-opt-level=none\n"
               << "data-layout="
               << Machine->createDataLayout().getStringRepresentation()
               << "\n";
  return 0;
}
