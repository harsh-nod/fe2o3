use fe2o3_runtime_model::*;
use std::hint::black_box;
use std::time::Instant;

fn entry(local: u64) -> ContextAllocationEnrollmentV1 {
    ContextAllocationEnrollmentV1 {
        key: ContextAllocationKeyV1 {
            context_generation: 7,
            local,
        },
        device: ContextJournalDeviceKeyV1 {
            context_generation: 7,
            local: 1,
        },
        byte_extent: 64,
    }
}

fn free_order(capacity: usize, occupied: usize, order: usize) -> Vec<usize> {
    let mut free: Vec<_> = (occupied..capacity).collect();
    match order {
        0 => free.reverse(),
        1 => {}
        _ => {
            let mut state = 0x31415926u64;
            for i in (1..free.len()).rev() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                free.swap(i, state as usize % (i + 1));
            }
        }
    }
    free
}

fn prepare(capacity: usize, occupied: usize, order: usize) -> ContextVersionJournalV1 {
    let mut journal = ContextVersionJournalV1::new(7, capacity, 1).unwrap();
    let entries: Vec<_> = (0..capacity).map(|i| entry(10_000 + i as u64)).collect();
    let mut references = vec![None; capacity];
    journal
        .enroll_allocations(&entries, &mut references)
        .unwrap();
    for (index, reference) in references.iter().enumerate() {
        assert_eq!(reference.unwrap().slot, index);
    }
    for slot in free_order(capacity, occupied, order) {
        journal
            .retire_allocations(&[references[slot].unwrap()])
            .unwrap();
    }
    assert_eq!(journal.remaining_allocation_slots(), capacity - occupied);
    journal
}

fn main() {
    println!("capacity,occupied,batch,order,sample,operations,elapsed_ns,floor_ns");
    for capacity in [64, 1024, 4096] {
        for occupied in [0, capacity / 2] {
            let mut batches = vec![1, 16, 63, capacity - occupied];
            batches.sort_unstable();
            batches.dedup();
            for count in batches.into_iter().filter(|&n| n <= capacity - occupied) {
                let entries: Vec<_> = (0..count).map(|i| entry(1 + i as u64)).collect();
                for order in 0..3 {
                    let slots: Vec<_> = free_order(capacity, occupied, order)
                        .into_iter()
                        .rev()
                        .take(count)
                        .collect();
                    let operations = (65_536 / capacity).clamp(16, 256);
                    for sample in 0..5 {
                        let mut journals: Vec<_> = (0..operations)
                            .map(|_| prepare(capacity, occupied, order))
                            .collect();
                        let mut outputs = vec![vec![None; count]; operations];
                        let mut results =
                            vec![Err(ContextVersionJournalErrorV1::InvalidState); operations];
                        let floor_start = Instant::now();
                        for (j, out) in journals.iter_mut().zip(outputs.iter_mut()) {
                            black_box((j, out, &entries));
                        }
                        let floor = floor_start.elapsed().as_nanos();
                        let start = Instant::now();
                        for ((journal, output), result) in journals
                            .iter_mut()
                            .zip(outputs.iter_mut())
                            .zip(results.iter_mut())
                        {
                            *result = black_box(journal)
                                .enroll_allocations(black_box(&entries), black_box(output));
                        }
                        let elapsed = start.elapsed().as_nanos();
                        for ((journal, output), result) in
                            journals.iter().zip(&outputs).zip(results)
                        {
                            assert_eq!(result, Ok(()));
                            assert_eq!(
                                journal.remaining_allocation_slots(),
                                capacity - occupied - count
                            );
                            for ((reference, expected), &slot) in
                                output.iter().zip(&entries).zip(&slots)
                            {
                                let actual = journal.lookup_allocation(reference.unwrap()).unwrap();
                                assert_eq!(actual.device, expected.device);
                                assert_eq!(actual.byte_extent, expected.byte_extent);
                                assert_eq!(actual.content_lineage, 0);
                                assert_eq!(reference.unwrap().key, expected.key);
                                assert_eq!(reference.unwrap().slot, slot);
                            }
                        }
                        println!(
                            "{capacity},{occupied},{count},{order},{sample},{operations},{elapsed},{floor}"
                        );
                    }
                }
            }
        }
    }
}
