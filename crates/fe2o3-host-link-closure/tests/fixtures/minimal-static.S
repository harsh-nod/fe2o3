.section .text._start,"ax",@progbits
.global _start
.type _start,@function
_start:
    mov $60, %eax
    xor %edi, %edi
    syscall
.size _start, .-_start

.section .comment,"MS",@progbits,1
.asciz "fe2o3-host-link-minimal-v1"

.section .note.GNU-stack,"",@progbits
