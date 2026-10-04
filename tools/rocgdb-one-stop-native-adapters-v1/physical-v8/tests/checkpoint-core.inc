// BEGIN bounded checkpoint-site diagnostic core.
enum class checkpoint_site : std::uint16_t {
  none = 0,
  hash_finish = 1,
  hash_inferior_extent = 2,
  hash_inferior_address = 3,
  executable_pass = 4,
  executable_before = 5,
  executable_eof = 6,
  executable_after = 7,
  executable_name = 8,
  executable_identity = 9,
  executable_recheck = 10,
  checkpoint_presentation = 11,
  selection_architecture = 12,
  selection_symfile = 13,
  selection_symbols = 14,
  selection_addresses = 15,
  owner_checkpoint_hit = 16,
  object_locator = 17,
  runtime_root_address = 18,
  transport_write = 19,
  transport_read = 20,
  transport_signal = 21,
  transport_ring = 22,
  transport_kernarg = 23,
  transport_output_address = 24,
  transport_output_bytes = 25,
  checkpoint_entry = 26,
  checkpoint_pc_argument = 27,
  checkpoint_argument_recheck = 28,
  checkpoint_codec = 29,
  checkpoint_relocation = 30,
  checkpoint_repeat = 31,
  checkpoint_callback_root = 32,
  checkpoint_root_recheck = 33,
  checkpoint_root_bind = 34,
  sole_queue_fields = 35,
  sole_queue_recheck = 36,
  checkpoint_stop_output = 37,
  snapshot_extent = 38
};
// A diagnostic label only. It never replaces a predicate, owner or permit.
struct checkpoint_note final {
  checkpoint_site site = checkpoint_site::none;
  template<class Predicate>
  bool evaluate (bool was_valid, checkpoint_site actual_site,
                 const Predicate &predicate) {
    const bool yes = predicate (); // Exactly once; exceptions propagate unchanged.
    if (was_valid && !yes && site == checkpoint_site::none
        && actual_site != checkpoint_site::none) site = actual_site;
    return yes;
  }
};
static_assert (sizeof (checkpoint_note) == 2, "fixed checkpoint diagnostic record");
static_assert (alignof (checkpoint_note) == 2, "fixed checkpoint diagnostic alignment");
// END bounded checkpoint-site diagnostic core.
