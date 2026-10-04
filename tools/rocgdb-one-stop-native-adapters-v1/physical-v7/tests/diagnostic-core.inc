// BEGIN bounded native-result diagnostic core.
enum class native_result_site : std::uint8_t { none, target_memory, scalar_query };
enum class native_result_family : std::uint8_t {
  none, queue, wave, workgroup, dispatch, agent, architecture, other
};
// Diagnostic data only: no owner, permit, target pointer, allocation or retry.
struct native_result_note final {
  native_result_site site = native_result_site::none;
  native_result_family family = native_result_family::none;
  std::uint32_t selector = 0, bytes = 0;
  std::int32_t status = 0;
  void remember (bool selected, bool invalid, native_result_site actual_site,
                 native_result_family actual_family, std::uint32_t actual_selector,
                 std::uint32_t actual_bytes, std::int32_t actual_status) noexcept {
    if (!selected || invalid || site != native_result_site::none
        || actual_site == native_result_site::none || actual_status == 0) return;
    site = actual_site; family = actual_family; selector = actual_selector;
    bytes = actual_bytes; status = actual_status;
  }
};
static_assert (sizeof (native_result_note) == 16, "fixed diagnostic record");
static_assert (alignof (native_result_note) == 4, "fixed diagnostic alignment");
// END bounded native-result diagnostic core.
