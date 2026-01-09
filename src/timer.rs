// Timer implementation for Game Boy
// Registers:
// 0xFF04 - DIV  (Divider Register) - Increments at 16384 Hz
// 0xFF05 - TIMA (Timer Counter) - Increments at frequency specified by TAC
// 0xFF06 - TMA  (Timer Modulo) - Value loaded into TIMA on overflow
// 0xFF07 - TAC  (Timer Control) - Timer enable and frequency selection
//
// The GB timer uses a single 16-bit internal counter that increments every T-cycle.
// DIV is the upper 8 bits of this counter.
// TIMA increments on a falling edge of a specific bit in the counter, selected by TAC.
// Note: The tick() method receives M-cycles and converts to T-cycles (1 M-cycle = 4 T-cycles).

pub struct Timer {
    internal_counter: u16,       // Internal 16-bit counter (increments every T-cycle)
    tima: u8,                    // Timer counter
    tma: u8,                     // Timer modulo
    tac: u8,                     // Timer control
    pub interrupt_pending: bool, // Timer overflow interrupt flag
    overflow_cycles: u8,         // T-cycles remaining in overflow delay (4 T-cycles)
    tima_overflow_value: u8,     // TIMA value during overflow window
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            internal_counter: 0xABCC, // Post-boot ROM initial value
            tima: 0,
            tma: 0,
            tac: 0,
            interrupt_pending: false,
            overflow_cycles: 0,
            tima_overflow_value: 0,
        }
    }

    // Get the bit position in internal_counter that controls TIMA increments
    // Based on TAC frequency setting (bits 0-1)
    fn get_timer_bit(&self) -> u16 {
        match self.tac & 0x03 {
            0 => 9,  // 4096 Hz   - bit 9 (1024 cycles)
            1 => 3,  // 262144 Hz - bit 3 (16 cycles)
            2 => 5,  // 65536 Hz  - bit 5 (64 cycles)
            3 => 7,  // 16384 Hz  - bit 7 (256 cycles)
            _ => unreachable!(),
        }
    }

    // Check if TIMA should increment (timer enabled AND selected bit is 1)
    fn get_timer_enable_bit(&self) -> bool {
        let timer_enabled = (self.tac & 0x04) != 0;
        if !timer_enabled {
            return false;
        }
        let bit_pos = self.get_timer_bit();
        (self.internal_counter & (1 << bit_pos)) != 0
    }

    // Tick the timer by the given number of M-cycles (machine cycles)
    // The internal counter increments at T-cycle rate (4x M-cycle rate)
    pub fn tick(&mut self, m_cycles: u16) {
        // Convert M-cycles to T-cycles (1 M-cycle = 4 T-cycles)
        let t_cycles = m_cycles * 4;
        let debug = std::env::var("TIMER_DEBUG").is_ok(); // Set TIMER_DEBUG=1 to enable

        for i in 0..t_cycles {
            // Handle overflow delay countdown
            if self.overflow_cycles > 0 {
                self.overflow_cycles -= 1;

                if debug {
                    println!("    T+{}: overflow_cycles={}, TIMA=0x{:02X}", i, self.overflow_cycles, self.tima);
                }

                if self.overflow_cycles == 0 {
                    // Overflow delay complete - load TMA into TIMA and trigger interrupt
                    self.tima = self.tma;
                    self.interrupt_pending = true;
                    if debug {
                        println!("    T+{}: Loaded TMA=0x{:02X} into TIMA, interrupt set", i, self.tma);
                    }
                }

                // Still increment the internal counter during overflow delay
                self.internal_counter = self.internal_counter.wrapping_add(1);
                continue;
            }

            // Store previous timer enable bit state
            let old_enable_bit = self.get_timer_enable_bit();

            // Increment internal counter (at T-cycle rate)
            self.internal_counter = self.internal_counter.wrapping_add(1);

            // Check new timer enable bit state
            let new_enable_bit = self.get_timer_enable_bit();

            // Falling edge detection: increment TIMA if bit went from 1 to 0
            if old_enable_bit && !new_enable_bit {
                let (new_tima, overflow) = self.tima.overflowing_add(1);

                if debug {
                    println!("    T+{}: Falling edge detected, TIMA 0x{:02X} -> 0x{:02X}, overflow={}",
                             i, self.tima, new_tima, overflow);
                }

                if overflow {
                    // Start overflow delay (4 T-cycles)
                    self.overflow_cycles = 4;
                    self.tima_overflow_value = new_tima; // This is 0x00
                    self.tima = new_tima; // TIMA becomes 0 immediately
                    if debug {
                        println!("    T+{}: Overflow! Starting 4 T-cycle delay", i);
                    }
                } else {
                    self.tima = new_tima;
                }
            }
        }
    }

    // Read from timer registers
    pub fn read(&self, address: u16) -> u8 {
        match address {
            0xFF04 => (self.internal_counter >> 8) as u8, // DIV returns upper 8 bits of internal counter
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8, // Upper 5 bits always set
            _ => 0xFF,
        }
    }

    // Write to timer registers
    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0xFF04 => {
                // Writing any value to DIV resets internal counter to 0
                // This can cause a falling edge if the timer bit was 1
                let old_enable_bit = self.get_timer_enable_bit();
                self.internal_counter = 0;
                let new_enable_bit = self.get_timer_enable_bit(); // Will be false since counter is 0

                // Check for falling edge
                if old_enable_bit && !new_enable_bit {
                    let (new_tima, overflow) = self.tima.overflowing_add(1);

                    if overflow {
                        // Start overflow delay
                        self.overflow_cycles = 4;
                        self.tima_overflow_value = new_tima;
                        self.tima = new_tima;
                    } else {
                        self.tima = new_tima;
                    }
                }
            }
            0xFF05 => {
                // Writing to TIMA
                self.tima = value;

                // Writing to TIMA during overflow window cancels the TMA load and interrupt
                if self.overflow_cycles > 0 {
                    self.overflow_cycles = 0;
                    // Don't clear interrupt_pending if it was already set from a previous overflow
                }
            }
            0xFF06 => {
                // Writing to TMA
                // If write happens during overflow window, the new TMA will be loaded
                self.tma = value;
                if self.overflow_cycles > 0 {
                    // TMA write during overflow - will load this new value
                    // (handled in tick when overflow completes)
                }
            }
            0xFF07 => {
                // Writing to TAC can cause falling edge
                let old_enable_bit = self.get_timer_enable_bit();
                self.tac = value & 0x07; // Only lower 3 bits are used
                let new_enable_bit = self.get_timer_enable_bit();

                // Check for falling edge when TAC changes
                if old_enable_bit && !new_enable_bit {
                    let (new_tima, overflow) = self.tima.overflowing_add(1);

                    if overflow {
                        // Start overflow delay
                        self.overflow_cycles = 4;
                        self.tima_overflow_value = new_tima;
                        self.tima = new_tima;
                    } else {
                        self.tima = new_tima;
                    }
                }
            }
            _ => {}
        }
    }

    // Clear the interrupt flag (called after interrupt is serviced)
    pub fn clear_interrupt(&mut self) {
        self.interrupt_pending = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_div_increment() {
        let mut timer = Timer::new();
        timer.tick(256);
        assert_eq!(timer.read(0xFF04), 1);

        timer.tick(256);
        assert_eq!(timer.read(0xFF04), 2);
    }

    #[test]
    fn test_div_reset() {
        let mut timer = Timer::new();
        timer.tick(512);
        assert_eq!(timer.read(0xFF04), 2);

        timer.write(0xFF04, 0xFF); // Writing any value resets DIV
        assert_eq!(timer.read(0xFF04), 0);
    }

    #[test]
    fn test_tima_increment() {
        let mut timer = Timer::new();
        timer.write(0xFF07, 0x05); // Enable timer, 262144 Hz (16 cycles)
        timer.write(0xFF05, 0);

        timer.tick(16);
        assert_eq!(timer.read(0xFF05), 1);

        timer.tick(16);
        assert_eq!(timer.read(0xFF05), 2);
    }

    #[test]
    fn test_tima_overflow() {
        let mut timer = Timer::new();
        timer.write(0xFF07, 0x05); // Enable timer
        timer.write(0xFF05, 0xFF);
        timer.write(0xFF06, 0x42); // TMA value

        timer.tick(16);
        assert_eq!(timer.read(0xFF05), 0x42); // Should load TMA
        assert!(timer.interrupt_pending);
    }

    #[test]
    fn test_timer_disabled() {
        let mut timer = Timer::new();
        timer.write(0xFF07, 0x00); // Timer disabled
        timer.write(0xFF05, 0);

        timer.tick(1000);
        assert_eq!(timer.read(0xFF05), 0); // TIMA should not increment
    }
}
