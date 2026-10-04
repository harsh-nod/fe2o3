  template<class Predicate>
  void checkpoint_require (checkpoint_site site, const Predicate &predicate) {
    static_assert (sizeof (Predicate) <= sizeof (checkpoint_diagnostic_scratch::predicate_reference_storage),
                   "checkpoint predicate closure exceeds its separate reservation");
    const bool was_valid=m_owner.selected () && !m_owner.invalid ();
    const bool yes=m_checkpoint_note.evaluate (was_valid,site,predicate);
    require (yes,failure::checkpoint_changed); // Original sticky refusal path.
  }
