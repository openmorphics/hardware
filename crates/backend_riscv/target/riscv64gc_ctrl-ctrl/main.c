#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <time.h>
#include <errno.h>
#include <string.h>

#define MMIO_BASE_ADDR  0x40000000UL
#define MMIO_SIZE       0x1000UL  // 4KB region

// Accelerator register offsets
#define ACCEL_CTRL      0x00  // Control register
#define ACCEL_STATUS    0x04  // Status register
#define DMA_ADDR        0x08  // DMA address register
#define DMA_LEN         0x0C  // DMA length register

// Control register bits
#define CTRL_START      (1 << 0)
#define CTRL_RESET      (1 << 1)

// Status register bits
#define STATUS_DONE     (1 << 0)
#define STATUS_BUSY     (1 << 1)

static inline uint64_t now_ns() {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

int main(void) {
    const char* graph = "ctrl";
    const char* backend = "riscv";
    const char* isa = "rv64gc";
    const char* simulator = "renode";

    printf("Starting control-plane test for graph '%s'\\n", graph);

    // Open /dev/mem for MMIO access
    int mem_fd = open("/dev/mem", O_RDWR | O_SYNC);
    if (mem_fd < 0) {
        fprintf(stderr, "Failed to open /dev/mem: %s\\n", strerror(errno));
        fprintf(stderr, "Note: This program requires root privileges or UIO driver\\n");
        return 1;
    }

    // Map MMIO region
    volatile uint32_t* mmio_base = (volatile uint32_t*)mmap(
        NULL, MMIO_SIZE, PROT_READ | PROT_WRITE, MAP_SHARED, mem_fd, MMIO_BASE_ADDR);
    
    if (mmio_base == MAP_FAILED) {
        fprintf(stderr, "Failed to mmap MMIO region: %s\\n", strerror(errno));
        close(mem_fd);
        return 1;
    }

    uint64_t t0 = now_ns();

    printf("Mapped MMIO region at 0x%lx\\n", MMIO_BASE_ADDR);

    // Reset accelerator
    printf("Resetting accelerator...\\n");
    mmio_base[ACCEL_CTRL/4] = CTRL_RESET;
    usleep(1000); // 1ms delay
    mmio_base[ACCEL_CTRL/4] = 0;

    // Configure DMA (dummy operation)
    if (1) {
        printf("Configuring DMA...\\n");
        mmio_base[DMA_ADDR/4] = 0x80000000; // Dummy DMA address
        mmio_base[DMA_LEN/4] = 1024;        // Dummy DMA length
    }

    // Start accelerator operation
    printf("Starting accelerator operation...\\n");
    mmio_base[ACCEL_CTRL/4] = CTRL_START;

    // Poll for completion
    int timeout = 1000; // 1000ms timeout
    uint32_t status;
    do {
        status = mmio_base[ACCEL_STATUS/4];
        if (status & STATUS_DONE) {
            break;
        }
        usleep(1000); // 1ms delay
        timeout--;
    } while (timeout > 0);

    uint64_t t1 = now_ns();

    if (timeout == 0) {
        printf("Operation timed out!\\n");
    } else {
        printf("Operation completed successfully\\n");
    }

    // Read final status
    status = mmio_base[ACCEL_STATUS/4];
    printf("Final status: 0x%08x\\n", status);

    double step_ns = (double)(t1 - t0);

    // Output telemetry in JSONL format
    printf("{\"metric\":\"kernel.step_ns\",\"value\":%.0f,\"labels\":{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}\\n",
           step_ns, graph, backend, isa, simulator);
    printf("{\"metric\":\"events.processed\",\"value\":%d,\"labels\":{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}\\n",
           1, graph, backend, isa, simulator);
    printf("{\"metric\":\"mmio.operations\",\"value\":%d,\"labels\":{\"graph\":\"%s\",\"backend\":\"%s\",\"isa\":\"%s\",\"simulator\":\"%s\"}}\\n",
           1 ? 5 : 3, graph, backend, isa, simulator);

    // Cleanup
    munmap((void*)mmio_base, MMIO_SIZE);
    close(mem_fd);

    return 0;
}
