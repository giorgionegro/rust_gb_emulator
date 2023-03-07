// Timer implementation for Game Boy
// Registers:
// 0xFF04 - DIV  (Divider Register) - Increments at 16384 Hz
// 0xFF05 - TIMA (Timer Counter) - Increments at frequency specified by TAC
// 0xFF06 - TMA  (Timer Modulo) - Value loaded into TIMA on overflow
// 0xFF07 - TAC  (Timer Control) - Timer enable and frequency selection

pub struct Timer {
    div: u16,           // Internal divider (increments every cycle)
    tima: u8,           // Timer counter
    tma: u8,            // Timer modulo
    tac: u8,            // Timer control
    pub interrupt_pending: bool,  // Timer overflow interrupt flag
    internal_counter: u16,  // Internal counter for TIMA
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            interrupt_pending: false,
            internal_counter: 0,
        }
    }

    // Tick the timer by the given number of cycles
    pub fn tick(&mut self, cycles: u16) {
        // Update DIV register (increments every 256 cycles = 16384 Hz)
        self.div = self.div.wrapping_add(cycles);

        // Only update TIMA if timer is enabled (bit 2 of TAC)
        if self.tac & 0x04 != 0 {
            self.internal_counter += cycles;

            // Get the frequency divider based on bits 0-1 of TAC
            let threshold = match self.tac & 0x03 {
                0 => 1024,  // 4096 Hz
                1 => 16,    // 262144 Hz
                2 => 64,    // 65536 Hz
                3 => 256,   // 16384 Hz
                _ => unreachable!(),
            };

            // Increment TIMA when internal counter reaches threshold
            while self.internal_counter >= threshold {
                self.internal_counter -= threshold;
                
                let (new_tima, overflow) = self.tima.overflowing_add(1);
                self.tima = new_tima;
                
                if overflow {
                    // TIMA overflowed, load TMA and set interrupt flag
                    self.tima = self.tma;
                    self.interrupt_pending = true;
                }
            }
        }
    }

    // Read from timer registers
    pub fn read(&self, address: u16) -> u8 {
        match address {
            0xFF04 => (self.div >> 8) as u8,  // DIV returns upper 8 bits
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8,  // Upper 5 bits always set
            _ => 0xFF,
        }
    }

    // Write to timer registers
    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0xFF04 => {
                // Writing any value to DIV resets it to 0
                self.div = 0;
                self.internal_counter = 0;
            }
            0xFF05 => self.tima = value,
            0xFF06 => self.tma = value,
            0xFF07 => self.tac = value & 0x07,  // Only lower 3 bits are used
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
        
        timer.write(0xFF04, 0xFF);  // Writing any value resets DIV
        assert_eq!(timer.read(0xFF04), 0);
    }

    #[test]
    fn test_tima_increment() {
        let mut timer = Timer::new();
        timer.write(0xFF07, 0x05);  // Enable timer, 262144 Hz (16 cycles)
        timer.write(0xFF05, 0);
        
        timer.tick(16);
        assert_eq!(timer.read(0xFF05), 1);
        
        timer.tick(16);
        assert_eq!(timer.read(0xFF05), 2);
    }

    #[test]
    fn test_tima_overflow() {
        let mut timer = Timer::new();
        timer.write(0xFF07, 0x05);  // Enable timer
        timer.write(0xFF05, 0xFF);
        timer.write(0xFF06, 0x42);  // TMA value
        
        timer.tick(16);
        assert_eq!(timer.read(0xFF05), 0x42);  // Should load TMA
        assert!(timer.interrupt_pending);
    }

    #[test]
    fn test_timer_disabled() {
        let mut timer = Timer::new();
        timer.write(0xFF07, 0x00);  // Timer disabled
        timer.write(0xFF05, 0);
        
        timer.tick(1000);
        assert_eq!(timer.read(0xFF05), 0);  // TIMA should not increment
    }
}

