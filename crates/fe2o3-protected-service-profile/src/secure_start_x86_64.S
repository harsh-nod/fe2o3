.section .text.fe2o3_secure_start_v1,"ax",@progbits
.p2align 4
.globl fe2o3_secure_start_v1
.hidden fe2o3_secure_start_v1
.type fe2o3_secure_start_v1,@function
fe2o3_secure_start_v1:
    # The trusted launcher already applies these controls. Exec can reset
    # dumpability, so repeat them before libc or Rust can inspect inherited FDs.
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

    # The kernel's initial stack is argc, argv[argc], NULL, envp, NULL.
    # Inspect only after hardening, still before libc: no allocation or proc access.
    cmpq $1, (%rsp)
    jne .Lfe2o3_secure_start_fail
    movq 8(%rsp), %rdi
    testq %rdi, %rdi
    je .Lfe2o3_secure_start_fail
    cmpq $0, 16(%rsp)
    jne .Lfe2o3_secure_start_fail
    cmpq $0, 24(%rsp)
    jne .Lfe2o3_secure_start_fail
    cmpb $0, (%rdi)
    je .Lfe2o3_secure_start_fail
    movl ${max_argv0}, %ecx
    xorl %eax, %eax
    cld
    repne scasb
    jne .Lfe2o3_secure_start_fail

    # Private atomic flag is published only after every startup check succeeds.
    movb $1, {invocation}(%rip)
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
