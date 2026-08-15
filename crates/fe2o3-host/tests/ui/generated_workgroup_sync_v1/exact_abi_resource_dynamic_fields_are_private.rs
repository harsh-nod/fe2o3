use fe2o3_host::{
    GeneratedWorkgroupLdsReductionV1HostAdapterV1, WorkgroupSyncImplicitKernargObservationV1,
    WorkgroupSyncKernelResourceObservationV1,
};

fn mutate_host(value: &mut GeneratedWorkgroupLdsReductionV1HostAdapterV1<'_, '_>) {
    value.explicit_kernarg = [0; 40];
    value.dynamic_lds_bytes = 0;
}

fn mutate_resource(value: &mut WorkgroupSyncKernelResourceObservationV1) {
    value.static_group_segment_bytes = 1;
    value.private_segment_bytes = 1;
}

fn mutate_implicit(value: &mut WorkgroupSyncImplicitKernargObservationV1) {
    value.hidden_dynamic_lds_offset = None;
    value.hidden_dynamic_lds_value = 0;
    value.aql_group_segment_bytes = 0;
}

fn main() {}
