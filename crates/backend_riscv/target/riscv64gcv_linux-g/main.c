#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <time.h>

/* RISC-V pass metadata (from pipeline/config):
 *  - align_bytes=16
 *  - quant_bits_default=8
 *  - fused_stages=op_fuse_scalar
 *  - rvv_enabled=false
 *  - vlen_bytes=0
 */

/* Conditionally include RVV intrinsics */
#if defined(__riscv_vector)
  #include <riscv_vector.h>
#endif

static inline uint64_t now_ns() {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

/* Optional RISC-V counters: rdcycle/rdinstret stubs when not building for RISC-V */
#if defined(__riscv) || defined(__riscv_xlen)
static inline uint64_t rdcycle(void) { uint64_t x; __asm__ volatile("rdcycle %0" : "=r"(x)); return x; }
static inline uint64_t rdinstret(void) { uint64_t x; __asm__ volatile("rdinstret %0" : "=r"(x)); return x; }
#else
static inline uint64_t rdcycle(void) { return 0ull; }
static inline uint64_t rdinstret(void) { return 0ull; }
#endif

int main(void) {
    const char* graph = "g";
    const char* backend = "riscv";
    const char* isa = "rv64gcv";
    const char* simulator = "qemu";

    uint64_t c0 = rdcycle();
    uint64_t i0 = rdinstret();

    uint64_t t0 = now_ns();

    /* Workloop: vectorized sum-reduction with scalar fallback */
#if defined(__riscv_vector)
    size_t n = 100000;
    uint64_t* data = (uint64_t*)malloc(n * sizeof(uint64_t));
    if (!data) return 1;
    for (size_t ii = 0; ii < n; ++ii) { data[ii] = (uint64_t)ii; }

    size_t ii = 0;
    size_t vl1 = vsetvl_e64m1(1);
    vuint64m1_t v_acc = vmv_v_x_u64m1(0, vl1);
    while (ii < n) {
        size_t vl = vsetvl_e64m8(n - ii);
        vuint64m8_t v_data = vle64_v_u64m8(&data[ii], vl);
        v_acc = vredsum_vs_u64m8_u64m1(v_data, v_acc, vl);
        ii += vl;
    }
    uint64_t sum = vmv_x_s_u64m1_u64(v_acc);
    volatile uint64_t acc = sum;
    free(data);
#else
    volatile uint64_t acc = 0;
    for (size_t i = 0; i < 100000; ++i) { acc += (uint64_t)i; }
#endif

    uint64_t t1 = now_ns();

    uint64_t c1 = rdcycle();
    uint64_t i1 = rdinstret();

    double step_ns = (double)(t1 - t0);
    printf("{\"metric\":\"kernel.step_ns\",\"value\":%.0f,\"labels\":{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}\\n", step_ns, graph, backend, isa, simulator);
    printf("{\"metric\":\"events.processed\",\"value\":%d,\"labels\":{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}\\n", 100000, graph, backend, isa, simulator);
    printf("{\"metric\":\"cpu.cycle\",\"value\":%llu,\"labels\":{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}\\n",
           (unsigned long long)(c1 - c0), graph, backend, isa, simulator);
    printf("{\"metric\":\"cpu.instret\",\"value\":%llu,\"labels\":{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}\\n",
           (unsigned long long)(i1 - i0), graph, backend, isa, simulator);
    (void)acc;
    return 0;
}
