#ifndef FE2O3_LLVM_LINK_WORKER_PIPELINE_H
#define FE2O3_LLVM_LINK_WORKER_PIPELINE_H

#include "WorkerProtocol.h"

namespace fe2o3::worker {

llvm::Expected<std::vector<std::string>>
inspectLinkedOutputForPublication(llvm::ArrayRef<uint8_t> Bytes,
                                  const Request &RequestValue);

Response execute(const Request &RequestValue);

} // namespace fe2o3::worker

#endif
