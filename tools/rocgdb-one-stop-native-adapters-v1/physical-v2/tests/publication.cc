/* SPDX-License-Identifier: GPL-3.0-or-later
   Pure inert controls only. Actual GDB/debit placement is checked separately.
   The immutable predecessor's fixtures cannot establish native custody. */
#define FE2O3_ONE_STOP_PUBLICATION_PURE_TEST
#include "snapshot-fixtures.inc"
#include "amd-dbgapi-one-stop-publication-v2.h"
#include <string>
#include <stdexcept>
namespace amd_owned_one_stop_v1 {
struct publication_fixture : inert {
  checkpoint cp;
  diagnostic row {8,phase::current_gpu_stop,failure::none};
  static void word (checkpoint &c,std::size_t at,std::uint64_t value) {
    for (unsigned n=0;n<8;++n) c.bytes[at+n]=static_cast<std::uint8_t> (value>>(n*8));
  }
  publication_fixture () {
    core_test::use (ledger).rows=8;
    cp.record_address=0x40000; cp.entry=0x20000+6144;
    cp.signal_base=stop.completion_base; cp.queue=stop.queue; cp.code_object=0x50000;
    stop.pc=cp.entry+80;
    word (cp,56,0x20000); word (cp,72,0x20000+1984); word (cp,80,cp.entry);
    word (cp,128,cp.signal_base); word (cp,144,0x30000); word (cp,160,0x10000);
    word (cp,168,272); word (cp,176,4096); word (cp,184,0x10008); word (cp,192,256);
  }
  void ready () { stage (); seal (); }
  usage pay (bool physical=true) {
    const auto before=ledger.consumed ();
    assert (core_test::charge (ledger,counter::work,physical ? 4608 : 256)); return before;
  }
};
struct publication_test {
  static bool render (snapshot_publication &p,publication_fixture &x,const usage &u) {
    return p.render (x.ledger,u,x.id,x.row,x.snapshot,x.stop,x.cp);
  }
  static void disabled_and_size () {
    assert (!snapshot_capture_available () && !snapshot_publication_available ());
    static_assert (!std::is_copy_constructible<snapshot_publication>::value);
    static_assert (!std::is_move_constructible<snapshot_publication>::value);
    static_assert (sizeof (snapshot_publication)<=1560);
  }
  static void actual_bytes_and_exact_fields () {
    publication_fixture x; x.ready (); snapshot_publication p;
    assert (render (p,x,x.pay ()));
    const std::string row=p.data ();
    assert (row.find("=fe2o3-owned-one-stop-v2,s=\"8\",p=\"8\",pid=\"5\",i=\"4\",a=\"7\",t=\"8\",start=\"6\"")==0);
    assert (row.find(",dwarf=\"44\",rbytes=\"4\",mbytes=\"272\",status=\"available\",reg=\"a7a7a7a7\",mem=\"")!=std::string::npos);
    assert (row.find(std::string(544,'b'))==std::string::npos);
    std::string raw; for (unsigned n=0;n<272;++n) raw+="5b";
    assert (row.find(raw)!=std::string::npos);
    assert (row.back()=='\n' && row.size()==p.m_size && row.size()<1316);
    // Deliberately differs from expected physical fixture values: no oracle fills buffers.
    assert (row.find("df9b5713")==std::string::npos);
  }
  static void unavailable () {
    for (unsigned kind=0;kind<3;++kind) {
      publication_fixture x; snapshot_publication p;
      assert (snapshot_test::begin (x.snapshot,x.ledger,x.id,x.stop));
      snapshot_test::raw (x.snapshot);
      if (kind==0) assert (snapshot_test::reg (x.snapshot,snapshot_read_result::unavailable));
      else {
        assert (snapshot_test::reg (x.snapshot));
        assert (snapshot_test::mem (x.snapshot,kind==1 ? snapshot_read_result::unavailable
                                                    : snapshot_read_result::success,kind==1 ? 0 : 271));
      }
      x.seal (); assert (!x.readable ()); assert (render (p,x,x.pay ()));
      const std::string row=p.data ();
      assert (row.find(kind==0 ? "status=\"register-unavailable\"" : kind==1
                              ? "status=\"memory-unavailable\"" : "status=\"short-memory\"")!=std::string::npos);
      assert (row.find(",reg=\"\",mem=\"\"\n")!=std::string::npos);
    }
  }
  static void no_values_before_seal () {
    for (unsigned stage=0;stage<3;++stage) {
      publication_fixture x; snapshot_publication p;
      if (stage>=1) x.stage ();
      if (stage>=2) x.confirm ();
      p.m_bytes.fill ('Q'); p.m_size=3; const auto prior=p.m_bytes;
      assert (!render (p,x,x.pay ())); assert (p.m_bytes==prior && p.m_size==3);
    }
  }
  static void denied_budget_before_side_effect () {
    for (unsigned kind=0;kind<4;++kind) {
      publication_fixture x; x.ready (); snapshot_publication p;
      p.m_bytes.fill ('Q'); p.m_size=3; const auto prior=p.m_bytes;
      const auto before=x.ledger.consumed ();
      if (kind==0) {} // No payment.
      if (kind==1) assert (core_test::charge (x.ledger,counter::work,4607));
      if (kind==2) assert (core_test::charge (x.ledger,counter::work,4609));
      if (kind==3) { assert (core_test::charge (x.ledger,counter::work,4608)); ++core_test::use (x.ledger).api; }
      assert (!render (p,x,before)); assert (p.m_bytes==prior && p.m_size==3);
      assert (!p.m_physical_attempted);
    }
  }
  static void exact_last_work_and_denial () {
    publication_fixture x; x.ready (); snapshot_publication p;
    const auto floor=x.ledger.consumed ().work;
    assert (core_test::charge (x.ledger,counter::work,limits::work-floor-4608));
    assert (render (p,x,x.pay ()) && x.ledger.consumed ().work==limits::work);
    const auto prior=p.m_bytes;
    assert (!core_test::charge (x.ledger,counter::work,1));
    assert (!p.still_current (x.ledger,x.id,x.snapshot,x.stop)); assert (p.m_bytes==prior);
  }
  static void owner_and_stop_mutations () {
    for (unsigned n=0;n<8;++n) {
      publication_fixture x; x.ready (); snapshot_publication p;
      switch (n) {
        case 0: ++x.id.inferior; break; case 1: ++x.id.program_space; break;
        case 2: ++x.id.process_owner; break; case 3: ++x.id.inferior_number; break;
        case 4: ++x.id.pid; break; case 5: ++x.id.pid_start; break;
        case 6: ++x.id.process; break; case 7: ++x.id.host_thread; break;
      }
      assert (!render (p,x,x.pay ())); assert (p.m_size==0);
    }
    for (unsigned n=0;n<9;++n) {
      publication_fixture x; x.ready (); snapshot_publication p;
      switch (n) {
        case 0: ++x.stop.thread; break; case 1: ++x.stop.wave; break; case 2: ++x.stop.workgroup; break;
        case 3: ++x.stop.dispatch; break; case 4: ++x.stop.queue; break; case 5: ++x.stop.agent; break;
        case 6: ++x.stop.architecture; break; case 7: ++x.stop.pc; break; case 8: ++x.stop.completion_base; break;
      }
      assert (!render (p,x,x.pay ()));
    }
  }
  static void checkpoint_mutations () {
    for (unsigned n=0;n<16;++n) {
      publication_fixture x; x.ready (); snapshot_publication p;
      switch(n) {
        case 0: ++x.cp.entry; break; case 1: ++x.cp.signal_base; break; case 2: ++x.cp.queue; break;
        case 3: x.cp.code_object=0; break; case 4: x.cp.record_address=0; break;
        default: x.cp.bytes[std::array<std::size_t,11>{{56,72,80,128,144,160,168,176,184,192,0}}[n-5]]^=1; break;
      }
      if (n==9) { publication_fixture::word (x.cp,144,0); }
      if (n==15) { // Unused checkpoint byte is not a second full-codec admission here.
        assert (render (p,x,x.pay ()));
      } else assert (!render (p,x,x.pay ()));
    }
  }
  static void duplicate_and_revoke () {
    publication_fixture x; x.ready (); snapshot_publication p;
    assert (render (p,x,x.pay ())); const std::string actual=p.data ();
    assert (!render (p,x,x.pay ())); assert (actual==p.data ());
    snapshot_test::revoke (x.snapshot); p.forget ();
    assert (!p.still_current (x.ledger,x.id,x.snapshot,x.stop)); assert (p.m_size==0);
    assert (snapshot_test::clear (x.snapshot));
  }
  static void normal_lifecycle_uses_retained_identity () {
    publication_fixture x; snapshot_publication p;
    x.row={8,phase::lifecycle_disappeared,failure::none};
    assert (render (p,x,x.pay (false)));
    assert (std::string(p.data ()).find(",p=\"11\"")!=std::string::npos);
    assert (std::string(p.data ()).find("reg=")==std::string::npos);
  }
  static void stale_floor_and_other_owner () {
    publication_fixture x,y; x.ready (); y.ready (); snapshot_publication p;
    const auto before=y.pay ();
    assert (!p.render (y.ledger,before,x.id,x.row,x.snapshot,x.stop,x.cp));
    publication_fixture z; z.ready (); snapshot_publication q;
    --core_test::use (z.ledger).logical_bytes;
    assert (!render (q,z,z.pay ()));
  }
  static void max_encoder_and_capacity () {
    snapshot_publication p;
    assert (p.literal ("=fe2o3-owned-one-stop-v2"));
    const auto n=UINT64_MAX;
    assert (p.field("s",n)&&p.field("p",n)&&p.field("pid",n)&&p.field("i",n)&&p.field("a",n)&&p.field("t",n)&&p.field("start",n));
    assert (p.m_size+1==206);
    assert (p.field("g",n)&&p.field("w",n)&&p.field("wg",n)&&p.field("d",n)&&p.field("q",n)&&p.field("ag",n)&&p.field("ar",n)&&p.field("pc",n)&&p.field("c",n)&&p.field("cp",n)&&p.field("co",n)&&p.field("entry",n)&&p.field("desc",n)&&p.field("kernarg",n)&&p.field("out",n)&&p.field("regid",n)&&p.field("dwarf",n)&&p.field("rbytes",n)&&p.field("mbytes",n));
    std::array<std::uint8_t,4> reg {}; std::array<std::uint8_t,272> mem {};
    assert (p.literal(",status=\"register-unavailable\",reg=\"")&&p.hex(reg)&&p.literal("\",mem=\"")&&p.hex(mem)&&p.literal("\"\n"));
    assert (p.m_size==1316 && p.m_bytes[p.m_size]==0);
    while (p.m_size<1535) assert (p.append_char('x'));
    const auto old=p.m_bytes; assert (!p.append_char('x') && p.m_bytes==old);
    assert (!p.literal("x") && p.m_bytes==old); assert (!p.hex(mem) && p.m_bytes==old);
  }
  static void composed_original_work_and_other_caps () {
    publication_fixture x; x.ready (); snapshot_publication p;
    // Seed prepaid4096 capture work; all other fixed-success categories from
    // reviewed PLAN, plus the real formatter debit below, not a reset meter.
    for (auto n:{87168u,13568u,8192u,1528u,48u,256u,640u,2080u,340u,192u,128u,508u})
      assert (core_test::charge (x.ledger,counter::work,n));
    const auto physical=x.row;
    for (unsigned n=0;n<15;++n) {
      x.row={1,phase::host_running,failure::none}; assert (render(p,x,x.pay(false))); p.forget ();
    }
    x.row=physical; assert (render (p,x,x.pay ())); assert (x.ledger.consumed ().work==127192);
    assert (x.ledger.consumed ().rows==8); // Formatting never allocates a diagnostic row.
    assert (core_test::charge (x.ledger,counter::work,3880));
    assert (!core_test::charge (x.ledger,counter::work,1));
    static_assert (187+4<=limits::api && 47904+276<=limits::read_bytes && 192+2<=limits::read_calls);
  }
  static void post_write_invalidation_not_success () {
    publication_fixture x; x.ready (); snapshot_publication p;
    assert (render (p,x,x.pay ())); const auto prefix=std::string(p.data());
    assert (!prefix.empty()); core_test::invalidate(x.ledger);
    assert (!p.still_current(x.ledger,x.id,x.snapshot,x.stop)); p.forget(); assert(p.m_size==0);
  }
  static void unavailable_never_reads_value_arrays () {
    publication_fixture x; snapshot_publication p;
    assert(snapshot_test::begin(x.snapshot,x.ledger,x.id,x.stop));
    assert(snapshot_test::reg(x.snapshot,snapshot_read_result::unavailable)); x.seal();
    snapshot_test::raw(x.snapshot); // Deliberate inert corruption, never a production write.
    assert(render(p,x,x.pay())); const std::string row=p.data();
    assert(row.find(",reg=\"\",mem=\"\"\n")!=std::string::npos);
    assert(row.find("a7a7a7a7")==std::string::npos);
  }
  static void partial_or_throwing_write_cannot_retry () {
    for(unsigned mode=0;mode<2;++mode) {
      publication_fixture x; x.ready(); snapshot_publication p;
      assert(render(p,x,x.pay())); const std::string written(p.data(),mode==0 ? 1 : p.m_size);
      try { throw std::runtime_error("inert write failure"); }
      catch(...) { p.forget(); snapshot_test::revoke(x.snapshot); core_test::invalidate(x.ledger); }
      assert(!written.empty() && p.m_size==0 && snapshot_test::clear(x.snapshot));
      assert(!p.still_current(x.ledger,x.id,x.snapshot,x.stop));
      const auto prior=x.ledger.consumed();
      assert(!p.render(x.ledger,prior,x.id,x.row,x.snapshot,x.stop,x.cp));
    }
  }
  static void run () {
    disabled_and_size (); actual_bytes_and_exact_fields (); unavailable ();
    no_values_before_seal (); denied_budget_before_side_effect (); exact_last_work_and_denial ();
    owner_and_stop_mutations (); checkpoint_mutations (); duplicate_and_revoke ();
    normal_lifecycle_uses_retained_identity (); stale_floor_and_other_owner ();
    max_encoder_and_capacity (); composed_original_work_and_other_caps (); post_write_invalidation_not_success ();
    unavailable_never_reads_value_arrays (); partial_or_throwing_write_cannot_retry ();
  }
};
}
int main () { amd_owned_one_stop_v1::publication_test::run (); }
