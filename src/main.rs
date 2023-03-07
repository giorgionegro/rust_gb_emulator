use gbemu_rust::cpu::Cpu;
use gbemu_rust::memory::Memory;
use gbemu_rust::joypad::{Joypad, JoypadButton};
use std::fs::File;
use std::io::Read;
use std::time::{Duration, Instant};
use std::env;

extern crate sdl2;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

const SCREEN_WIDTH: u32 = 160;
const SCREEN_HEIGHT: u32 = 144;
const SCALE: u32 = 4;
const WINDOW_WIDTH: u32 = SCREEN_WIDTH * SCALE;
const WINDOW_HEIGHT: u32 = SCREEN_HEIGHT * SCALE;

fn map_keycode_to_button(keycode: Keycode) -> Option<JoypadButton> {
    match keycode {
        Keycode::Right => Some(JoypadButton::Right),
        Keycode::Left => Some(JoypadButton::Left),
        Keycode::Up => Some(JoypadButton::Up),
        Keycode::Down => Some(JoypadButton::Down),
        Keycode::Z => Some(JoypadButton::A),
        Keycode::X => Some(JoypadButton::B),
        Keycode::Return => Some(JoypadButton::Start),
        Keycode::RShift | Keycode::LShift => Some(JoypadButton::Select),
        _ => None,
    }
}

fn main() {
    // Enable backtrace for debugging
    std::env::set_var("RUST_BACKTRACE", "1");

    // Choose ROM path: first CLI arg or default to CPU instr test ROM
    let args: Vec<String> = env::args().collect();
    let rom_path = if args.len() > 1 {
        args[1].clone()
    } else {
        String::from("roms/test_roms/cpu_instrs.gb")
    };

    println!("Loading ROM: {}", rom_path);

    // Initialize SDL2
    let sdl_context = sdl2::init().expect("Failed to initialize SDL2");
    let video_subsystem = sdl_context.video().expect("Failed to initialize video subsystem");

    // Create a window
    let window = video_subsystem
        .window("Game Boy Emulator", WINDOW_WIDTH, WINDOW_HEIGHT)
        .position_centered()
        .build()
        .expect("Failed to create window");

    // Create a canvas
    let mut canvas = window.into_canvas().build().expect("Failed to create canvas");
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, SCREEN_WIDTH, SCREEN_HEIGHT)
        .expect("Failed to create texture");

    // Load ROM
    let mut rom_file = File::open(&rom_path).expect("Failed to open ROM file");
    let mut rom_buffer = Vec::new();
    rom_file.read_to_end(&mut rom_buffer).expect("Failed to read ROM file");

    // Initialize emulator components
    let mut mem = Memory::new(rom_buffer.clone());
    mem.init_rom_bank();
    mem.init_post_boot_state();

    let mut cpu = Cpu::new();
    cpu.registers.write_16("af", 0x01B0);
    cpu.registers.write_16("bc", 0x0013);
    cpu.registers.write_16("de", 0x00D8);
    cpu.registers.write_16("hl", 0x014D);
    cpu.registers.write_16("sp", 0xFFFE);
    cpu.registers.write_16("pc", 0x0100);

    let mut joypad = Joypad::new();

    // Main emulation loop
    let mut event_pump = sdl_context.event_pump().expect("Failed to get SDL event pump");
    let mut frame_count = 0;
    let frame_duration = Duration::from_secs_f64(1.0 / 60.0);
    let mut last_frame = Instant::now();

    // Serial forwarding state (mirror final_test harness)
    let mut last_serial_len: usize = 0;

    'running: loop {
        // Handle SDL events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown { keycode: Some(key), .. } => {
                    if let Some(button) = map_keycode_to_button(key) {
                        joypad.press_button(button);
                        mem.write_8(0xFF00, joypad.read());
                        if joypad.interrupt_requested {
                            let current_if = mem.read_8(0xFF0F);
                            mem.write_8(0xFF0F, current_if | 0x10);
                            joypad.clear_interrupt();
                        }
                    }
                }
                Event::KeyUp { keycode: Some(key), .. } => {
                    if let Some(button) = map_keycode_to_button(key) {
                        joypad.release_button(button);
                        mem.write_8(0xFF00, joypad.read());
                    }
                }
                _ => {}
            }
        }

        // Run CPU cycles for one frame
        let mut cycles = 0u32;
        while cycles < 70224 {
            let c = cpu.step(&mut mem);
            cycles += c;
            // Step PPU and Timer incrementally as in tests
            mem.ppu.step(c);
            mem.timer.tick(c as u16);
            cpu.handle_interrupts(&mut mem);

            // Forward serial output as it arrives
            let serial_output = mem.serial.get_output_string();
            if serial_output.len() > last_serial_len {
                let new_output = &serial_output[last_serial_len..];
                print!("{}", new_output);
                last_serial_len = serial_output.len();
            }
        }

        // Update texture with framebuffer
        let framebuffer = &mem.ppu.framebuffer;
        texture.update(None, framebuffer, (SCREEN_WIDTH * 3) as usize).expect("Failed to update texture");

        // Render to screen
        canvas.clear();
        canvas.copy(&texture, None, None).expect("Failed to copy texture");
        canvas.present();

        // Frame timing
        let frame_time = last_frame.elapsed();
        if frame_time < frame_duration {
            std::thread::sleep(frame_duration - frame_time);
        }
        last_frame = Instant::now();

        frame_count += 1;

        // Debug: print PC and serial status every 60 frames (approx 1s)
        if frame_count % 60 == 0 {
            let pc = cpu.registers.read_16("pc");
            let sp = cpu.registers.read_16("sp");
            let serial_len = mem.serial.get_output_string().len();
            println!("Frame {}: PC=0x{:04X}, SP=0x{:04X}, SerialLen={}", frame_count, pc, sp, serial_len);
        }
    }
}
