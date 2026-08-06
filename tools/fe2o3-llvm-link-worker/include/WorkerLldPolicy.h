#ifndef FE2O3_LLVM_LINK_WORKER_LLD_POLICY_H
#define FE2O3_LLVM_LINK_WORKER_LLD_POLICY_H

#include "lld/Common/Driver.h"
#include "lld/Common/ErrorHandler.h"

namespace fe2o3::worker::detail {

inline void enforceReusableLldResult(const lld::Result &Result) {
  if (!Result.canRunAgain)
    lld::exitLld(Result.retCode);
}

} // namespace fe2o3::worker::detail

#endif
