# Exact external source compatibility allowlist, not executable authentication.
# Reviewed unchanged harsh-nod/fe2o3 base:
# c60cd746e63b34b9072a493744d73b87ed1defc3
# Twelve files measured by the worker CMake plus its directly included test
# support. No worker pipeline/decoder/protocol implementation is copied here.
function(instruction_check_worker_sources)
  set(Pins
    "e49e137a133b9b93cfb98b9d8021b9cc24e34844714bccb03fdc5f1b96eda035|18827|CMakeLists.txt"
    "dc61fef7a9eb8194a8d5f0ed6166a134e7fd8f6193efa4629effde189af13027|680|include/WorkerBuildConfig.h.in"
    "1796398c083900edf549e7d3081790a1fc4de050e3a5be68cff040393bda32de|1598|include/WorkerDeviceLibraryPolicy.h"
    "1202e9d875cf3b3441438a05ee33b916733d699fcf338fc99efb2481c05f497d|368|include/WorkerLldPolicy.h"
    "11508e4a7e232c76dfd7ff62c0e366f10651ef828f474c10d56e4031059fc34a|6394|include/WorkerMachineEffect.h"
    "55fe4064cef360502d00b12993c558674adba12338ac3472ce760552cfe290d6|732|include/WorkerPipeline.h"
    "16176e14a3996d641bfb31985cfeda4895c79cc8b229d21255856214a3186726|5522|include/WorkerProtocol.h"
    "16b20461c3cfe40a81d56cfa4268952716f30f754609e538308b9f4754556334|13161|src/WorkerDeviceLibraryPolicy.cpp"
    "69a106af31384bed8f3b412e9550be3355758bf7a7bcc891fd250da9152504bb|115346|src/WorkerMachineEffect.cpp"
    "2fe07481072463a103c768f4216c95353f80a0adc1f3c214f32a0d989a5f3ae3|91183|src/WorkerPipeline.cpp"
    "068b4085fe8e5b463d5206902476c85705f00d7d35c0b0ea8e20a531ec56b669|42433|src/WorkerProtocol.cpp"
    "4b10a412648ac2e1cb07475796cc35130260079af0959fea9ba8b127736d6e5e|13203|src/main.cpp"
    "566f9871c704bace0dc432a43f6e01d2517a891345e9b6bc0b12b51014254049|8740|tests/OrderedProgramWorkerSupport.inc")
  foreach(Pin IN LISTS Pins)
    string(REPLACE "|" ";" Fields "${Pin}")
    list(GET Fields 0 ExpectedDigest)
    list(GET Fields 1 ExpectedBytes)
    list(GET Fields 2 Relative)
    set(Path "${FE2O3_INSTRUCTION_WORKER_SOURCE}/${Relative}")
    if(NOT EXISTS "${Path}" OR IS_DIRECTORY "${Path}")
      message(FATAL_ERROR "Required unchanged worker source is absent: ${Relative}")
    endif()
    file(REAL_PATH "${Path}" Resolved)
    if(NOT Resolved STREQUAL Path)
      message(FATAL_ERROR "Worker source must not traverse symlinks: ${Relative}")
    endif()
    file(SIZE "${Path}" Bytes)
    if(NOT Bytes EQUAL ExpectedBytes)
      message(FATAL_ERROR "Worker source size differs from reviewed base: ${Relative}")
    endif()
    file(SHA256 "${Path}" Digest)
    if(NOT Digest STREQUAL ExpectedDigest)
      message(FATAL_ERROR "Worker source differs from reviewed base: ${Relative}")
    endif()
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${Path}")
  endforeach()
endfunction()
