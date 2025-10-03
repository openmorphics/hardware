from Antmicro.Renode.Peripherals import IDoubleWordPeripheral
from Antmicro.Renode.Peripherals.Bus import BusRangeRegistration
from Antmicro.Renode.Core import Range

class SNNAccelerator(IDoubleWordPeripheral):
    def __init__(self):
        self.ctrl_reg = 0
        self.status_reg = 0
        self.dma_addr_reg = 0
        self.dma_len_reg = 0
        self.operation_active = False
        
    def ReadDoubleWord(self, offset):
        if offset == 0x00:  # ACCEL_CTRL
            return self.ctrl_reg
        elif offset == 0x04:  # ACCEL_STATUS
            return self.status_reg
        elif offset == 0x08:  # DMA_ADDR
            return self.dma_addr_reg
        elif offset == 0x0C:  # DMA_LEN
            return self.dma_len_reg
        else:
            print(f"[SNN_Accelerator] Read from unknown offset 0x{offset:02x}")
            return 0
            
    def WriteDoubleWord(self, offset, value):
        if offset == 0x00:  # ACCEL_CTRL
            self.ctrl_reg = value
            if value & 0x1:  # CTRL_START
                print("[SNN_Accelerator] Accelerator control register written: START operation")
                self.operation_active = True
                self.status_reg |= 0x2  # Set BUSY bit
                # Simulate operation completion
                self.status_reg |= 0x1  # Set DONE bit
                self.status_reg &= ~0x2  # Clear BUSY bit
                print("[SNN_Accelerator] Operation completed")
            elif value & 0x2:  # CTRL_RESET
                print("[SNN_Accelerator] Accelerator control register written: RESET")
                self.status_reg = 0
                self.operation_active = False
        elif offset == 0x04:  # ACCEL_STATUS (read-only, but allow writes for testing)
            print(f"[SNN_Accelerator] Status register write attempted: 0x{value:08x}")
        elif offset == 0x08:  # DMA_ADDR
            self.dma_addr_reg = value
            print(f"[SNN_Accelerator] DMA transfer configured: addr=0x{value:08x}")
        elif offset == 0x0C:  # DMA_LEN
            self.dma_len_reg = value
            print(f"[SNN_Accelerator] DMA transfer configured: len={value}")
        else:
            print(f"[SNN_Accelerator] Write to unknown offset 0x{offset:02x}: 0x{value:08x}")

    def Reset(self):
        self.ctrl_reg = 0
        self.status_reg = 0
        self.dma_addr_reg = 0
        self.dma_len_reg = 0
        self.operation_active = False
        print("[SNN_Accelerator] Reset")
