#ifndef FE2O3_LLVM_LINK_WORKER_PIPELINE_H
#define FE2O3_LLVM_LINK_WORKER_PIPELINE_H

#include "WorkerProtocol.h"

namespace fe2o3::worker {

Response execute(const Request &RequestValue);

} // namespace fe2o3::worker

#endif
