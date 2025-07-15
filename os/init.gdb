target remote localhost:1234
b rust_main
layout split
c
b __switch
c
si 40
n 6
si 38
file ../user/target/riscv64gc-unknown-none-elf/debug/ch4_trace1
si
b main
c