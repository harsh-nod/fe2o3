.section .text.fe2o3_parent_death_test,"ax",@progbits
.globl fe2o3_parent_death_test_start
.hidden fe2o3_parent_death_test_start
fe2o3_parent_death_test_start:
    subq $16, %rsp
    movl $0, (%rsp)
    movl $157, %eax
    movl $2, %edi
    movq %rsp, %rsi
    xorl %edx, %edx
    xorl %r10d, %r10d
    xorl %r8d, %r8d
    syscall
    testq %rax, %rax
    jne .Lpdeath_fail
    cmpl $9, (%rsp)
    jne .Lpdeath_fail
    movl $1, %eax
    movl $3, %edi
    movq %rsp, %rsi
    movl $1, %edx
    syscall
    cmpq $1, %rax
    jne .Lpdeath_fail
.Lpdeath_wait:
    movl $34, %eax
    syscall
    jmp .Lpdeath_wait
.Lpdeath_fail:
    movl $231, %eax
    movl $98, %edi
    syscall
    ud2
.globl fe2o3_parent_death_test_end
.hidden fe2o3_parent_death_test_end
fe2o3_parent_death_test_end:
.section .note.GNU-stack,"",@progbits
