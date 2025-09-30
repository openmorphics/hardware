#include <stdio.h>
#include <stdint.h>
#include <time.h>

/* RISC-V pass metadata (from pipeline/config):
 *  - align_bytes=16
 *  - quant_bits_default=8
 *  - fused_stages=op_fuse_scalar
 */

static inline uint64_t now_ns() {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

int main(void) {
    const char* graph = "g";
    const char* backend = "riscv";
    const char* isa = "rv64gcv";
    const char* simulator = "qemu";
    uint64_t t0 = now_ns();
    volatile uint64_t acc = 0;
    for (int i = 0; i < 100000; ++i) { acc += (uint64_t)i; }
    uint64_t t1 = now_ns();
    double step_ns = (double)(t1 - t0);
    printf("{\"metric\":\"kernel.step_ns\",\"value\":%.0f,\"labels\":{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}\\n", step_ns, graph, backend, isa, simulator);
    printf("{\"metric\":\"events.processed\",\"value\":%d,\"labels\":{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}\\n", 100000, graph, backend, isa, simulator);
    (void)acc;
    return 0;
}
