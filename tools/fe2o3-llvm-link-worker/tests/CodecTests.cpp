#include "WorkerProtocol.h"

#include "llvm/Support/Error.h"

#include <algorithm>
#include <array>
#include <cassert>
#include <cstdint>
#include <vector>

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must be supplied by CMake"
#endif
#ifndef FE2O3_WORKER_BUILD_ID
#error "FE2O3_WORKER_BUILD_ID must be supplied by CMake"
#endif

using namespace fe2o3::worker;

int main() {
  std::vector<uint8_t> Empty;
  auto EmptyResult = decodeRequest(Empty);
  assert(!EmptyResult);
  llvm::consumeError(EmptyResult.takeError());

  std::vector<uint8_t> BadMagic{'N', 'O', 'T', 'F', 'E', '2', 'O', '3'};
  auto BadMagicResult = decodeRequest(BadMagic);
  assert(!BadMagicResult);
  llvm::consumeError(BadMagicResult.takeError());

  std::vector<uint8_t> UnknownTag{'F', '3',  'L',  'R', 'E', 'Q', '0',
                                  '1', 0xff, 0xff, 0,   0,   0,   0};
  auto UnknownTagResult = decodeRequest(UnknownTag);
  assert(!UnknownTagResult);
  llvm::consumeError(UnknownTagResult.takeError());

  Response Failure{{},
                   {},
                   FE2O3_WORKER_BUILD_ID,
                   Stage::Decode,
                   {"z diagnostic", "a diagnostic", "a diagnostic"},
                   std::nullopt};
  auto Encoded = encodeResponse(std::move(Failure));
  assert(Encoded);
  const std::array<uint8_t, 8> ResponseMagic = {'F', '3', 'L', 'R',
                                                'S', 'P', '0', '1'};
  assert(Encoded->size() > ResponseMagic.size());
  assert(
      std::equal(ResponseMagic.begin(), ResponseMagic.end(), Encoded->begin()));

  std::vector<std::string> Sanitized = canonicalDiagnostics(
      {"/tmp/private-123/input: failure", "a", "a"}, "/tmp/private-123");
  assert(Sanitized.size() == 2);
  assert(Sanitized[0] == "<internal>/input: failure");
  assert(Sanitized[1] == "a");
  return 0;
}
