std::uint64_t native_adapter::legacy_read_start_ticks () {
  require (m_inferior!=nullptr && m_inferior->pid>0,failure::owner_changed);
  const auto count=std::snprintf (m_proc_path.data (),m_proc_path.size (),
                                 "/proc/%d/stat",m_inferior->pid);
  require (count>0 && static_cast<std::size_t> (count)<m_proc_path.size (),failure::budget);
  debit (counter::proc_requested_bytes,m_proc.size ());
  debit (counter::work,m_proc.size ());
  scoped_fd fd (::open (m_proc_path.data (),O_RDONLY|O_CLOEXEC|O_NOFOLLOW));
  require (fd.get ()>=0,failure::owner_changed);
  const auto got=::read (fd.get (),m_proc.data (),m_proc.size ());
  require (got>0 && static_cast<std::size_t> (got)<m_proc.size ()
           && m_proc[static_cast<std::size_t> (got)-1]=='\n',failure::owner_changed);
  const auto end=static_cast<std::size_t> (got);
  std::size_t at=0; std::uint64_t pid=0;
  while (at<end && m_proc[at]>='0' && m_proc[at]<='9') {
    const unsigned digit=static_cast<unsigned> (m_proc[at++]-'0');
    require (pid<=(UINT64_MAX-digit)/10,failure::owner_changed); pid=pid*10+digit;
  }
  require (pid==static_cast<std::uint64_t> (m_inferior->pid)
           && at+2<end && m_proc[at]==' ' && m_proc[at+1]=='(',failure::owner_changed);
  std::size_t close=0;
  for (std::size_t n=at+2;n<end;++n) if (m_proc[n]==')') close=n;
  require (close>at+1 && close+4<end && m_proc[close+1]==' ',failure::owner_changed);
  at=close+2; // Field3 state; advance exactly to field22 starttime.
  for (unsigned field=3;field<22;++field) {
    require (at<end && m_proc[at]!=' ',failure::owner_changed);
    while (at<end && m_proc[at]!=' ') ++at;
    require (at<end,failure::owner_changed); ++at;
  }
  std::uint64_t ticks=0; const auto first=at;
  while (at<end && m_proc[at]>='0' && m_proc[at]<='9') {
    const unsigned digit=static_cast<unsigned> (m_proc[at++]-'0');
    require (ticks<=(UINT64_MAX-digit)/10,failure::owner_changed); ticks=ticks*10+digit;
  }
  require (at>first && at<end && m_proc[at]==' ' && ticks!=0,failure::owner_changed);
  return ticks;
}
