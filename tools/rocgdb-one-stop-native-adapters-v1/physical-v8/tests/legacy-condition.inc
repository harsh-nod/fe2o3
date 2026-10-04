adapter.m_owner.state ()==phase::provisional_checkpoint
                       && inferior_thread ()==adapter.m_host
                       && !adapter.m_host->executing () && !adapter.m_host->resumed ()
                       && status!=nullptr && status->breakpoint_at==this
                       && status->bp_location_at.get ()==&first_loc ()
                       && status->stop && status->simd_lane_mask==0
                       && type==bp_breakpoint && number<0 && enable_state==bp_enabled
                       && !silent && ignore_count==0 && !commands
                       && cond_string==nullptr && extra_string==nullptr
