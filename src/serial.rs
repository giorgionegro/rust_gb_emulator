// Serial I/O implementation for Game Boy
// Registers:
// 0xFF01 - SB (Serial Transfer Data)
// 0xFF02 - SC (Serial Transfer Control)
//   Bit 7: Transfer Start Flag (1=Start, 0=None)
//   Bit 0: Shift Clock (1=Internal, 0=External)

pub struct Serial {
    sb: u8,                      // Serial transfer data
    sc: u8,                      // Serial transfer control
    pub interrupt_pending: bool, // Serial interrupt flag
    pub output_buffer: Vec<u8>,  // Buffer for captured output
}

impl Default for Serial {
    fn default() -> Self {
        Self::new()
    }
}

impl Serial {
    pub fn new() -> Serial {
        Serial {
            sb: 0,
            sc: 0,
            interrupt_pending: false,
            output_buffer: Vec::new(),
        }
    }

    // Read from serial registers
    pub fn read(&self, address: u16) -> u8 {
        match address {
            0xFF01 => self.sb,        // Reading SB returns 0xFF (no connection)
            0xFF02 => self.sc | 0x7E, // Bits 1-6 always set
            _ => 0xFF,
        }
    }

    // Write to serial registers
    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0xFF01 => self.sb = value,
            0xFF02 => {
                // Only bits 0 and 7 are writable
                self.sc = (value & 0x81) | 0x7E;

                // If Bit 7 (0x80) is set, a transfer is requested
                if (value & 0x80) != 0 {
                    // For Tetris: Just ignore serial transfers - don't complete them
                    // This prevents the game from getting stuck waiting for link cable

                    // Capture output for test ROMs that use serial for output
                    if self.sb != 0 && self.sb != 0x55 {
                        self.output_buffer.push(self.sb);
                    }

                    // DON'T complete the transfer - let bit 7 stay set
                    // DON'T set interrupt_pending
                    // Tetris will eventually give up and continue
                }
            }
            _ => {}
        }
    }

    // Clear the interrupt flag (called after interrupt is serviced)
    pub fn clear_interrupt(&mut self) {
        self.interrupt_pending = false;
    }

    // Get the latest output character (for test ROM output)
    pub fn get_output(&mut self) -> Option<u8> {
        if !self.output_buffer.is_empty() {
            Some(self.output_buffer.remove(0))
        } else {
            None
        }
    }

    // Get all output as a string
    pub fn get_output_string(&self) -> String {
        String::from_utf8_lossy(&self.output_buffer).to_string()
    }

    // Clear the output buffer
    pub fn clear_output(&mut self) {
        self.output_buffer.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_transfer() {
        let mut serial = Serial::new();

        // Write data to SB
        serial.write(0xFF01, 0x42);
        assert_eq!(serial.read(0xFF01), 0x42);

        // Start transfer by setting bit 7 of SC
        serial.write(0xFF02, 0x81);

        // Note: Transfer is NOT completed automatically to avoid Tetris link cable issues
        // SB remains unchanged, SC keeps bit 7 set, no interrupt is generated
        assert_eq!(serial.read(0xFF01), 0x42); // SB unchanged
        assert_eq!(serial.read(0xFF02) & 0x80, 0x80); // Transfer flag still set
        assert!(!serial.interrupt_pending); // No interrupt
        assert_eq!(serial.output_buffer.len(), 1); // But output is captured
        assert_eq!(serial.output_buffer[0], 0x42);
    }

    #[test]
    fn test_get_output() {
        let mut serial = Serial::new();

        serial.write(0xFF01, b'H');
        serial.write(0xFF02, 0x81);

        serial.write(0xFF01, b'i');
        serial.write(0xFF02, 0x81);

        // Output is captured even though transfers don't complete
        assert_eq!(serial.get_output(), Some(b'H'));
        assert_eq!(serial.get_output(), Some(b'i'));
        assert_eq!(serial.get_output(), None);
    }

    #[test]
    fn test_output_string() {
        let mut serial = Serial::new();

        for &byte in b"Hello" {
            serial.write(0xFF01, byte);
            serial.write(0xFF02, 0x81);
        }

        assert_eq!(serial.get_output_string(), "Hello");
    }
}
