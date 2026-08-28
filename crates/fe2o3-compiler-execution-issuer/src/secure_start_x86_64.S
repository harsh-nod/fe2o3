.section .text.fe2o3_secure_start_v1,"ax",@progbits
.p2align 4
.globl fe2o3_secure_start_v1
.hidden fe2o3_secure_start_v1
.type fe2o3_secure_start_v1,@function
fe2o3_secure_start_v1:
    # The static launcher already applies these controls. Exec can reset
    # dumpability, so repeat them before musl or Rust can inspect inherited FDs.
    pushq %rdx

    movl $157, %eax
    movl $4, %edi
    xorl %esi, %esi
    xorl %edx, %edx
    xorl %r10d, %r10d
    xorl %r8d, %r8d
    syscall
    testq %rax, %rax
    jne .Lfe2o3_secure_start_fail

    movl $157, %eax
    movl $38, %edi
    movl $1, %esi
    xorl %edx, %edx
    xorl %r10d, %r10d
    xorl %r8d, %r8d
    syscall
    testq %rax, %rax
    jne .Lfe2o3_secure_start_fail

    movl $302, %eax
    xorl %edi, %edi
    movl $4, %esi
    leaq .Lfe2o3_zero_core_limit(%rip), %rdx
    xorl %r10d, %r10d
    syscall
    testq %rax, %rax
    jne .Lfe2o3_secure_start_fail

    movl $157, %eax
    movl $3, %edi
    xorl %esi, %esi
    xorl %edx, %edx
    xorl %r10d, %r10d
    xorl %r8d, %r8d
    syscall
    testq %rax, %rax
    jne .Lfe2o3_secure_start_fail

    movl $157, %eax
    movl $39, %edi
    xorl %esi, %esi
    xorl %edx, %edx
    xorl %r10d, %r10d
    xorl %r8d, %r8d
    syscall
    cmpq $1, %rax
    jne .Lfe2o3_secure_start_fail

    popq %rdx
    jmp _start

.Lfe2o3_secure_start_fail:
    movl $231, %eax
    movl $126, %edi
    syscall
    ud2
.size fe2o3_secure_start_v1, .-fe2o3_secure_start_v1

.section .rodata.fe2o3_secure_start_v1,"a",@progbits
.p2align 3
.Lfe2o3_zero_core_limit:
    .quad 0
    .quad 0

.section .note.GNU-stack,"",@progbits
