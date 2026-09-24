// SPDX-License-Identifier: GPL-3.0-or-later
#include "../src/amd-dbgapi-one-stop-activation-v1.h"
#include "../src/amd-dbgapi-one-stop-locator-v1.h"
#include <cassert>
#include <cstring>
#include <limits>
using namespace amd_owned_one_stop_v1;
static_assert (!selection_available ());
int main () {
  assert (!selection_available ());
  const char exact[]="memory://2090559#offset=0x100000&size=5312";
  std::uint64_t address=0;
  assert (fixed_memory_locator (exact,sizeof exact,2090559,address) && address==0x100000);
  const char *bad[]={
    "memory://2090558#offset=0x100000&size=5312",
    "memory://02090559#offset=0x100000&size=5312",
    "memory://2090559#offset=0x0100000&size=5312",
    "memory://2090559#offset=0X100000&size=5312",
    "memory://2090559#offset=0xABCDEF&size=5312",
    "memory://2090559#offset=0x0&size=5312",
    "memory://2090559#offset=0xffffffffffffffff&size=5312",
    "memory://2090559#offset=0x100000&size=5311",
    "memory://2090559#offset=0x100000&size=05312",
    "memory://2090559#offset=0x100000&size=5312&more=1",
    "file:///proc/2090559/mem"
  };
  for (const auto *value:bad) {
    address=0x55;
    assert (!fixed_memory_locator (value,std::strlen(value)+1,2090559,address));
    assert (address==0x55);
  }
  assert (!fixed_memory_locator (exact,sizeof exact-1,2090559,address));
  assert (!fixed_memory_locator (nullptr,sizeof exact,2090559,address));
  assert (!fixed_memory_locator (exact,129,2090559,address));
  assert (!selection_available ()); // Parsing cannot enable the source gate.
}
