  if (m_owner.why ()==failure::budget && m_owner.m_budget_failure.present)
    error ("fixed owned one-stop native relation refused (%u); commit-site=%u; budget-counter=%u; budget-phase=%u; budget-used=%llu; budget-request=%llu; budget-cap=%llu",
           static_cast<unsigned> (m_owner.why ()),
           static_cast<unsigned> (m_owner.first_commit_site ()),
           static_cast<unsigned> (m_owner.m_budget_failure.kind),
           static_cast<unsigned> (m_owner.m_budget_failure.state),
           static_cast<unsigned long long> (m_owner.m_budget_failure.used),
           static_cast<unsigned long long> (m_owner.m_budget_failure.requested),
           static_cast<unsigned long long> (m_owner.m_budget_failure.cap));
