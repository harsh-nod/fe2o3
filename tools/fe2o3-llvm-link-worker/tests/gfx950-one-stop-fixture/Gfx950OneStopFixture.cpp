// CPU/static diagnostic fixture only. No source, queue or stopped-wave authority.
#include "WorkerPipeline.h"
#include "llvm/ADT/SmallVector.h"
#include "llvm/ADT/StringExtras.h"
#include "llvm/BinaryFormat/AMDGPUMetadataVerifier.h"
#include "llvm/BinaryFormat/MsgPackDocument.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/InlineAsm.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Verifier.h"
#include "llvm/MC/MCAsmInfo.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCCodeEmitter.h"
#include "llvm/MC/MCFixup.h"
#include "llvm/MC/MCDisassembler/MCDisassembler.h"
#include "llvm/MC/MCInst.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/MCTargetOptions.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Object/ELFObjectFile.h"
#include "llvm/Support/AMDHSAKernelDescriptor.h"
#include "llvm/Support/Endian.h"
#include "llvm/Support/JSON.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include <algorithm>
#include <array>
#include <chrono>
#include <cstdlib>
#include <fcntl.h>
#include <limits>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <tuple>
#include <sys/stat.h>
#include <unistd.h>
#include <vector>
using namespace llvm;
using namespace fe2o3::worker;
namespace {
constexpr StringLiteral Name = "fe2o3_gfx950_one_stop_fixture";
constexpr size_t ArtifactCap = 1024 * 1024, ReportCap = 256 * 1024;
[[noreturn]] void fail(StringRef Message) { errs() << Message.take_front(4096) << '\n'; std::exit(1); }
void require(bool Good, StringRef Message) { if (!Good) fail(Message); }
Error refusal(StringRef Message) { return createStringError(inconvertibleErrorCode(), Message); }
std::string hex(ArrayRef<uint8_t> Bytes) { return toHex(Bytes, true); }
#include "OneStopInput.inc"
#include "OneStopMetadata.inc"
#include "OneStopMC.inc"
#include "OneStopInspection.inc"
#include "OneStopControls.inc"
#include "OneStopRun.inc"
} // namespace
int main(int Count, char **Arguments) { return run(Count, Arguments); }
