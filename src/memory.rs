use crate::joypad::Joypad;
use crate::ppu::Ppu;
use crate::serial::Serial;
use crate::timer::Timer;
use std::ptr::null_mut;

type MainMemory = [u8; 0x10000];

type RawBankNumber = u8;

const BANK_MASK: u8 = 0b0001_1111;

pub struct Memory {
    pub main_memory: MainMemory,
    pub rom: Rom,
    rom_loaded: bool,
    pub(crate) current_rom_bank: u8,
    pub timer: Timer,
    pub serial: Serial,
    pub ppu: Ppu,
    pub joypad: Joypad,
    // OAM DMA state
    pub dma_active: bool,
    pub dma_cycles_remaining: u16,
    pub dma_source: u16,
    // When true, `write_8` will not trigger side-effects (used during init/reset)
    pub suppress_io_side_effects: bool,
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
            joypad: Joypad::new(),
            dma_active: false,
            dma_cycles_remaining: 0,
            dma_source: 0,
            suppress_io_side_effects: false,
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

impl Memory {
    pub fn read_8(&self, address: u16) -> u8 {
        if address == 0xFF00 {
            self.joypad.read()
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
            let bank = if self.current_rom_bank == 0 {
                1
            } else {
                self.current_rom_bank
            };
            let offset = (bank as usize) * 0x4000 + (address as usize - 0x4000);

            if offset < self.rom.buffer.len() {
                self.rom.buffer[offset]
            } else {
                0xFF // Return 0xFF for out-of-bounds ROM access
            }
        } else {
            self.main_memory[address as usize]
        }
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
        // If IO side effects are suppressed (e.g., during post-boot memcpy),
        // just write the byte to main memory and return without triggering
        // peripheral/PPU/serial logic.
        if self.suppress_io_side_effects {
            self.main_memory[address as usize] = value;
            return;
        }

        // OAM DMA trigger (write to 0xFF46)
        if address == 0xFF46 {
            let source = (value as u16) << 8;
            self.dma_active = true;
            // DMA takes 160 * 4 machine cycles on DMG (approx 640 cycles)
            self.dma_cycles_remaining = 160 * 4;
            self.dma_source = source;

            // Immediate copy of 160 bytes into OAM (FE00..FE9F)
            for i in 0..160u16 {
                let v = self.read_8(source + i);
                self.ppu.oam[i as usize] = v;
            }

            // Also write the value to IO register if code expects to read it
            self.main_memory[address as usize] = value;
            return;
        }

        if address == 0xFF00 {
            self.joypad.write(value);
            return;
        } else if (0xFF04..=0xFF07).contains(&address) {
            self.timer.write(address, value);
            return;
        } else if (0xFF01..=0xFF02).contains(&address) {
            self.serial.write(address, value);

            return;
        } else if (0xFF40..=0xFF4B).contains(&address) {
            self.ppu.write(address, value);
            return;
        } else if (0x8000..=0x9FFF).contains(&address) {
            // VRAM can only be written when LCD is off OR PPU is not in mode 3 (drawing)
            // Mode is stored in lower 2 bits of STAT register
            let ppu_mode = self.ppu.stat & 0x03;
            if self.dma_active || ppu_mode == 3 {
                // DMA or mode 3 active, maybe we should add the LCD off check later but fir now it works
                return;
            }

            self.ppu.vram[(address - 0x8000) as usize] = value;
            return;
        } else if (0xFE00..=0xFE9F).contains(&address) {
            if self.dma_active {
                return;
            }
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
        // Initialize IO registers from the canonical post-boot table
        // IO_RESET maps to 0xFF00..0xFFFF
        // Suppress IO side-effects while copying the canonical IO reset table
        // (this mirrors the behavior of a memcpy in the original C code)
        self.suppress_io_side_effects = true;
        for i in 0..0x100u16 {
            let addr = 0xFF00u16.wrapping_add(i);
            let value = IO_RESET[i as usize];
            // Directly copy into main memory while side-effects are suppressed
            self.write_8(addr, value);
        }
        self.suppress_io_side_effects = false;

        // Ensure the Joypad internal register reflects the copied IO_RESET value at 0xFF00
        let joypad_init = self.main_memory[0xFF00];
        self.joypad.set_register_raw(joypad_init);
 // Ensure boot-disable (FF50) is set to 1 to indicate boot ROM finished
        self.main_memory[0xFF50] = 0x01;
    }
}

// IO register post-boot defaults (maps to 0xFF00..0xFFFF)
static IO_RESET: [u8; 0x100] = [
    0xCF, 0x00, 0x7C, 0xFF, 0x00, 0x00, 0x00, 0xF8, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01,
    0x80, 0xBF, 0xF3, 0xFF, 0xBF, 0xFF, 0x3F, 0x00, 0xFF, 0xBF, 0x7F, 0xFF, 0x9F, 0xFF, 0xBF, 0xFF,
    0xFF, 0x00, 0x00, 0xBF, 0x77, 0xF3, 0xF1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF,
    0x91, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFC, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x7E, 0xFF, 0xFE,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x3E, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xC0, 0xFF, 0xC1, 0x00, 0xFE, 0xFF, 0xFF, 0xFF,
    0xF8, 0xFF, 0x00, 0x00, 0x00, 0x8F, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
    0x45, 0xEC, 0x52, 0xFA, 0x08, 0xB7, 0x07, 0x5D, 0x01, 0xFD, 0xC0, 0xFF, 0x08, 0xFC, 0x00, 0xE5,
    0x0B, 0xF8, 0xC2, 0xCE, 0xF4, 0xF9, 0x0F, 0x7F, 0x45, 0x6D, 0x3D, 0xFE, 0x46, 0x97, 0x33, 0x5E,
    0x08, 0xEF, 0xF1, 0xFF, 0x86, 0x83, 0x24, 0x74, 0x12, 0xFC, 0x00, 0x9F, 0xB4, 0xB7, 0x06, 0xD5,
    0xD0, 0x7A, 0x00, 0x9E, 0x04, 0x5F, 0x41, 0x2F, 0x1D, 0x77, 0x36, 0x75, 0x81, 0xAA, 0x70, 0x3A,
    0x98, 0xD1, 0x71, 0x02, 0x4D, 0x01, 0xC1, 0xFF, 0x0D, 0x00, 0xD3, 0x05, 0xF9, 0x00, 0x0B, 0x00,
];
