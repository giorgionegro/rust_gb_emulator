use crate::memory::Memory;

pub static mut DEBUG_PC: u16 = 0;

// Enum for register operands - replaces string manipulation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg8 {
    A, B, C, D, E, H, L, F,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg16 {
    AF, BC, DE, HL, SP, PC,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    Reg8(Reg8),
    Reg16(Reg16),
    MemHL,  // (HL)
    MemBC,  // (BC)
    MemDE,  // (DE)
}

impl Reg8 {


}

impl Reg16 {
}

impl Operand {
    // Operand index: 0=B, 1=C, 2=D, 3=E, 4=H, 5=L, 6=(HL), 7=A
    fn from_index(idx: u8) -> Self {
        match idx {
            0 => Operand::Reg8(Reg8::B),
            1 => Operand::Reg8(Reg8::C),
            2 => Operand::Reg8(Reg8::D),
            3 => Operand::Reg8(Reg8::E),
            4 => Operand::Reg8(Reg8::H),
            5 => Operand::Reg8(Reg8::L),
            6 => Operand::MemHL,
            7 => Operand::Reg8(Reg8::A),
            _ => panic!("Invalid operand index: {}", idx),
        }
    }
}

pub struct Cpu {
    pub registers: Registers,
    pub cycles: u64,
    pub ei_pending: bool, // EI has 1-instruction delay
    pub halted: bool,     // CPU is halted waiting for interrupt
    pub halt_bug: bool, // HALT bug: PC doesn't increment after HALT when IME=0 and interrupt pending
}

#[derive(Clone, Copy)]
pub struct Registers {
    af: u16,
    bc: u16,
    de: u16,
    hl: u16,
    sp: u16,
    pc: u16,
    pub ime: u8,
}

impl Registers {
    // Enum-based read for 8-bit registers
    pub fn read_r8(&self, register: Reg8) -> u8 {
        match register {
            Reg8::A => (self.af >> 8) as u8,
            Reg8::B => (self.bc >> 8) as u8,
            Reg8::C => (self.bc & 0xFF) as u8,
            Reg8::D => (self.de >> 8) as u8,
            Reg8::E => (self.de & 0xFF) as u8,
            Reg8::H => (self.hl >> 8) as u8,
            Reg8::L => (self.hl & 0xFF) as u8,
            Reg8::F => (self.af & 0xFF) as u8,
        }
    }

    // Enum-based read for 16-bit registers
    pub fn read_r16(&self, register: Reg16) -> u16 {
        match register {
            Reg16::AF => self.af,
            Reg16::BC => self.bc,
            Reg16::DE => self.de,
            Reg16::HL => self.hl,
            Reg16::SP => self.sp,
            Reg16::PC => self.pc,
        }
    }

    // Enum-based write for 8-bit registers
    pub fn write_r8(&mut self, register: Reg8, value: u8) {
        match register {
            Reg8::A => self.af = (self.af & 0xFF) | ((value as u16) << 8),
            Reg8::B => self.bc = (self.bc & 0xFF) | ((value as u16) << 8),
            Reg8::C => self.bc = (self.bc & 0xFF00) | value as u16,
            Reg8::D => self.de = (self.de & 0xFF) | ((value as u16) << 8),
            Reg8::E => self.de = (self.de & 0xFF00) | value as u16,
            Reg8::H => self.hl = (self.hl & 0xFF) | ((value as u16) << 8),
            Reg8::L => self.hl = (self.hl & 0xFF00) | value as u16,
            Reg8::F => self.af = (self.af & 0xFF00) | ((value & 0xF0) as u16),
        }
    }

    // Enum-based write for 16-bit registers
    pub fn write_r16(&mut self, register: Reg16, value: u16) {
        match register {
            Reg16::AF => self.af = value & 0xFFF0,
            Reg16::BC => self.bc = value,
            Reg16::DE => self.de = value,
            Reg16::HL => self.hl = value,
            Reg16::SP => self.sp = value,
            Reg16::PC => self.pc = value,
        }
    }

    // Read IME (Interrupt Master Enable)
    pub fn read_ime(&self) -> u8 {
        self.ime
    }

    // Write IME (Interrupt Master Enable)
    pub fn write_ime(&mut self, value: u8) {
        self.ime = value;
    }
}

const OPCODE_DURATION: [u8; 256] = [
    // 0x00-0x0F
    4, 12, 8, 8, 4, 4, 8, 4, 20, 8, 8, 8, 4, 4, 8, 4, // 0x10-0x1F
    4, 12, 8, 8, 4, 4, 8, 4, 12, 8, 8, 8, 4, 4, 8, 4, // 0x20-0x2F
    12, 12, 8, 8, 4, 4, 8, 4, 12, 8, 8, 8, 4, 4, 8, 4, // 0x30-0x3F
    12, 12, 8, 8, 12, 12, 12, 4, 12, 8, 8, 8, 4, 4, 8, 4,
    // 0x40-0x4F (LD r,r = 4 cycles, LD r,(HL) = 8)
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // 0x50-0x5F
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // 0x60-0x6F
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4,
    // 0x70-0x77 (LD (HL),r = 8 cycles EXCEPT 0x76 HALT = 4)
    8, 8, 8, 8, 8, 8, 4, 8, // 0x78-0x7F
    4, 4, 4, 4, 4, 4, 8, 4, // 0x80-0x8F (ALU r = 4, ALU (HL) = 8)
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // 0x90-0x9F
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // 0xA0-0xAF
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4, // 0xB0-0xBF
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4,
    // 0xC0-0xCF (RET cc = 20/8, RET = 16, JP cc = 16/12, JP = 16, CALL cc = 24/12, CALL = 24)
    20, 12, 16, 16, 24, 16, 8, 16, 20, 16, 16, 4, 24, 24, 8, 16, // 0xD0-0xDF
    20, 12, 16, 0, 24, 16, 8, 16, 20, 16, 16, 0, 24, 0, 8, 16, // 0xE0-0xEF
    12, 12, 8, 0, 0, 16, 8, 16, 16, 4, 16, 0, 0, 0, 8, 16, // 0xF0-0xFF
    12, 12, 8, 4, 0, 16, 8, 16, 12, 8, 16, 4, 0, 0, 8, 16,
];
const OPCODE_DURATION_CB: [u8; 256] = [
    8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8,
    16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8,
    8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8,
    8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8,
    8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8,
    16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8,
    8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8,
    8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8,
    8, 8, 8, 8, 8, 8, 16, 8, 8, 8, 8, 8, 8, 8, 16, 8,
];
const OPCODE_LENGTHS: [u8; 256] = [
    // 0x00-0x0F
    1, 3, 1, 1, 1, 1, 2, 1, 3, 1, 1, 1, 1, 1, 2, 1, // 0x10-0x1F (0x10 STOP is 2 bytes)
    2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1, // 0x20-0x2F
    2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1, // 0x30-0x3F
    2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1,
    // 0x40-0x7F (LD r,r and HALT - all 1 byte)
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    // 0x80-0xBF (ALU operations - all 1 byte)
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    // 0xC0-0xCF
    1, 1, 3, 3, 3, 1, 2, 1, 1, 1, 3, 2, 3, 3, 2, 1,
    // 0xD0-0xDF (0xD3, 0xDB, 0xDD are invalid = 0)
    1, 1, 3, 0, 3, 1, 2, 1, 1, 1, 3, 0, 3, 0, 2, 1,
    // 0xE0-0xEF (0xE3, 0xE4, 0xEB, 0xEC, 0xED are invalid = 0)
    2, 1, 1, 0, 0, 1, 2, 1, 2, 1, 3, 0, 0, 0, 2, 1,
    // 0xF0-0xFF (0xF4, 0xFC, 0xFD are invalid = 0)
    2, 1, 1, 1, 0, 1, 2, 1, 2, 1, 3, 1, 0, 0, 2, 1,
];

const ZERO_FLAG: u8 = 0b10000000;
const SUBTRACT_FLAG: u8 = 0b01000000;
const HALF_CARRY_FLAG: u8 = 0b00100000;
const CARRY_FLAG: u8 = 0b00010000;

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu {
            registers: Registers {
                af: 0,
                bc: 0,
                de: 0,
                hl: 0,
                sp: 0,
                pc: 0,
                ime: 0,
            },
            cycles: 0,
            ei_pending: false,
            halted: false,
            halt_bug: false,
        }
    }

    /// Execute one instruction and return cycles taken
    pub fn step(&mut self, mem: &mut Memory) -> u32 {
        // If CPU is halted, check if we should exit halt
        if self.halted {
            // Check if any interrupt is pending (regardless of IME)
            let ie = mem.read_8(0xFFFF);
            let if_reg = mem.read_8(0xFF0F);
            if (ie & if_reg & 0x1F) != 0 {
                // Exit halt state
                self.halted = false;
            } else {
                // Still halted, consume 4 cycles and return
                return 4;
            }
        }

        let pc = self.registers.read_r16(Reg16::PC);

        let opcode = mem.read_8(pc);


        self.execute(opcode, mem);
        let cycles = self.handle_post_instruction(mem, opcode, 0);

        // Handle EI delay - if EI was executed, enable interrupts AFTER this instruction
        if self.ei_pending {
            self.registers.write_ime(1);
            self.ei_pending = false;
        }

        cycles
    }

    fn ld_r16_nn(&mut self, mem: &mut Memory, reg: Reg16) {
        let value = mem.read_16(self.registers.read_r16(Reg16::PC) + 1);
        self.registers.write_r16(reg, value);
    }

    fn ld_r8_n(&mut self, mem: &mut Memory, reg: Reg8) {
        let value = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        self.registers.write_r8(reg, value);
    }

    fn ld_operand(&mut self, mem: &mut Memory, dest: Operand, source: Operand) {
        let value = self.read_operand(mem, source);
        self.write_operand(mem, dest, value);
    }

    fn ld_nn_a(&mut self, mem: &mut Memory) {
        let value = self.registers.read_r8(Reg8::A);
        mem.write_8(mem.read_16(self.registers.read_r16(Reg16::PC) + 1), value);
    }

    fn ld_m_n(&mut self, mem: &mut Memory) {
        let value = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        mem.write_8(self.registers.read_r16(Reg16::HL), value);
    }

    fn ld_sp_e(&mut self, mem: &mut Memory) {
        // Opcode 0xF8: LD HL, SP+e - Load SP + signed offset into HL
        let offset = mem.read_8(self.registers.read_r16(Reg16::PC) + 1) as i8;
        let sp = self.registers.read_r16(Reg16::SP);
        let result = sp.wrapping_add(offset as i16 as u16);
        self.registers.write_r16(Reg16::HL, result);

        // Flags are calculated on UNSIGNED byte addition (low byte of SP + offset as unsigned byte)
        // Z=0, N=0, H=carry from bit 3, C=carry from bit 7
        let sp_low = (sp & 0xFF) as u8;
        let offset_u8 = offset as u8;

        let mut flags = 0;
        // Half carry: carry from bit 3
        if ((sp_low & 0xF) + (offset_u8 & 0xF)) > 0xF {
            flags |= HALF_CARRY_FLAG;
        }
        // Carry: carry from bit 7
        if (sp_low as u16 + offset_u8 as u16) > 0xFF {
            flags |= CARRY_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn ld_sp_hl(&mut self, _mem: &mut Memory) {
        // Opcode 0xF9: LD SP, HL - Copy HL to SP
        let value = self.registers.read_r16(Reg16::HL);
        self.registers.write_r16(Reg16::SP, value);
    }

    fn ld_nn_sp(&mut self, mem: &mut Memory) {
        // Opcode 0x08: LD (nn), SP - Store SP at memory address nn
        let addr = mem.read_16(self.registers.read_r16(Reg16::PC) + 1);
        let sp = self.registers.read_r16(Reg16::SP);
        mem.write_16(addr, sp);
    }

    fn ldh_n_a(&mut self, mem: &mut Memory) {
        let value = self.registers.read_r8(Reg8::A);
        mem.write_8(
            0xFF00 + mem.read_8(self.registers.read_r16(Reg16::PC) + 1) as u16,
            value,
        );
    }

    fn ldh_a_n(&mut self, mem: &mut Memory) {
        let offset = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        let value = mem.read_8(0xFF00 + offset as u16);
        self.registers.write_r8(Reg8::A, value);
    }

    fn ldh_c_a(&mut self, mem: &mut Memory) {
        let value = self.registers.read_r8(Reg8::A);
        mem.write_8(0xFF00 + self.registers.read_r8(Reg8::C) as u16, value);
    }

    fn ldh_a_c(&mut self, mem: &mut Memory) {
        let value = mem.read_8(0xFF00 + (self.registers.read_r8(Reg8::C) as u16));
        self.registers.write_r8(Reg8::A, value);
    }

    fn pop(&mut self, mem: &mut Memory, reg: Reg16) {
        let value = mem.read_16(self.registers.read_r16(Reg16::SP));
        self.registers.write_r16(reg, value);
        let sp = self.registers.read_r16(Reg16::SP);
        self.registers.write_r16(Reg16::SP, sp + 2);
    }

    fn push(&mut self, mem: &mut Memory, reg: Reg16) {
        let value = self.registers.read_r16(reg);
        let sp = self.registers.read_r16(Reg16::SP);
        self.registers.write_r16(Reg16::SP, sp - 2);
        mem.write_16(self.registers.read_r16(Reg16::SP), value);
    }

    fn inc_r8(&mut self, reg: Reg8) {
        let value = self.registers.read_r8(reg);
        let result = value.wrapping_add(1);
        self.registers.write_r8(reg, result);

        let mut flags = self.registers.read_r8(Reg8::F);
        flags &= !(ZERO_FLAG | SUBTRACT_FLAG | HALF_CARRY_FLAG);
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        if (value & 0x0F) == 0x0F {
            flags |= HALF_CARRY_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn inc_r16(&mut self, reg: Reg16) {
        let value = self.registers.read_r16(reg);
        self.registers.write_r16(reg, value.wrapping_add(1));
    }

    fn dec_r8(&mut self, reg: Reg8) {
        let value = self.registers.read_r8(reg);
        let result = value.wrapping_sub(1);
        self.registers.write_r8(reg, result);

        let mut flags = self.registers.read_r8(Reg8::F);
        flags &= !(ZERO_FLAG | HALF_CARRY_FLAG);
        flags |= SUBTRACT_FLAG;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        if (value & 0x0F) == 0 {
            flags |= HALF_CARRY_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn dec_r16(&mut self, reg: Reg16) {
        let value = self.registers.read_r16(reg);
        self.registers.write_r16(reg, value.wrapping_sub(1));
    }

    fn inc_mem(&mut self, mem: &mut Memory, reg: Reg16) {
        let addr = self.registers.read_r16(reg);
        let value = mem.read_8(addr);
        let result = value.wrapping_add(1);
        mem.write_8(addr, result);

        let mut flags = self.registers.read_r8(Reg8::F);
        flags &= !(ZERO_FLAG | SUBTRACT_FLAG | HALF_CARRY_FLAG);
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        if (value & 0x0F) == 0x0F {
            flags |= HALF_CARRY_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn dec_mem(&mut self, mem: &mut Memory, reg: Reg16) {
        let addr = self.registers.read_r16(reg);
        let value = mem.read_8(addr);
        let result = value.wrapping_sub(1);
        mem.write_8(addr, result);

        let mut flags = self.registers.read_r8(Reg8::F);
        flags &= !(ZERO_FLAG | HALF_CARRY_FLAG);
        flags |= SUBTRACT_FLAG;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        if (value & 0x0F) == 0 {
            flags |= HALF_CARRY_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn rlca(&mut self) {
        let value = self.registers.read_r8(Reg8::A);
        let msb = value & 0x80;
        let new_value = (value << 1) | (msb >> 7);
        self.registers.write_r8(Reg8::A, new_value);
        let mut flags = self.registers.read_r8(Reg8::F);
        flags &= !(HALF_CARRY_FLAG | SUBTRACT_FLAG | ZERO_FLAG);
        if msb != 0 {
            flags |= CARRY_FLAG;
        } else {
            flags &= !CARRY_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn rla(&mut self) {
        let value = self.registers.read_r8(Reg8::A);
        let msb = value & 0x80;
        let new_value = (value << 1) | ((self.registers.read_r8(Reg8::F) & CARRY_FLAG) >> 4);
        self.registers.write_r8(Reg8::A, new_value);
        let mut flags = self.registers.read_r8(Reg8::F);
        flags &= !(HALF_CARRY_FLAG | SUBTRACT_FLAG | ZERO_FLAG | CARRY_FLAG);
        if msb != 0 {
            flags |= CARRY_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn rrca(&mut self) {
        let value = self.registers.read_r8(Reg8::A);
        let lsb = value & 0x01;
        let new_value = (value >> 1) | (lsb << 7);
        self.registers.write_r8(Reg8::A, new_value);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !(HALF_CARRY_FLAG | SUBTRACT_FLAG | ZERO_FLAG | CARRY_FLAG);

        if lsb != 0 {
            flags |= CARRY_FLAG;
        }

        self.registers.write_r8(Reg8::F, flags);
    }

    fn rra(&mut self, _mem: &mut Memory) {
        let value = self.registers.read_r8(Reg8::A);
        let lsb = value & 0x01;
        let new_value = (value >> 1) | ((self.registers.read_r8(Reg8::F) & CARRY_FLAG) << 3);
        self.registers.write_r8(Reg8::A, new_value);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !(HALF_CARRY_FLAG | SUBTRACT_FLAG | ZERO_FLAG | CARRY_FLAG);

        if lsb != 0 {
            flags |= CARRY_FLAG;
        }

        self.registers.write_r8(Reg8::F, flags);
    }

    //arithmetic and logic
    fn add_hl(&mut self, reg: Reg16) {
        let value = self.registers.read_r16(reg);
        let hl = self.registers.read_r16(Reg16::HL);
        let result: u32 = value as u32 + hl as u32;
        self.registers.write_r16(Reg16::HL, result as u16);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !(SUBTRACT_FLAG | CARRY_FLAG | HALF_CARRY_FLAG);

        if result > 0xFFFF {
            flags |= CARRY_FLAG;
        }

        if (value & 0xFFF) + (hl & 0xFFF) > 0xFFF {
            flags |= HALF_CARRY_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn add_a_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let a = self.registers.read_r8(Reg8::A);
        let result: u16 = value as u16 + a as u16;
        self.registers.write_r8(Reg8::A, result as u8);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !(SUBTRACT_FLAG | CARRY_FLAG | HALF_CARRY_FLAG | ZERO_FLAG);

        if result > 0xFF {
            flags |= CARRY_FLAG;
        }

        if (value & 0xF) + (a & 0xF) > 0xF {
            flags |= HALF_CARRY_FLAG;
        }

        if result as u8 == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn add_a_n(&mut self, mem: &mut Memory) {
        let value = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        let a = self.registers.read_r8(Reg8::A);
        let result: u16 = value as u16 + a as u16;
        self.registers.write_r8(Reg8::A, result as u8);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !(SUBTRACT_FLAG | CARRY_FLAG | HALF_CARRY_FLAG | ZERO_FLAG);

        if result > 0xFF {
            flags |= CARRY_FLAG;
        }

        if (value & 0xF) + (a & 0xF) > 0xF {
            flags |= HALF_CARRY_FLAG;
        }

        if result as u8 == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn add_sp_e(&mut self, mem: &mut Memory) {
        // Opcode 0xE8: ADD SP, e - Add signed offset to SP
        let offset = mem.read_8(self.registers.read_r16(Reg16::PC) + 1) as i8;
        let sp = self.registers.read_r16(Reg16::SP);
        let result = sp.wrapping_add(offset as i16 as u16);
        self.registers.write_r16(Reg16::SP, result);

        // Flags are calculated on UNSIGNED byte addition (low byte of SP + offset as unsigned byte)
        // Z=0, N=0, H=carry from bit 3, C=carry from bit 7
        let sp_low = (sp & 0xFF) as u8;
        let offset_u8 = offset as u8;

        let mut flags = 0;
        // Half carry: carry from bit 3
        if ((sp_low & 0xF) + (offset_u8 & 0xF)) > 0xF {
            flags |= HALF_CARRY_FLAG;
        }
        // Carry: carry from bit 7
        if (sp_low as u16 + offset_u8 as u16) > 0xFF {
            flags |= CARRY_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn adc_a_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let a = self.registers.read_r8(Reg8::A);
        let carry = (self.registers.read_r8(Reg8::F) & CARRY_FLAG) >> 4;

        let result: u16 = value as u16 + a as u16 + carry as u16;
        self.registers.write_r8(Reg8::A, result as u8);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !(SUBTRACT_FLAG | CARRY_FLAG | HALF_CARRY_FLAG | ZERO_FLAG);

        if result > 0xFF {
            flags |= CARRY_FLAG;
        }

        if (value & 0xF) + (a & 0xF) + carry > 0xF {
            flags |= HALF_CARRY_FLAG;
        }

        if result as u8 == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn adc_a_n(&mut self, mem: &mut Memory) {
        let value = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        let a = self.registers.read_r8(Reg8::A);
        let carry = (self.registers.read_r8(Reg8::F) & CARRY_FLAG) >> 4;

        let result: u16 = value as u16 + a as u16 + carry as u16;
        self.registers.write_r8(Reg8::A, result as u8);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !(SUBTRACT_FLAG | CARRY_FLAG | HALF_CARRY_FLAG | ZERO_FLAG);

        if result > 0xFF {
            flags |= CARRY_FLAG;
        }

        if (value & 0xF) + (a & 0xF) + carry > 0xF {
            flags |= HALF_CARRY_FLAG;
        }

        if result & 0xFF == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn sub_a_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let a = self.registers.read_r8(Reg8::A);
        let result = a.wrapping_sub(value);
        self.registers.write_r8(Reg8::A, result);

        // Clear all flags first, then set as needed
        let mut flags = 0;

        // Always set subtract flag for SUB instruction
        flags |= SUBTRACT_FLAG;

        // Set carry flag if A < value (subtraction would underflow)
        if a < value {
            flags |= CARRY_FLAG;
        }

        // Set half-carry flag if lower nibble of A < lower nibble of value
        if (a & 0x0F) < (value & 0x0F) {
            flags |= HALF_CARRY_FLAG;
        }

        // Set zero flag if result is 0
        if result == 0 {
            flags |= ZERO_FLAG;
        }

        self.registers.write_r8(Reg8::F, flags);
    }

    fn sub_a_n(&mut self, mem: &mut Memory) {
        let value = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        let a = self.registers.read_r8(Reg8::A);
        let result = a.wrapping_sub(value);
        self.registers.write_r8(Reg8::A, result);

        // Clear all flags first, then set as needed
        let mut flags = 0;

        // Always set subtract flag for SUB instruction
        flags |= SUBTRACT_FLAG;

        // Set carry flag if A < value (subtraction would underflow)
        if a < value {
            flags |= CARRY_FLAG;
        }

        // Set half-carry flag if lower nibble of A < lower nibble of value
        if (a & 0x0F) < (value & 0x0F) {
            flags |= HALF_CARRY_FLAG;
        }

        // Set zero flag if result is 0
        if result == 0 {
            flags |= ZERO_FLAG;
        }

        self.registers.write_r8(Reg8::F, flags);
    }

    fn sbc_a_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let a = self.registers.read_r8(Reg8::A);
        let carry_in = if (self.registers.read_r8(Reg8::F) & CARRY_FLAG) != 0 {
            1u8
        } else {
            0u8
        };

        let temp = a as u16;
        let temp_result = temp
            .wrapping_sub(value as u16)
            .wrapping_sub(carry_in as u16);
        let result = temp_result as u8;
        self.registers.write_r8(Reg8::A, result);

        // Clear all flags first, then set as needed
        let mut flags = 0;

        // Always set subtract flag for SBC instruction
        flags |= SUBTRACT_FLAG;

        // Set carry flag if subtraction underflows
        if temp_result > 0xFF {
            flags |= CARRY_FLAG;
        }

        // Set half-carry flag if lower nibble underflows
        if (a & 0x0F) < (value & 0x0F) + carry_in {
            flags |= HALF_CARRY_FLAG;
        }

        // Set zero flag if result is 0
        if result == 0 {
            flags |= ZERO_FLAG;
        }

        self.registers.write_r8(Reg8::F, flags);
    }

    fn sbc_a_n(&mut self, mem: &mut Memory) {
        let value = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        let a = self.registers.read_r8(Reg8::A);
        let carry_in = if (self.registers.read_r8(Reg8::F) & CARRY_FLAG) != 0 {
            1u8
        } else {
            0u8
        };

        let temp = a as u16;
        let temp_result = temp
            .wrapping_sub(value as u16)
            .wrapping_sub(carry_in as u16);
        let result = temp_result as u8;
        self.registers.write_r8(Reg8::A, result);

        // Clear all flags first, then set as needed
        let mut flags = 0;

        // Always set subtract flag for SBC instruction
        flags |= SUBTRACT_FLAG;

        // Set carry flag if subtraction underflows
        if temp_result > 0xFF {
            flags |= CARRY_FLAG;
        }

        // Set half-carry flag if lower nibble underflows
        if (a & 0x0F) < (value & 0x0F) + carry_in {
            flags |= HALF_CARRY_FLAG;
        }

        // Set zero flag if result is 0
        if result == 0 {
            flags |= ZERO_FLAG;
        }

        self.registers.write_r8(Reg8::F, flags);
    }

    fn daa(&mut self, _mem: &mut Memory) {
        let mut value = self.registers.read_r8(Reg8::A);
        let flags = self.registers.read_r8(Reg8::F);
        let mut new_flags = flags;

        let mut adjust = 0u8;
        let mut set_carry = false;

        if (flags & SUBTRACT_FLAG) == 0 {
            // After addition
            if (flags & CARRY_FLAG) != 0 || value > 0x99 {
                adjust |= 0x60;
                set_carry = true;
            }
            if (flags & HALF_CARRY_FLAG) != 0 || (value & 0x0F) > 0x09 {
                adjust |= 0x06;
            }
            value = value.wrapping_add(adjust);
        } else {
            // After subtraction
            if (flags & CARRY_FLAG) != 0 {
                adjust |= 0x60;
                set_carry = true;
            }
            if (flags & HALF_CARRY_FLAG) != 0 {
                adjust |= 0x06;
            }
            value = value.wrapping_sub(adjust);
        }

        // Update flags
        new_flags &= !(ZERO_FLAG | HALF_CARRY_FLAG | CARRY_FLAG);

        if value == 0 {
            new_flags |= ZERO_FLAG;
        }
        if set_carry {
            new_flags |= CARRY_FLAG;
        }
        // N flag unchanged, H flag cleared

        self.registers.write_r8(Reg8::F, new_flags);
        self.registers.write_r8(Reg8::A, value);
    }

    fn cpl(&mut self) {
        let value = self.registers.read_r8(Reg8::A);
        self.registers.write_r8(Reg8::A, !value);
        let mut flags = self.registers.read_r8(Reg8::F);
        flags |= SUBTRACT_FLAG | HALF_CARRY_FLAG;
        self.registers.write_r8(Reg8::F, flags);
    }

    fn and_a_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let a = self.registers.read_r8(Reg8::A);
        let result = a & value;
        self.registers.write_r8(Reg8::A, result);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags |= HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !CARRY_FLAG;

        flags &= !ZERO_FLAG;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn and_a_n(&mut self, mem: &mut Memory) {
        let value = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        let a = self.registers.read_r8(Reg8::A);
        let result = a & value;
        self.registers.write_r8(Reg8::A, result);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags |= HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !CARRY_FLAG;

        flags &= !ZERO_FLAG;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn xor_a_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let a = self.registers.read_r8(Reg8::A);
        let result = a ^ value;
        self.registers.write_r8(Reg8::A, result);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !CARRY_FLAG;

        flags &= !ZERO_FLAG;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn xor_a_n(&mut self, mem: &mut Memory) {
        let value = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        let a = self.registers.read_r8(Reg8::A);
        let result = a ^ value;
        self.registers.write_r8(Reg8::A, result);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !CARRY_FLAG;

        flags &= !ZERO_FLAG;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn or_a_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let a = self.registers.read_r8(Reg8::A);
        let result = a | value;
        self.registers.write_r8(Reg8::A, result);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !CARRY_FLAG;

        flags &= !ZERO_FLAG;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn or_a_n(&mut self, mem: &mut Memory) {
        let value = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        let a = self.registers.read_r8(Reg8::A);
        let result = a | value;
        self.registers.write_r8(Reg8::A, result);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !HALF_CARRY_FLAG;
        flags &= !SUBTRACT_FLAG;

        flags &= !CARRY_FLAG;

        flags &= !ZERO_FLAG;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn cp_a_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let a = self.registers.read_r8(Reg8::A);

        // Clear all flags first, then set as needed
        let mut flags = 0;

        // Always set subtract flag for CP instruction
        flags |= SUBTRACT_FLAG;

        // Set carry flag if A < value (subtraction would underflow)
        if a < value {
            flags |= CARRY_FLAG;
        }

        // Set half-carry flag if lower nibble of A < lower nibble of value
        if (a & 0x0F) < (value & 0x0F) {
            flags |= HALF_CARRY_FLAG;
        }

        // Set zero flag if A == value
        if a == value {
            flags |= ZERO_FLAG;
        }

        self.registers.write_r8(Reg8::F, flags);
    }

    pub fn cp_a_n(&mut self, mem: &mut Memory) {
        let value = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        let a = self.registers.read_r8(Reg8::A);

        // Clear all flags first, then set as needed
        let mut flags = 0;

        // Always set subtract flag for CP instruction
        flags |= SUBTRACT_FLAG;

        // Set carry flag if A < value (subtraction would underflow)
        if a < value {
            flags |= CARRY_FLAG;
        }

        // Set half-carry flag if lower nibble of A < lower nibble of value
        if (a & 0x0F) < (value & 0x0F) {
            flags |= HALF_CARRY_FLAG;
        }

        // Set zero flag if A == value
        if a == value {
            flags |= ZERO_FLAG;
        }

        self.registers.write_r8(Reg8::F, flags);
    }
    //utils

    // Read an operand value (register or memory)
    fn read_operand(&self, mem: &Memory, op: Operand) -> u8 {
        match op {
            Operand::Reg8(reg) => self.registers.read_r8(reg),
            Operand::MemHL => mem.read_8(self.registers.read_r16(Reg16::HL)),
            Operand::MemBC => mem.read_8(self.registers.read_r16(Reg16::BC)),
            Operand::MemDE => mem.read_8(self.registers.read_r16(Reg16::DE)),
            _ => panic!("Invalid operand for 8-bit read: {:?}", op),
        }
    }

    // Write an operand value (register or memory)
    fn write_operand(&mut self, mem: &mut Memory, op: Operand, value: u8) {
        match op {
            Operand::Reg8(reg) => self.registers.write_r8(reg, value),
            Operand::MemHL => mem.write_8(self.registers.read_r16(Reg16::HL), value),
            Operand::MemBC => mem.write_8(self.registers.read_r16(Reg16::BC), value),
            Operand::MemDE => mem.write_8(self.registers.read_r16(Reg16::DE), value),
            _ => panic!("Invalid operand for 8-bit write: {:?}", op),
        }
    }


    //misc
    fn nop(&mut self) {}

    fn stop(&mut self) {
        //stop Cpu until button pressed
    }

    fn halt(&mut self, mem: &Memory) {
        // HALT: Stop CPU until interrupt occurs
        // HALT bug: If IME=0 and an interrupt is pending, don't halt
        // but set halt_bug flag to prevent PC increment after next instruction
        let ie = mem.read_8(0xFFFF);
        let if_reg = mem.read_8(0xFF0F);
        let interrupt_pending = (ie & if_reg & 0x1F) != 0;

        if self.registers.read_ime() == 0 && interrupt_pending {
            // HALT bug: don't halt, but next instruction won't increment PC
            self.halt_bug = true;
        } else {
            // Normal HALT behavior
            self.halted = true;
        }
    }

    fn scf(&mut self) {
        let mut flags = self.registers.read_r8(Reg8::F);
        flags |= CARRY_FLAG;
        flags &= !HALF_CARRY_FLAG;
        flags &= !SUBTRACT_FLAG;
        self.registers.write_r8(Reg8::F, flags);
    }

    fn ccf(&mut self) {
        let mut flags = self.registers.read_r8(Reg8::F);
        flags ^= CARRY_FLAG;
        flags &= !HALF_CARRY_FLAG;
        flags &= !SUBTRACT_FLAG;
        self.registers.write_r8(Reg8::F, flags);
    }

    //cb instructions
    fn rlc_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !CARRY_FLAG;
        if value & 0x80 != 0 {
            flags |= CARRY_FLAG;
        }

        flags &= !HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !ZERO_FLAG;
        let result = value.rotate_left(1);
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
        self.write_operand(mem, op, result);
    }

    fn rrc_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !CARRY_FLAG;
        if value & 0x01 != 0 {
            flags |= CARRY_FLAG;
        }

        flags &= !HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !ZERO_FLAG;
        let result = value.rotate_right(1);
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
        self.write_operand(mem, op, result);
    }

    fn rl_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let mut flags = self.registers.read_r8(Reg8::F);

        let mut carry = 0;
        if flags & CARRY_FLAG != 0 {
            carry = 1;
        }
        flags &= !CARRY_FLAG;
        if value & 0x80 != 0 {
            flags |= CARRY_FLAG;
        }

        flags &= !HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !ZERO_FLAG;
        let result = (value << 1) | carry;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
        self.write_operand(mem, op, result);
    }

    fn rr_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let mut flags = self.registers.read_r8(Reg8::F);

        let mut carry = 0;
        if flags & CARRY_FLAG != 0 {
            carry = 1;
        }
        flags &= !CARRY_FLAG;
        if value & 0x01 != 0 {
            flags |= CARRY_FLAG;
        }

        flags &= !HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !ZERO_FLAG;
        let result = (value >> 1) | (carry << 7);
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
        self.write_operand(mem, op, result);
    }

    fn sla_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op) as i8;
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !CARRY_FLAG;
        if value as u8 & 0x80 != 0 {
            flags |= CARRY_FLAG;
        }

        flags &= !HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !ZERO_FLAG;
        let result = value << 1;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
        self.write_operand(mem, op, result as u8);
    }

    fn sra_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op) as i8;
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !CARRY_FLAG;
        if value & 0x01 != 0 {
            flags |= CARRY_FLAG;
        }

        flags &= !HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !ZERO_FLAG;
        let result = value >> 1;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
        self.write_operand(mem, op, result as u8);
    }

    fn swap_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !(CARRY_FLAG | HALF_CARRY_FLAG | SUBTRACT_FLAG | ZERO_FLAG);

        let result = value.rotate_left(4);
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
        self.write_operand(mem, op, result);
    }

    fn bit_n_r(&mut self, mem: &mut Memory, op: Operand, n: u8) {
        let value = self.read_operand(mem, op);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags |= HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !ZERO_FLAG;
        if value & (1 << n) == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
    }

    fn srl_r(&mut self, mem: &mut Memory, op: Operand) {
        let value = self.read_operand(mem, op);
        let mut flags = self.registers.read_r8(Reg8::F);

        flags &= !CARRY_FLAG;
        if value & 0x01 != 0 {
            flags |= CARRY_FLAG;
        }

        flags &= !HALF_CARRY_FLAG;

        flags &= !SUBTRACT_FLAG;

        flags &= !ZERO_FLAG;
        let result = value >> 1;
        if result == 0 {
            flags |= ZERO_FLAG;
        }
        self.registers.write_r8(Reg8::F, flags);
        self.write_operand(mem, op, result);
    }

    fn res_n_r(&mut self, mem: &mut Memory, op: Operand, n: u8) {
        let value = self.read_operand(mem, op);
        let result = value & !(1 << n);
        self.write_operand(mem, op, result);
    }

    fn set_n_r(&mut self, mem: &mut Memory, op: Operand, n: u8) {
        let value = self.read_operand(mem, op);
        let result = value | (1 << n);
        self.write_operand(mem, op, result);
    }

    fn call_cb(&mut self, mem: &mut Memory) {
        let cb_opcode = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
        let op = Operand::from_index(cb_opcode & 0x07);

        match cb_opcode {
            0x00..=0x07 => self.rlc_r(mem, op),
            0x08..=0x0F => self.rrc_r(mem, op),
            0x10..=0x17 => self.rl_r(mem, op),
            0x18..=0x1F => self.rr_r(mem, op),
            0x20..=0x27 => self.sla_r(mem, op),
            0x28..=0x2F => self.sra_r(mem, op),
            0x30..=0x37 => self.swap_r(mem, op),
            0x38..=0x3F => self.srl_r(mem, op),
            0x40..=0x7F => {
                let n = (cb_opcode >> 3) & 0x07;
                self.bit_n_r(mem, op, n);
            }
            0x80..=0xBF => {
                let n = (cb_opcode >> 3) & 0x07;
                self.res_n_r(mem, op, n);
            }
            0xC0..=0xFF => {
                let n = (cb_opcode >> 3) & 0x07;
                self.set_n_r(mem, op, n);
            }
        }
    }

    fn di(&mut self) {
        self.registers.write_ime(0);
        self.ei_pending = false; // Cancel any pending EI
    }

    fn ei(&mut self) {
        // EI enables interrupts after the NEXT instruction executes
        self.ei_pending = true;
    }

    //flow
    fn jr_e(&mut self, mem: &mut Memory) {
        let offset = mem.read_8(self.registers.read_r16(Reg16::PC) + 1) as i8;
        let pc = self.registers.read_r16(Reg16::PC);
        // Jump relative to PC+2 (after the JR instruction which is 2 bytes)
        let target = (pc as i32 + 2 + offset as i32) as u16;
        self.registers.write_r16(Reg16::PC, target);
    }

    fn jr_f_e(&mut self, mem: &mut Memory, cflag: char, z: bool) {
        let flag = match cflag {
            'c' => CARRY_FLAG,
            'z' => ZERO_FLAG,
            _ => panic!("Invalid flag"),
        };
        let shift = match cflag {
            'c' => 4,
            'z' => 7,
            _ => panic!("Invalid flag"),
        };

        let cond = if z { 1 } else { 0 };

        if (self.registers.read_r8(Reg8::F) & flag) >> shift == cond {
            // Condition met - take the jump
            let offset = mem.read_8(self.registers.read_r16(Reg16::PC) + 1) as i8;
            let pc = self.registers.read_r16(Reg16::PC);
            let target = (pc as i32 + 2 + offset as i32) as u16;
            self.registers.write_r16(Reg16::PC, target);
        } else {
            // Condition not met - skip to next instruction (PC+2)
            let pc = self.registers.read_r16(Reg16::PC);
            self.registers.write_r16(Reg16::PC, pc + 2);
        }
    }

    fn jp_nn(&mut self, mem: &mut Memory) {
        let target_address = mem.read_16(self.registers.read_r16(Reg16::PC) + 1);
        self.registers.write_r16(Reg16::PC, target_address);
    }

    fn jp_f_nn(&mut self, mem: &mut Memory, cflag: char, condition: bool) {
        let flag = match cflag {
            'c' => CARRY_FLAG,
            'z' => ZERO_FLAG,
            _ => panic!("Invalid flag"),
        };

        let shift = match cflag {
            'c' => 4,
            'z' => 7,
            _ => panic!("Invalid flag"),
        };

        let cond = if condition { 1 } else { 0 };

        if (self.registers.read_r8(Reg8::F) & flag) >> shift == cond {
            let target_address = mem.read_16(self.registers.read_r16(Reg16::PC) + 1);
            self.registers.write_r16(Reg16::PC, target_address);
        } else {
            let pc = self.registers.read_r16(Reg16::PC);
            self.registers.write_r16(Reg16::PC, pc + 3);
        }
    }

    fn jp_hl(&mut self) {
        self.registers.write_r16(Reg16::PC, self.registers.read_r16(Reg16::HL));
    }

    fn call_nn(&mut self, mem: &mut Memory) {
        let target_address = mem.read_16(self.registers.read_r16(Reg16::PC) + 1);
        let return_address = self.registers.read_r16(Reg16::PC) + 3; // Return to instruction after CALL

        // Push return address onto stack
        self.registers
            .write_r16(Reg16::SP, self.registers.read_r16(Reg16::SP) - 2);
        mem.write_16(self.registers.read_r16(Reg16::SP), return_address);

        // Jump to target address
        self.registers.write_r16(Reg16::PC, target_address);
    }

    fn call_f_nn(&mut self, mem: &mut Memory, cflag: char, z: bool) {
        let flag = match cflag {
            'c' => CARRY_FLAG,
            'z' => ZERO_FLAG,
            _ => panic!("Invalid flag"),
        };

        let shift = match cflag {
            'c' => 4,
            'z' => 7,
            _ => panic!("Invalid flag"),
        };

        let cond = if z { 1 } else { 0 };

        if (self.registers.read_r8(Reg8::F) & flag) >> shift == cond {
            let target_address = mem.read_16(self.registers.read_r16(Reg16::PC) + 1);
            let return_address = self.registers.read_r16(Reg16::PC) + 3; // Return to instruction after CALL

            // Push return address onto stack
            self.registers
                .write_r16(Reg16::SP, self.registers.read_r16(Reg16::SP) - 2);
            mem.write_16(self.registers.read_r16(Reg16::SP), return_address);

            // Jump to target address
            self.registers.write_r16(Reg16::PC, target_address);
        } else {
            // Condition not met - skip to next instruction (PC+3)
            let pc = self.registers.read_r16(Reg16::PC);
            self.registers.write_r16(Reg16::PC, pc + 3);
        }
    }

    fn rst(&mut self, mem: &mut Memory, value: u16) {
        let return_address = self.registers.read_r16(Reg16::PC) + 1; // RST is 1 byte

        // Push return address onto stack
        let sp = self.registers.read_r16(Reg16::SP);
        self.registers.write_r16(Reg16::SP, sp.wrapping_sub(2));
        mem.write_16(self.registers.read_r16(Reg16::SP), return_address);

        // Jump to RST vector
        self.registers.write_r16(Reg16::PC, value);
    }

    fn ret(&mut self, mem: &mut Memory) {
        let value = mem.read_16(self.registers.clone().read_r16(Reg16::SP));
        self.registers
            .write_r16(Reg16::SP, self.registers.clone().read_r16(Reg16::SP) + 2);
        self.registers.write_r16(Reg16::PC, value);
    }

    fn ret_f(&mut self, mem: &mut Memory, cflag: char, z: bool) {
        let flag = match cflag {
            'c' => CARRY_FLAG,
            'z' => ZERO_FLAG,
            _ => panic!("Invalid flag"),
        };

        let shift = match cflag {
            'c' => 4,
            'z' => 7,
            _ => panic!("Invalid flag"),
        };

        let cond = if z { 1 } else { 0 };

        if (self.registers.read_r8(Reg8::F) & flag) >> shift == cond {
            // Condition met - perform return
            let value = mem.read_16(self.registers.read_r16(Reg16::SP));
            self.registers
                .write_r16(Reg16::SP, self.registers.read_r16(Reg16::SP) + 2);
            self.registers.write_r16(Reg16::PC, value);
        } else {
            // Condition not met - skip to next instruction (PC+1)
            let pc = self.registers.read_r16(Reg16::PC);
            self.registers.write_r16(Reg16::PC, pc + 1);
        }
    }

    fn reti(&mut self, mem: &mut Memory) {
        let value = mem.read_16(self.registers.read_r16(Reg16::SP));
        self.registers
            .write_r16(Reg16::SP, self.registers.read_r16(Reg16::SP) + 2);
        self.registers.write_r16(Reg16::PC, value);
        self.registers.write_ime(1); // Re-enable interrupts
    }

    //end of Cpu
    pub fn execute(&mut self, opcode: u8, mem: &mut Memory) {
        match opcode {
            0x00 => self.nop(),
            0x01 => self.ld_r16_nn(mem, Reg16::BC),
            0x02 => self.ld_operand(mem, Operand::MemBC, Operand::Reg8(Reg8::A)),
            0x03 => self.inc_r16(Reg16::BC),
            0x04 => self.inc_r8(Reg8::B),
            0x05 => self.dec_r8(Reg8::B),
            0x06 => self.ld_r8_n(mem, Reg8::B),
            0x07 => self.rlca(),
            0x08 => self.ld_nn_sp(mem),
            0x09 => self.add_hl(Reg16::BC),
            0x0A => self.ld_operand(mem, Operand::Reg8(Reg8::A), Operand::MemBC),
            0x0B => self.dec_r16(Reg16::BC),
            0x0C => self.inc_r8(Reg8::C),
            0x0D => self.dec_r8(Reg8::C),
            0x0E => self.ld_r8_n(mem, Reg8::C),
            0x0F => self.rrca(),
            0x10 => self.stop(),
            0x11 => self.ld_r16_nn(mem, Reg16::DE),
            0x12 => self.ld_operand(mem, Operand::MemDE, Operand::Reg8(Reg8::A)),
            0x13 => self.inc_r16(Reg16::DE),
            0x14 => self.inc_r8(Reg8::D),
            0x15 => self.dec_r8(Reg8::D),
            0x16 => self.ld_r8_n(mem, Reg8::D),
            0x17 => self.rla(),
            0x18 => self.jr_e(mem),
            0x19 => self.add_hl(Reg16::DE),
            0x1A => self.ld_operand(mem, Operand::Reg8(Reg8::A), Operand::MemDE),
            0x1B => self.dec_r16(Reg16::DE),
            0x1C => self.inc_r8(Reg8::E),
            0x1D => self.dec_r8(Reg8::E),
            0x1E => self.ld_r8_n(mem, Reg8::E),
            0x1F => self.rra(mem),
            0x20 => self.jr_f_e(mem, 'z', false),
            0x21 => self.ld_r16_nn(mem, Reg16::HL),
            0x22 => {
                self.ld_operand(mem, Operand::MemHL, Operand::Reg8(Reg8::A));
                self.inc_r16(Reg16::HL);
            }
            0x23 => self.inc_r16(Reg16::HL),
            0x24 => self.inc_r8(Reg8::H),
            0x25 => self.dec_r8(Reg8::H),
            0x26 => self.ld_r8_n(mem, Reg8::H),
            0x27 => self.daa(mem),
            0x28 => self.jr_f_e(mem, 'z', true),
            0x29 => self.add_hl(Reg16::HL),
            0x2A => {
                self.ld_operand(mem, Operand::Reg8(Reg8::A), Operand::MemHL);
                self.inc_r16(Reg16::HL);
            }
            0x2B => self.dec_r16(Reg16::HL),
            0x2C => self.inc_r8(Reg8::L),
            0x2D => self.dec_r8(Reg8::L),
            0x2E => self.ld_r8_n(mem, Reg8::L),
            0x2F => self.cpl(),
            0x30 => self.jr_f_e(mem, 'c', false),
            0x31 => self.ld_r16_nn(mem, Reg16::SP),
            0x32 => {
                self.ld_operand(mem, Operand::MemHL, Operand::Reg8(Reg8::A));
                self.dec_r16(Reg16::HL);
            }
            0x33 => self.inc_r16(Reg16::SP),
            0x34 => self.inc_mem(mem, Reg16::HL),
            0x35 => self.dec_mem(mem, Reg16::HL),
            0x36 => self.ld_m_n(mem),
            0x37 => self.scf(),
            0x38 => self.jr_f_e(mem, 'c', true),
            0x39 => self.add_hl(Reg16::SP),
            0x3A => {
                self.ld_operand(mem, Operand::Reg8(Reg8::A), Operand::MemHL);
                self.dec_r16(Reg16::HL);
            }
            0x3B => self.dec_r16(Reg16::SP),
            0x3C => self.inc_r8(Reg8::A),
            0x3D => self.dec_r8(Reg8::A),
            0x3E => self.ld_r8_n(mem, Reg8::A),
            0x3F => self.ccf(),
            0x76 => self.halt(mem), // HALT instruction (not LD (HL),(HL))
            0x40..=0x75 | 0x77..=0x7F => {
                // LD r1, r2 instructions (excluding 0x76 which is HALT)
                let dest = Operand::from_index((opcode >> 3) & 0x07);
                let src = Operand::from_index(opcode & 0x07);
                self.ld_operand(mem, dest, src);
            }
            0x80..=0x87 => self.add_a_r(mem, Operand::from_index(opcode & 0x07)),
            0x88..=0x8F => self.adc_a_r(mem, Operand::from_index(opcode & 0x07)),
            0x90..=0x97 => self.sub_a_r(mem, Operand::from_index(opcode & 0x07)),
            0x98..=0x9F => self.sbc_a_r(mem, Operand::from_index(opcode & 0x07)),
            0xA0..=0xA7 => self.and_a_r(mem, Operand::from_index(opcode & 0x07)),
            0xA8..=0xAF => self.xor_a_r(mem, Operand::from_index(opcode & 0x07)),
            0xB0..=0xB7 => self.or_a_r(mem, Operand::from_index(opcode & 0x07)),
            0xB8..=0xBF => self.cp_a_r(mem, Operand::from_index(opcode & 0x07)),
            0xC0 => self.ret_f(mem, 'z', false),
            0xC1 => self.pop(mem, Reg16::BC),
            0xC2 => self.jp_f_nn(mem, 'z', false),
            0xC3 => self.jp_nn(mem),
            0xC4 => self.call_f_nn(mem, 'z', false),
            0xC5 => self.push(mem, Reg16::BC),
            0xC6 => self.add_a_n(mem),
            0xC7 => self.rst(mem, 0x00),
            0xC8 => self.ret_f(mem, 'z', true),
            0xC9 => self.ret(mem),
            0xCA => self.jp_f_nn(mem, 'z', true),
            0xCB => self.call_cb(mem),
            0xCC => self.call_f_nn(mem, 'z', true),
            0xCD => self.call_nn(mem),
            0xCE => self.adc_a_n(mem),
            0xCF => self.rst(mem, 0x08),
            0xD0 => self.ret_f(mem, 'c', false),
            0xD1 => self.pop(mem, Reg16::DE),
            0xD2 => self.jp_f_nn(mem, 'c', false),
            0xD4 => self.call_f_nn(mem, 'c', false),
            0xD5 => self.push(mem, Reg16::DE),
            0xD6 => self.sub_a_n(mem),
            0xD7 => self.rst(mem, 0x10),
            0xD8 => self.ret_f(mem, 'c', true),
            0xD9 => self.reti(mem),
            0xDA => self.jp_f_nn(mem, 'c', true),
            0xDC => self.call_f_nn(mem, 'c', true),
            0xDE => self.sbc_a_n(mem),
            0xDF => self.rst(mem, 0x18),
            0xE0 => self.ldh_n_a(mem),
            0xE1 => self.pop(mem, Reg16::HL),
            0xE2 => self.ldh_c_a(mem),
            0xE5 => self.push(mem, Reg16::HL),
            0xE6 => self.and_a_n(mem),
            0xE7 => self.rst(mem, 0x20),
            0xE8 => self.add_sp_e(mem),
            0xE9 => self.jp_hl(),
            0xEA => self.ld_nn_a(mem),
            0xEE => self.xor_a_n(mem),
            0xEF => self.rst(mem, 0x28),
            0xF0 => self.ldh_a_n(mem),
            0xF1 => self.pop(mem, Reg16::AF),
            0xF2 => self.ldh_a_c(mem),
            0xF3 => self.di(),
            0xF5 => self.push(mem, Reg16::AF),
            0xF6 => self.or_a_n(mem),
            0xF7 => self.rst(mem, 0x30),
            0xF8 => self.ld_sp_e(mem),
            0xF9 => self.ld_sp_hl(mem),
            0xFA => {
                let addr = mem.read_16(self.registers.read_r16(Reg16::PC) + 1);
                let value = mem.read_8(addr);
                self.registers.write_r8(Reg8::A, value);
            }
            0xFB => self.ei(),
            0xFE => self.cp_a_n(mem),
            0xFF => self.rst(mem, 0x38),
            _ => {
                println!(
                    "CPU: Unknown/unimplemented opcode 0x{:02X} at PC 0x{:04X}!",
                    opcode,
                    self.registers.read_r16(Reg16::PC)
                );
                // Just NOP and continue instead of panicking
                let pc = self.registers.read_r16(Reg16::PC);
                self.registers.write_r16(Reg16::PC, pc + 1);
            }
        }
    }

    pub fn handle_post_instruction(&mut self, mem: &mut Memory, opcode: u8, _length: u64) -> u32 {
        // Check if this opcode modifies PC directly (jumps, calls, returns)
        // These opcodes should NOT have PC incremented

        let pc_modifying_opcodes = [
            0xC3, 0xC2, 0xCA, 0xD2, 0xDA, // JP nn, JP cc,nn
            0xE9, // JP (HL)
            0x18, 0x20, 0x28, 0x30, 0x38, // JR e, JR cc,e
            0xCD, 0xC4, 0xCC, 0xD4, 0xDC, // CALL nn, CALL cc,nn
            0xC9, 0xC0, 0xC8, 0xD0, 0xD8, // RET, RET cc
            0xD9, // RETI
            0xC7, 0xCF, 0xD7, 0xDF, 0xE7, 0xEF, 0xF7, 0xFF, // RST
        ];

        // Only increment PC if this is not a PC-modifying instruction
        if !pc_modifying_opcodes.contains(&opcode) {
            let pc = self.registers.read_r16(Reg16::PC);
            let length = OPCODE_LENGTHS[opcode as usize] as u16;

            // Handle HALT bug: when halt_bug is set, the next instruction after HALT
            // doesn't increment PC, causing it to execute twice
            if self.halt_bug {
                self.halt_bug = false;
                // PC stays at current position - next fetch will read same byte again
            } else {
                self.registers.write_r16(Reg16::PC, pc.wrapping_add(length));
            }
        }

        // Track cycles
        let mut cycles = OPCODE_DURATION[opcode as usize];

        if opcode == 0xCB {
            let cb_opcode = mem.read_8(self.registers.read_r16(Reg16::PC) + 1);
            cycles = OPCODE_DURATION_CB[cb_opcode as usize];
        }

        self.cycles += cycles as u64;
        cycles as u32
    }

    // Handle interrupts - should be called after each instruction
    pub fn handle_interrupts(&mut self, mem: &mut Memory) {
        // --- 1. SYNC HARDWARE FLAGS TO IF REGISTER (0xFF0F) ---

        let mut request_flags = 0;

        // VBlank (Bit 0)
        if mem.ppu.vblank_interrupt {
            request_flags |= 0x01;
            mem.ppu.vblank_interrupt = false; // Clear source
        }

        // LCD STAT (Bit 1)
        if mem.ppu.stat_interrupt {
            request_flags |= 0x02;
            mem.ppu.stat_interrupt = false; // Clear source
        }

        // Timer (Bit 2)
        if mem.timer.interrupt_pending {
            request_flags |= 0x04;
            mem.timer.interrupt_pending = false; // Clear source
        }

        // Serial (Bit 3)
        if mem.serial.interrupt_pending {
            request_flags |= 0x08;
            mem.serial.interrupt_pending = false; // Clear source
        }

        // Joypad (Bit 4)
        if mem.joypad.interrupt_requested {
            request_flags |= 0x10;
            mem.joypad.clear_interrupt(); // Clear source
        }

        // Write to IF register (0xFF0F)
        if request_flags != 0 {
            let current_if = mem.read_8(0xFF0F);
            mem.write_8(0xFF0F, current_if | request_flags);
        }

        // --- 2. SERVICE INTERRUPTS ---

        if self.registers.read_ime() == 0 && !self.halted {
            return;
        }

        // Read IE (Enabled) and IF (Request)
        let ie = mem.read_8(0xFFFF);
        let if_reg = mem.read_8(0xFF0F);
        let pending = ie & if_reg;

        // HALT BUG: If CPU is Halted, IME=0, and interrupt is pending,
        // the CPU wakes up but often encounters the "HALT bug" (PC fails to increment).
        if self.halted && pending != 0 {
            self.halted = false;
        }

        // If IME is disabled, we don't actually jump to the handler
        if self.registers.read_ime() == 0 {
            return;
        }

        if pending == 0 {
            return;
        }

        // Service highest priority interrupt
        // Priority: VBlank(0) > Stat(1) > Timer(2) > Serial(3) > Joypad(4)
        for i in 0..5 {
            if pending & (1 << i) != 0 {
                self.service_interrupt(mem, i);
                return; // Only service one interrupt per step
            }
        }
    }
    // Service an interrupt
    fn service_interrupt(&mut self, mem: &mut Memory, interrupt: u8) {
        // Cancel halted state if CPU was halted
        self.halted = false;

        // Disable interrupts
        self.registers.write_ime(0);
        self.ei_pending = false; // Cancel any pending EI

        let if_reg = mem.read_8(0xFF0F);
        mem.write_8(0xFF0F, if_reg & !(1 << interrupt));

        // Push PC onto stack
        let pc = self.registers.read_r16(Reg16::PC);
        let sp = self.registers.read_r16(Reg16::SP);
        self.registers.write_r16(Reg16::SP, sp.wrapping_sub(2));
        mem.write_16(self.registers.read_r16(Reg16::SP), pc);

        // Jump to interrupt vector
        let vector = 0x0040 + (interrupt as u16 * 0x08);
        self.registers.write_r16(Reg16::PC, vector);

        // Add interrupt handling cycles (20 cycles)
        self.cycles += 20;
    }
}
