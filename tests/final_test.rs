use gbemu_rust::cpu::Cpu;
use gbemu_rust::memory::Memory;
use std::fs::File;
use std::io::Read;

fn main() {
    println!("=== CPU Instruction Test ===\n");

    let mut file = File::open("roms/test_roms/cpu_instrs.gb")
        .expect("Failed to open ROM file");

    let mut buffer = vec![];
    file.read_to_end(&mut buffer).expect("Failed to read ROM");
    buffer.resize(0x2FFFF, 0);

    // Initialize memory with the ROM buffer
    let mut mem = Memory::new(buffer.clone());
    mem.init_rom_bank();
    mem.init_post_boot_state();

    let mut cpu = Cpu::new();
    cpu.registers.write_16("af", 0x01B0);
    cpu.registers.write_16("bc", 0x0013);
    cpu.registers.write_16("de", 0x00D8);
    cpu.registers.write_16("hl", 0x014D);
    cpu.registers.write_16("sp", 0xFFFE);
    cpu.registers.write_16("pc", 0x0100);

    println!("Initial state:");
    println!("  PC: 0x{:04X}", cpu.registers.read_16("pc"));
    println!("  SP: 0x{:04X}", cpu.registers.read_16("sp"));
    println!();

    let max_cycles: u64 = 2_000_000_000; // explicit u64
    let mut cycle_count = 0;
    let mut last_serial_len = 0;
    let mut instruction_count = 0;
    let mut prev_pc = 0;
    let mut pc_zero_count = 0;

    while cycle_count < max_cycles {
        let pc = cpu.registers.read_16("pc");

        // Check for infinite loop or stuck state
        if pc == 0 {
            pc_zero_count += 1;
            if pc_zero_count == 1 {
                println!("\n  PC became 0!");
                println!("Previous PC: 0x{:04X}", prev_pc);
                println!("SP: 0x{:04X}", cpu.registers.read_16("sp"));
            }
            // Continue for a bit at PC=0 to catch final serial output
            if pc_zero_count > 10000 {
                // Check what's on the stack
                let sp = cpu.registers.read_16("sp");
                if sp < 0xFFFE {
                    let stack_top = mem.read_16(sp);
                    println!("Value at SP: 0x{:04X}", stack_top);
                    let stack_prev = mem.read_16(sp + 2);
                    println!("Value at SP+2: 0x{:04X}", stack_prev);
                }

                // Check what instruction was at previous PC
                if prev_pc > 0 {
                    let opcode = mem.read_8(prev_pc);
                    println!("Opcode at prev PC: 0x{:02X}", opcode);
                }
                break;
            }
        }

        prev_pc = pc;
        let cycles = cpu.step(&mut mem);
        cycle_count += cycles as u64;
        instruction_count += 1;

        // Step PPU with actual cycles
        mem.ppu.step(cycles as u32);

        // Step Timer with actual cycles
        mem.timer.tick(cycles as u16);

        // Handle interrupts
        cpu.handle_interrupts(&mut mem);

        // Check for serial output
        let serial_output = mem.serial.get_output_string();
        if serial_output.len() > last_serial_len {
            let new_output = &serial_output[last_serial_len..];
            print!("{}", new_output);
            last_serial_len = serial_output.len();

            // Check if test completed
            if serial_output.contains("Passed") {
                println!("\n\n TEST PASSED!");
                break;
            } else if serial_output.contains("Failed") {
                println!("\n\n❌ TEST FAILED");
                break;
            }
        }

        // Print progress
        if instruction_count % 100_000 == 0 {
            println!("\nInstructions: {}, Cycles: {}, PC: 0x{:04X}, SP: 0x{:04X}",
                instruction_count, cycle_count, pc, cpu.registers.read_16("sp"));
        }
    }

    println!("\n=== Final State ===");
    println!("Instructions executed: {}", instruction_count);
    println!("Total cycles: {}", cycle_count);
    println!("Final PC: 0x{:04X}", cpu.registers.read_16("pc"));
    println!("Final SP: 0x{:04X}", cpu.registers.read_16("sp"));

    if last_serial_len > 0 {
        println!("\n=== Serial Output ===");
        println!("{}", mem.serial.get_output_string());
    } else {
        println!("\n No serial output detected");
    }
}
