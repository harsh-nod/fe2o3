#include "WorkerProtocol.h"

#include "llvm/Support/Error.h"
#include "llvm/Support/raw_ostream.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <vector>

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must be supplied by CMake"
#endif
#ifndef FE2O3_WORKER_BUILD_ID
#error "FE2O3_WORKER_BUILD_ID must be supplied by CMake"
#endif

using namespace fe2o3::worker;

namespace {

int fail(const char *Message) {
  llvm::errs() << "codec test failure: " << Message << '\n';
  return 1;
}

} // namespace

int main() {
  std::vector<uint8_t> Empty;
  auto EmptyResult = decodeRequest(Empty);
  if (EmptyResult)
    return fail("empty request was accepted");
  llvm::consumeError(EmptyResult.takeError());

  std::vector<uint8_t> BadMagic{'N', 'O', 'T', 'F', 'E', '2', 'O', '3'};
  auto BadMagicResult = decodeRequest(BadMagic);
  if (BadMagicResult)
    return fail("request with bad magic was accepted");
  llvm::consumeError(BadMagicResult.takeError());

  std::vector<uint8_t> UnknownTag{'F', '3',  'L',  'R', 'E', 'Q', '0',
                                  '1', 0xff, 0xff, 0,   0,   0,   0};
  auto UnknownTagResult = decodeRequest(UnknownTag);
  if (UnknownTagResult)
    return fail("request with unknown tag was accepted");
  llvm::consumeError(UnknownTagResult.takeError());

  Response Failure{{},
                   {},
                   FE2O3_WORKER_BUILD_ID,
                   Stage::Decode,
                   {"z diagnostic", "a diagnostic", "a diagnostic"},
                   std::nullopt};
  auto Encoded = encodeResponse(std::move(Failure));
  if (!Encoded) {
    llvm::logAllUnhandledErrors(Encoded.takeError(), llvm::errs(),
                                "codec test failure: ");
    return 1;
  }
  const std::array<uint8_t, 8> ResponseMagic = {'F', '3', 'L', 'R',
                                                'S', 'P', '0', '1'};
  if (Encoded->size() <= ResponseMagic.size())
    return fail("encoded response is too short");
  if (!std::equal(ResponseMagic.begin(), ResponseMagic.end(), Encoded->begin()))
    return fail("encoded response has bad magic");

  std::vector<std::string> Sanitized = canonicalDiagnostics(
      {"/tmp/private-123/input: failure", "a", "a"}, "/tmp/private-123");
  if (Sanitized.size() != 2)
    return fail("canonical diagnostics have the wrong size");
  if (Sanitized[0] != "<internal>/input: failure")
    return fail("canonical diagnostics retained an internal path");
  if (Sanitized[1] != "a")
    return fail("canonical diagnostics were not sorted and deduplicated");
  return 0;
}
