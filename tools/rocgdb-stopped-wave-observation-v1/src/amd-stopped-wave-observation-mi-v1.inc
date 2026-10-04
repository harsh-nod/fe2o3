/* SPDX-License-Identifier: GPL-3.0-or-later
   This file is part of GDB; GPL version 3 or (at your option) any later version.
   Distinct observation-only protocol. Never a register/memory sample or token. */
void
mi_interp::on_amd_stopped_wave_observation_v1
  (const amd_stopped_wave_observation_v1::record &row)
{
  target_terminal::scoped_restore_terminal_state term_state;
  target_terminal::ours_for_output ();
  gdb_printf (event_channel, "amd-stopped-wave-observation-v1");
  ui_out_redirect_pop redir (mi_uiout, event_channel);
  mi_uiout->field_string ("schema", "fe2o3-gfx950-native-stopped-wave-observation-v1");
  mi_uiout->field_string ("profile", "fe2o3.gfx950.same-client-stopped-wave-observation.v1");
  mi_uiout->field_string ("queue-provenance", "unavailable");
  mi_uiout->field_unsigned ("version", 1);
  mi_uiout->field_unsigned ("sequence", row.sequence);
  mi_uiout->field_unsigned ("generation", row.generation);
  mi_uiout->field_unsigned ("inferior", row.inferior);
  mi_uiout->field_unsigned ("pid", row.pid);
  mi_uiout->field_unsigned ("process", row.process);
  mi_uiout->field_unsigned ("event", row.event);
  mi_uiout->field_unsigned ("event-kind", row.event_kind);
  mi_uiout->field_unsigned ("runtime-state", row.runtime_state);
  mi_uiout->field_unsigned ("callback", row.callback);
  mi_uiout->field_unsigned ("breakpoint", row.breakpoint);
  mi_uiout->field_unsigned ("thread", row.thread);
  mi_uiout->field_unsigned ("action", row.action);
  mi_uiout->field_signed ("status", row.status);
  mi_uiout->field_unsigned ("type", static_cast<unsigned> (row.type));
  mi_uiout->field_unsigned ("reason", static_cast<unsigned> (row.invalidation));
  mi_uiout->field_unsigned ("stop-generation", row.stop_generation);
  mi_uiout->field_unsigned ("wave", row.stop.wave);
  mi_uiout->field_unsigned ("workgroup", row.stop.workgroup);
  mi_uiout->field_unsigned ("dispatch", row.stop.dispatch);
  mi_uiout->field_unsigned ("queue", row.stop.queue);
  mi_uiout->field_unsigned ("agent", row.stop.agent);
  mi_uiout->field_unsigned ("architecture", row.stop.architecture);
  mi_uiout->field_unsigned ("os-queue", row.stop.os_queue);
  mi_uiout->field_unsigned ("packet", row.stop.packet);
  mi_uiout->field_unsigned ("os-agent", row.stop.os_agent);
  mi_uiout->field_unsigned ("private-bytes", row.stop.private_bytes);
  mi_uiout->field_unsigned ("group-bytes", row.stop.group_bytes);
  mi_uiout->field_unsigned ("wave-state", row.stop.state);
  mi_uiout->field_unsigned ("stop-reason", row.stop.stop_reason);
  mi_uiout->field_unsigned ("lanes", row.stop.lanes);
  mi_uiout->field_unsigned ("wave-index", row.stop.wave_index);
  mi_uiout->field_unsigned ("group-x", row.stop.coordinate[0]);
  mi_uiout->field_unsigned ("group-y", row.stop.coordinate[1]);
  mi_uiout->field_unsigned ("group-z", row.stop.coordinate[2]);
  mi_uiout->field_unsigned ("grid-x", row.stop.grid[0]);
  mi_uiout->field_unsigned ("grid-y", row.stop.grid[1]);
  mi_uiout->field_unsigned ("grid-z", row.stop.grid[2]);
  mi_uiout->field_unsigned ("size-x", row.stop.workgroup_size[0]);
  mi_uiout->field_unsigned ("size-y", row.stop.workgroup_size[1]);
  mi_uiout->field_unsigned ("size-z", row.stop.workgroup_size[2]);
  mi_uiout->field_unsigned ("queue-type", row.stop.queue_type);
  mi_uiout->field_unsigned ("queue-state", row.stop.queue_state);
  mi_uiout->field_unsigned ("elf-machine", row.stop.elf_machine);
  gdb_flush (event_channel);
}
