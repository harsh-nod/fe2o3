use fe2o3_kernel_descriptor::AdmittedRowSoftmaxV1StructuralDescriptorV1;

fn claim_runtime_length(admitted: AdmittedRowSoftmaxV1StructuralDescriptorV1) {
    let _ = admitted.declared_row_elements();
}

fn main() {}
