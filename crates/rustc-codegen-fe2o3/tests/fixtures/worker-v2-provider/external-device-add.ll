target triple = "amdgcn-amd-amdhsa"

define i32 @external_device_add_v1(i32 %value) #0 {
entry:
  %result = add i32 %value, 9
  ret i32 %result
}

attributes #0 = { nounwind "target-cpu"="gfx942" }
