.section .text.startup,"ax",@progbits
.global _start
.type _start,@function
_start:
  xorq %rbp, %rbp
  cld
  movq (%rsp), %rdi
  leaq 8(%rsp), %rsi
  andq $-16, %rsp
  call fe2o3_preexec_entry
  movl %eax, %edi
  movl $60, %eax
  syscall
  ud2
.size _start, .-_start

.section .note.GNU-stack,"",@progbits
