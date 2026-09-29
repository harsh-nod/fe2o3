
.section .bss
.globl descriptor_invocation
descriptor_invocation:
    .zero 1
.p2align 3
.Lcore_limit:
    .zero 16

.section .text
.globl _start
_start:
    cmpb $1, descriptor_invocation(%rip)
    jne .Lprobe_fail
    movl $157, %eax
    movl $3, %edi
    xorl %esi, %esi
    xorl %edx, %edx
    xorl %r10d, %r10d
    xorl %r8d, %r8d
    syscall
    testq %rax, %rax
    jne .Lprobe_fail
    movl $157, %eax
    movl $39, %edi
    syscall
    cmpq $1, %rax
    jne .Lprobe_fail
    movl $302, %eax
    xorl %edi, %edi
    movl $4, %esi
    xorl %edx, %edx
    leaq .Lcore_limit(%rip), %r10
    syscall
    testq %rax, %rax
    jne .Lprobe_fail
    cmpq $0, .Lcore_limit(%rip)
    jne .Lprobe_fail
    cmpq $0, .Lcore_limit+8(%rip)
    jne .Lprobe_fail
    xorl %edi, %edi
    jmp .Lprobe_exit
.Lprobe_fail:
    movl $127, %edi
.Lprobe_exit:
    movl $231, %eax
    syscall
    ud2

.section .note.GNU-stack,"",@progbits
