
#include <stdint.h>
#include <stddef.h>

#define UART0_BASE      0x10000000UL
#define UART_THR        (UART0_BASE + 0x00)
#define UART_LSR        (UART0_BASE + 0x05)
#define LSR_THRE        0x20

#define QEMU_FINISHER_BASE 0x00100000UL
#define QEMU_FINISHER_PASS 0x5555

static inline void mmio_write8(uintptr_t addr, uint8_t val) { *(volatile uint8_t*)addr = val; }
static inline uint8_t mmio_read8(uintptr_t addr) { return *(volatile uint8_t*)addr; }

static void uart_putc(char c) {
    /* wait for THR empty */
    while ((mmio_read8(UART_LSR) & LSR_THRE) == 0) { }
    mmio_write8(UART_THR, (uint8_t)c);
}

static void uart_puts(const char* s) {
    while (*s) {
        uart_putc(*s++);
    }
}

static void print_u32(uint32_t x) {
    char buf[11]; // max 10 digits + NUL
    int i = 0;
    if (x == 0) { uart_putc('0'); return; }
    while (x > 0 && i < 10) {
        uint32_t q = x / 10;
        uint32_t r = x - q * 10;
        buf[i++] = (char)('0' + r);
        x = q;
    }
    while (i--) uart_putc(buf[i]);
}

static inline uint32_t rdcycle(void) {
    uint32_t x; __asm__ volatile("csrr %0, cycle" : "=r"(x)); return x;
}
static inline uint32_t rdinstret(void) {
    uint32_t x; __asm__ volatile("csrr %0, instret" : "=r"(x)); return x;
}

static inline void qemu_exit(uint32_t code) {
    volatile uint32_t* fin = (volatile uint32_t*)QEMU_FINISHER_BASE;
    /* Encode status: (code<<16) | PASS */
    *fin = (code << 16) | QEMU_FINISHER_PASS;
}

int main(void) {
    const char* graph = "example-json";
    const char* backend = "riscv";
    const char* isa = "rv32imac";
    const char* simulator = "qemu";

    volatile uint32_t acc = 0;
    uint32_t c0 = rdcycle();
    uint32_t i0 = rdinstret();

    for (uint32_t i = 0; i < 100000; ++i) { acc += i; }

    uint32_t c1 = rdcycle();
    uint32_t i1 = rdinstret();
    uint32_t dc = c1 - c0;
    uint32_t di = i1 - i0;

    /* JSONL lines */
    uart_puts("{\"metric\":\"kernel.step_ns\",\"value\":"); print_u32(dc); uart_puts(",\"labels\":{\"graph\":\"");
    uart_puts(graph); uart_puts("\",\"backend\":\""); uart_puts(backend); uart_puts("\",\"isa\":\""); uart_puts(isa);
    uart_puts("\",\"simulator\":\""); uart_puts(simulator); uart_puts("\"}}\\n");

    uart_puts("{\"metric\":\"events.processed\",\"value\":"); print_u32(100000u); uart_puts(",\"labels\":{\"graph\":\"");
    uart_puts(graph); uart_puts("\",\"backend\":\""); uart_puts(backend); uart_puts("\",\"isa\":\""); uart_puts(isa);
    uart_puts("\",\"simulator\":\""); uart_puts(simulator); uart_puts("\"}}\\n");

    uart_puts("{\"metric\":\"cpu.cycle\",\"value\":"); print_u32(dc); uart_puts(",\"labels\":{\"graph\":\"");
    uart_puts(graph); uart_puts("\",\"backend\":\""); uart_puts(backend); uart_puts("\",\"isa\":\""); uart_puts(isa);
    uart_puts("\",\"simulator\":\""); uart_puts(simulator); uart_puts("\"}}\\n");

    uart_puts("{\"metric\":\"cpu.instret\",\"value\":"); print_u32(di); uart_puts(",\"labels\":{\"graph\":\"");
    uart_puts(graph); uart_puts("\",\"backend\":\""); uart_puts(backend); uart_puts("\",\"isa\":\""); uart_puts(isa);
    uart_puts("\",\"simulator\":\""); uart_puts(simulator); uart_puts("\"}}\\n");

    (void)acc;
    qemu_exit(0);
    for(;;) { }
    return 0;
}
