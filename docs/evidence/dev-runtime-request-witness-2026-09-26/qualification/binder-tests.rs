use super::*;
use crate::RuntimeAllocationDeviceAdmissionV1 as Entry;

#[test]
fn qualification_generated_binder_rejects_missing_extent_order_leaf_brand_and_tail() {
    for mutation in 0..7 {
        let root = Entry::qualification_root_v1();
        let entry = Entry::qualification_entry_v1(&root, 7);
        let sibling = Entry::qualification_entry_v1(&root, 7);
        let mut fixture = Fixture::new();
        fixture
            .backend
            .qualification_install_binding_v1(entry.clone());
        let plan = fixture.prepare().unwrap();
        let before = fixture.backend.next_handle;
        let credits: Vec<_> = plan.members[..plan.count]
            .iter()
            .map(|member| {
                entry
                    .account()
                    .reserve_v1(member.unwrap().description.byte_len)
                    .unwrap()
                    .retain()
            })
            .collect();
        let foreign = sibling
            .account()
            .reserve_v1(plan.members[0].unwrap().description.byte_len)
            .unwrap()
            .retain();
        let mut witnesses = core::array::from_fn(|index| {
            if index < plan.count {
                let bytes = plan.members[index].unwrap().description.byte_len;
                Some(entry.qualification_witness_v1(plan.binding.device, &credits[index], bytes))
            } else {
                None
            }
        });
        match mutation {
            0 => (),
            1 => witnesses[0] = None,
            2 => {
                witnesses[0] =
                    Some(entry.qualification_witness_v1(plan.binding.device, &credits[0], 1))
            }
            3 => witnesses.swap(0, 1),
            4 => {
                witnesses[0] = Some(sibling.qualification_witness_v1(
                    plan.binding.device,
                    &foreign,
                    plan.members[0].unwrap().description.byte_len,
                ))
            }
            5 => {
                let other = KfdRuntimeBackendV1::mock();
                let context = crate::RuntimeContextV1::open(other).unwrap();
                witnesses[0] = Some(entry.qualification_witness_v1(
                    context.devices()[0].id(),
                    &credits[0],
                    plan.members[0].unwrap().description.byte_len,
                ));
            }
            6 => {
                witnesses[plan.count] = Some(entry.qualification_witness_v1(
                    plan.binding.device,
                    &credits[0],
                    plan.members[0].unwrap().description.byte_len,
                ))
            }
            _ => unreachable!(),
        }
        let result = fixture
            .backend
            .bind_generated_shell_requests_v1(plan, witnesses);
        assert_eq!(result.is_ok(), mutation == 0);
        assert_eq!(fixture.backend.next_handle, before);
        assert!(fixture.backend.allocations.is_empty());
        assert!(fixture.storage.control_available());
        fixture.assert_no_native_effects();
        for credit in credits {
            credit.release_after_rejection().unwrap();
        }
        foreign.release_after_rejection().unwrap();
    }
}
