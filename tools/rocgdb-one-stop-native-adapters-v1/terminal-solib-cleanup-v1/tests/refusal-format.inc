  if (m_first_breakpoint_site!=breakpoint_site::none && m_terminal_delete_field!=0)
    error ("fixed owned one-stop native relation refused (%u); commit-site=%u; breakpoint-site=%u; breakpoint-type=%u; breakpoint-number=%d; terminal-delete-field=%u",
           static_cast<unsigned> (m_owner.why ()),
           static_cast<unsigned> (m_owner.first_commit_site ()),
           static_cast<unsigned> (m_first_breakpoint_site),
           m_first_breakpoint_type,m_first_breakpoint_number,
           static_cast<unsigned> (m_terminal_delete_field));
