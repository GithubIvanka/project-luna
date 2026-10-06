extern void luna_main(void);

__attribute__((noreturn)) void _start(void) {
    luna_main();
    __builtin_unreachable();
}
