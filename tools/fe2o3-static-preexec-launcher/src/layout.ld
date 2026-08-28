ENTRY(_start)

PHDRS
{
  phdr PT_PHDR PHDRS FLAGS(4);
  headers PT_LOAD FILEHDR PHDRS FLAGS(4);
  text PT_LOAD FLAGS(5);
  rodata PT_LOAD FLAGS(4);
  data PT_LOAD FLAGS(6);
  stack PT_GNU_STACK FLAGS(6);
}

SECTIONS
{
  . = 0x400000 + SIZEOF_HEADERS;
  .header_anchor : { BYTE(0); } :headers

  . = ALIGN(0x1000);
  .text :
  {
    *(.text.startup)
    *(.text .text.*)
  } :text

  . = ALIGN(0x1000);
  .rodata :
  {
    *(.rodata .rodata.*)
  } :rodata

  . = ALIGN(0x1000);
  .data :
  {
    *(.data .data.*)
  } :data
  .bss :
  {
    *(.bss .bss.*)
    *(COMMON)
  } :data

  /DISCARD/ :
  {
    *(.comment)
    *(.note .note.*)
    *(.eh_frame .eh_frame.*)
  }
}
