// Serial I/O implementation for Game Boy
// Registers:
// 0xFF01 - SB (Serial Transfer Data)
// 0xFF02 - SC (Serial Transfer Control)
//   Bit 7: Transfer Start Flag (1=Start, 0=None)
//   Bit 0: Shift Clock (1=Internal, 0=External)

pub struct Serial {
    sb: u8,  // Serial transfer data
    sc: u8,  // Serial transfer control
    pub interrupt_pending: bool,  // Serial interrupt flag
    pub output_buffer: Vec<u8>,  // Buffer for captured output
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
            0xFF01 => self.sb,
            0xFF02 => self.sc | 0x7E,  // Bits 1-6 always set
            _ => 0xFF,
        }
    }

    // Write to serial registers
    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0xFF01 => self.sb = value,
            0xFF02 => {
                self.sc = value & 0x81;  // Only bits 7 and 0 are used

                // Check if transfer is starting (bit 7 set)
                if value & 0x80 != 0 {
                    self.start_transfer();
                }
            }
            _ => {}
        }
    }

    // Start a serial transfer
    fn start_transfer(&mut self) {
        // In a real Game Boy, this would take 8 cycles per bit (8192 Hz)
        // For emulation purposes, we complete the transfer immediately

        // Store the output byte
        self.output_buffer.push(self.sb);

        // In real hardware, data would shift in from the other Game Boy
        // For test ROMs, we just receive 0xFF (no connection)
        self.sb = 0xFF;

        // Clear transfer start flag (bit 7)
        self.sc &= 0x7F;

        // Set interrupt flag
        self.interrupt_pending = true;
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

        // Transfer should complete immediately
        assert_eq!(serial.read(0xFF01), 0xFF);  // SB should be 0xFF (no connection)
        assert_eq!(serial.read(0xFF02) & 0x80, 0);  // Transfer flag should be clear
        assert!(serial.interrupt_pending);
        assert_eq!(serial.output_buffer.len(), 1);
        assert_eq!(serial.output_buffer[0], 0x42);
    }

    #[test]
    fn test_get_output() {
        let mut serial = Serial::new();

        serial.write(0xFF01, b'H');
        serial.write(0xFF02, 0x81);

        serial.write(0xFF01, b'i');
        serial.write(0xFF02, 0x81);

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

