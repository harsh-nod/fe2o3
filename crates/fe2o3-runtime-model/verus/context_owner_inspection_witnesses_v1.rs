verus! {

#[verifier::spinoff_prover]
fn owner_inspection_constructor_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_constructor_success_fixture_v1(false);
    let ghost before = actual;
    let ghost model_before = model;
    let direct = inspection_journal_paired_exec_v1(&mut actual.stable.journal, &mut model.stable.journal);
    let stable_explicit = inspection_stable_explicit_paired_exec_v1(&mut actual.stable, &mut model.stable);
    let stable_auto = inspection_stable_auto_paired_exec_v1(&mut actual.stable, &mut model.stable);
    let producer_explicit = inspection_producer_explicit_paired_exec_v1(&mut actual, &mut model);
    let producer_auto = inspection_producer_auto_paired_exec_v1(&mut actual, &mut model);
    assert(direct.0 == (7u64, 3usize, 1usize, 0u64, 1usize, 0usize, 3usize));
    assert(stable_explicit.0 == (direct.0, 2usize));
    assert(stable_auto.0 == stable_explicit.0);
    assert(producer_explicit.0 == stable_explicit.0 && producer_auto.0 == producer_explicit.0);
    assert(actual == before && model == model_before);
    true
}

#[verifier::spinoff_prover]
fn owner_inspection_malformed_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_constructor_success_fixture_v1(false);
    actual.stable.journal.context_generation = 0;
    model.stable.journal.context_generation = 0;
    actual.stable.journal.allocation_capacity = usize::MAX;
    model.stable.journal.allocation_capacity = usize::MAX;
    actual.stable.journal.writer_capacity = 0;
    model.stable.journal.writer_capacity = 0;
    actual.stable.journal.registration_watermark = u64::MAX;
    model.stable.journal.registration_watermark = u64::MAX;
    actual.stable.journal.reserved_count = usize::MAX;
    model.stable.journal.reserved_count = usize::MAX;
    actual.stable.journal.free = vec![99usize, 99];
    model.stable.journal.free = vec![99usize, 99];
    actual.stable.journal.allocation_free = vec![usize::MAX];
    model.stable.journal.allocation_free = vec![usize::MAX];
    actual.stable.free_reads = vec![9usize, 9, 9, 9];
    model.stable.free_reads = vec![9usize, 9, 9, 9];
    proof {
        assert(actual.stable.journal.free@ =~= model.stable.journal.free@);
        assert(actual.stable.journal.allocation_free@ =~= model.stable.journal.allocation_free@);
        assert(actual.stable.free_reads@ =~= model.stable.free_reads@);
        assert(producer_represents(actual, model));
        assert(!logical::producer_invariant_v1(model));
    }
    let ghost before = actual;
    let ghost model_before = model;
    let direct = inspection_journal_paired_exec_v1(&mut actual.stable.journal, &mut model.stable.journal);
    let stable_explicit = inspection_stable_explicit_paired_exec_v1(&mut actual.stable, &mut model.stable);
    let stable_auto = inspection_stable_auto_paired_exec_v1(&mut actual.stable, &mut model.stable);
    let producer_explicit = inspection_producer_explicit_paired_exec_v1(&mut actual, &mut model);
    let producer_auto = inspection_producer_auto_paired_exec_v1(&mut actual, &mut model);
    assert(direct.0 == (0u64, usize::MAX, 0usize, u64::MAX, 2usize, usize::MAX, 1usize));
    assert(stable_explicit.0 == (direct.0, 4usize));
    assert(stable_auto.0 == stable_explicit.0);
    assert(producer_explicit.0 == stable_explicit.0 && producer_auto.0 == producer_explicit.0);
    assert(actual == before && model == model_before);
    true
}

}
