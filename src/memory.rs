use std::ptr::null_mut;
use crate::timer::Timer;
use crate::serial::Serial;
use crate::ppu::Ppu;

type MainMemory = [u8; 0x10000];

type RawBankNumber = u8;

const BANK_MASK: u8 = 0b0001_1111;

type Bank = [u8; 0x4000];

pub struct Memory {
    pub main_memory: MainMemory,
    pub rom: Rom,
    rom_loaded: bool,
    pub(crate) current_rom_bank: u8,
    pub timer: Timer,
    pub serial: Serial,
    pub ppu: Ppu,
    pub joypad_state: u8,
}

impl Memory {
    pub fn new(rom_buffer: Vec<u8>) -> Memory {
        let mut memory = Memory {
            main_memory: [0; 0x10000],
            rom: Rom {
                buffer: [0; 0x2FFFF],
                bank: null_mut(),
            },
            rom_loaded: false,
            current_rom_bank: 1,
            timer: Timer::new(),
            serial: Serial::new(),
            ppu: Ppu::new(),
            joypad_state: 0xCF, // Initial joypad state
        };

        // Copy the ROM buffer into the memory's ROM
        let len = rom_buffer.len().min(memory.rom.buffer.len());
        memory.rom.buffer[..len].copy_from_slice(&rom_buffer[..len]);
        memory.rom_loaded = true;

        memory
    }
}

#[derive(Clone, Copy)]
pub struct Rom {
    pub buffer: [u8; 0x2FFFF],
    pub bank: *mut u8,
}

const RBN: u16 = 0x2000;

impl Memory {
    pub fn read_8(&self, address: u16) -> u8 {
        let value = if address == 0xFF00 {
            self.joypad_state
        } else if (0xFF04..=0xFF07).contains(&address) {
            self.timer.read(address)
        } else if (0xFF01..=0xFF02).contains(&address) {
            self.serial.read(address)
        } else if (0xFF40..=0xFF4B).contains(&address) {
            self.ppu.read(address)
        } else if (0x8000..=0x9FFF).contains(&address) {
            self.ppu.vram[(address - 0x8000) as usize]
        } else if (0xFE00..=0xFE9F).contains(&address) {
            self.ppu.oam[(address - 0xFE00) as usize]
        } else if self.rom_loaded && address < 0x4000 {
            self.rom.buffer[address as usize]
        } else if self.rom_loaded && (0x4000..0x8000).contains(&address) {
            let bank = if self.current_rom_bank == 0 { 1 } else { self.current_rom_bank };
            let offset = (bank as usize) * 0x4000 + (address as usize - 0x4000);

            // Debug ROM banking
            if address == 0x4000 || address == 0x7FFF {
                let debug_info = format!(
                    "ROM_BANK_ACCESS: addr=0x{:04X} bank={} offset=0x{:06X} value=0x{:02X}\n",
                    address, bank, offset, self.rom.buffer.get(offset).unwrap_or(&0xFF)
                );
                use std::fs::OpenOptions;
                use std::io::Write;
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("rom_banking.txt") {
                    let _ = file.write_all(debug_info.as_bytes());
                }
            }

            if offset < self.rom.buffer.len() {
                self.rom.buffer[offset]
            } else {
                0xFF // Return 0xFF for out-of-bounds ROM access
            }
        } else {
            self.main_memory[address as usize]
        };

        // Log suspicious memory accesses
        if address >= 0x8000 && address < 0xA000 {
            // VRAM access - log occasionally
            static mut VRAM_LOG_COUNTER: u32 = 0;
            unsafe {
                VRAM_LOG_COUNTER += 1;
                if VRAM_LOG_COUNTER % 100 == 0 {
                    let debug_info = format!("VRAM_READ: addr=0x{:04X} value=0x{:02X}\n", address, value);
                    use std::fs::OpenOptions;
                    use std::io::Write;
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("vram_access.txt") {
                        let _ = file.write_all(debug_info.as_bytes());
                    }
                }
            }
        }

        value
    }

    pub fn read_16(&self, address: u16) -> u16 {
        let x = self.read_8(address);
        let y = self.read_8(address + 1);
        (y as u16) << 8 | x as u16
    }

    fn write_to_rom_register(&mut self, address: u16, value: u8) {
        // Simple MBC1-style lower 5 bits bank select in 0x2000-3FFF
        if (0x2000..=0x3FFF).contains(&address) {
            let mut bank_number: RawBankNumber = value & BANK_MASK;
            if bank_number == 0 {
                bank_number = 1; // Bank 0 is remapped to 1
            }
            self.current_rom_bank = bank_number;
        }
    }

    pub fn write_8(&mut self, address: u16, value: u8) {
        // Log ROM banking writes
        if address >= 0x2000 && address <= 0x3FFF {
            let debug_info = format!(
                "ROM_BANK_SWITCH: addr=0x{:04X} value=0x{:02X} old_bank={} new_bank={}\n",
                address, value, self.current_rom_bank, value & BANK_MASK
            );
            use std::fs::OpenOptions;
            use std::io::Write;
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("rom_banking.txt") {
                let _ = file.write_all(debug_info.as_bytes());
            }
        }

        if address == 0xFF00 {
            // Only bits 4-5 are writable (button group select)
            self.joypad_state = (self.joypad_state & 0x0F) | (value & 0x30);
            return;
        } else if (0xFF04..=0xFF07).contains(&address) {
            self.timer.write(address, value);
            return;
        } else if (0xFF01..=0xFF02).contains(&address) {
            self.serial.write(address, value);
            return;
        } else if (0xFF40..=0xFF4B).contains(&address) {
            // Log LCD register writes
            let debug_info = format!("LCD_REG_WRITE: addr=0x{:04X} value=0x{:02X}\n", address, value);
            use std::fs::OpenOptions;
            use std::io::Write;
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("lcd_debug.txt") {
                let _ = file.write_all(debug_info.as_bytes());
            }
            self.ppu.write(address, value);
            return;
        } else if (0x8000..=0x9FFF).contains(&address) {
            // Debug VRAM writes - important for seeing when tile data is loaded
            static mut VRAM_WRITE_COUNT: u32 = 0;
            unsafe {
                VRAM_WRITE_COUNT += 1;
                if VRAM_WRITE_COUNT <= 10 || VRAM_WRITE_COUNT % 100 == 0 {
                    // println!("VRAM_WRITE #{}: addr=0x{:04X} value=0x{:02X}", VRAM_WRITE_COUNT, address, value);
                    if VRAM_WRITE_COUNT == 1 {
                        // println!("VRAM: First write detected! Game is loading graphics data.");
                    }
                }
            }
            self.ppu.vram[(address - 0x8000) as usize] = value;
            return;
        } else if (0xFE00..=0xFE9F).contains(&address) {
            self.ppu.oam[(address - 0xFE00) as usize] = value;
            return;
        } else if address < 0x8000 {
            // ROM writes (for ROM banking control)
            self.write_to_rom_register(address, value);
            return;
        }

        // Default: write to main memory
        self.main_memory[address as usize] = value;
    }

    pub fn write_16(&mut self, address: u16, value: u16) {
        self.write_8(address, (value & 0xFF) as u8);
        self.write_8(address.wrapping_add(1), (value >> 8) as u8);
    }

    pub fn set_rom(&mut self, rom: [u8; 0x2FFFF]) {
        self.rom.buffer = rom;
        self.rom_loaded = true;
        self.current_rom_bank = 1;
    }

    //initialize rom bank pointer after loading ROM
    pub fn init_rom_bank(&mut self) {
        // Kept for compatibility; just marks ROM loaded and resets bank
        self.rom_loaded = true;
        self.current_rom_bank = 1;
    }

    pub fn init_post_boot_state(&mut self) {
        // Timer registers
        self.write_8(0xFF05, 0x00); // TIMA
        self.write_8(0xFF06, 0x00); // TMA
        self.write_8(0xFF07, 0x00); // TAC
        self.write_8(0xFF04, 0x00); // DIV

        // Sound registers (initialize to common post-boot values)
        self.write_8(0xFF10, 0x80); // NR10
        self.write_8(0xFF11, 0xBF); // NR11
        self.write_8(0xFF12, 0xF3); // NR12
        self.write_8(0xFF14, 0xBF); // NR14
        self.write_8(0xFF16, 0x3F); // NR21
        self.write_8(0xFF17, 0x00); // NR22
        self.write_8(0xFF19, 0xBF); // NR24
        self.write_8(0xFF1A, 0x7F); // NR30
        self.write_8(0xFF1B, 0xFF); // NR31
        self.write_8(0xFF1C, 0x9F); // NR32
        self.write_8(0xFF1E, 0xBF); // NR34
        self.write_8(0xFF20, 0xFF); // NR41
        self.write_8(0xFF21, 0x00); // NR42
        self.write_8(0xFF22, 0x00); // NR43
        self.write_8(0xFF23, 0xBF); // NR44
        self.write_8(0xFF24, 0x77); // NR50
        self.write_8(0xFF25, 0xF3); // NR51
        self.write_8(0xFF26, 0xF1); // NR52 (sound on, all channels enabled)

        // LCD registers (DMG post-boot defaults)
        self.write_8(0xFF40, 0x91); // LCDC: LCD on, BG on, tiles from 0x8000, tilemap 0x9800
        self.write_8(0xFF41, 0x85); // STAT
        self.write_8(0xFF42, 0x00); // SCY
        self.write_8(0xFF43, 0x00); // SCX
        self.write_8(0xFF44, 0x00); // LY
        self.write_8(0xFF45, 0x00); // LYC
        self.write_8(0xFF47, 0xFC); // BGP (11 11 11 00 - darkest to lightest)
        self.write_8(0xFF48, 0xFF); // OBP0
        self.write_8(0xFF49, 0xFF); // OBP1
        self.write_8(0xFF4A, 0x00); // WY
        self.write_8(0xFF4B, 0x00); // WX

        // Interrupt flags / enable
        self.write_8(0xFF0F, 0xE1); // IF (boot ROM leaves VBlank flag set)
        self.write_8(0xFFFF, 0x00); // IE (interrupts disabled initially)

        // Serial transfer registers
        self.write_8(0xFF01, 0x00); // SB
        self.write_8(0xFF02, 0x7E); // SC

        // HRAM initialization - common patterns from real boot ROM
        // Many games poll specific HRAM addresses during initialization
        self.main_memory[0xFFFF] = 0x00; // IE (already written above via write_8)

        // Some games expect certain HRAM bytes to be non-zero after boot
        // This is a safe default that matches real DMG behavior
        self.main_memory[0xFF50] = 0x01; // Boot ROM disable register (boot finished)
    }
}
