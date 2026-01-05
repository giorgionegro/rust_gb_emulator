extern crate sdl2;
use gbemu_rust::cpu::{Cpu, Reg16};
use gbemu_rust::joypad::JoypadButton;
use gbemu_rust::memory::Memory;
use std::env;
use std::fs::File;
use std::io::Read;
use std::time::{Duration, Instant};

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;

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
    std::env::set_var("RUST_BACKTRACE", "1");

    let args: Vec<String> = env::args().collect();
    let rom_path = if args.len() > 1 {
        args[1].clone()
    } else {
        String::from("roms/test_roms/cpu_instrs.gb")
    };

    println!("Loading ROM: {}", rom_path);

    // Initialize SDL2
    let sdl_context = sdl2::init().expect("Failed to initialize SDL2");
    let video_subsystem = sdl_context
        .video()
        .expect("Failed to initialize video subsystem");

    // Create a window
    let window = video_subsystem
        .window("Game Boy Emulator", WINDOW_WIDTH, WINDOW_HEIGHT)
        .position_centered()
        .build()
        .expect("Failed to create window");

    // Create a canvas
    let mut canvas = window
        .into_canvas()
        .build()
        .expect("Failed to create canvas");
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, SCREEN_WIDTH, SCREEN_HEIGHT)
        .expect("Failed to create texture");

    // Load ROM
    let mut rom_file = File::open(&rom_path).expect("Failed to open ROM file");
    let mut rom_buffer = Vec::new();
    rom_file
        .read_to_end(&mut rom_buffer)
        .expect("Failed to read ROM file");

    // Initialize emulator components
    let mut mem = Memory::new(rom_buffer.clone());
    mem.init_rom_bank();
    mem.init_post_boot_state();

    let mut cpu = Cpu::new();
    cpu.registers.write_r16(Reg16::AF, 0x01B0);
    cpu.registers.write_r16(Reg16::BC, 0x0013);
    cpu.registers.write_r16(Reg16::DE, 0x00D8);
    cpu.registers.write_r16(Reg16::HL, 0x014D);
    cpu.registers.write_r16(Reg16::SP, 0xFFFE);
    cpu.registers.write_r16(Reg16::PC, 0x0100);
    cpu.registers.ime = 1; // Interrupts enabled after boot ROM

    // Main emulation loop
    let mut event_pump = sdl_context
        .event_pump()
        .expect("Failed to get SDL event pump");
    let frame_duration = Duration::from_secs_f64(1.0 / 60.0);
    let mut last_frame = Instant::now();

    // Serial forwarding state (mirror final_test harness)
    let mut last_serial_len: usize = 0;

    'running: loop {
        // Handle SDL events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(key), ..
                } => {
                    if let Some(button) = map_keycode_to_button(key) {
                        mem.joypad.press_button(button);
                    }
                }
                Event::KeyUp {
                    keycode: Some(key), ..
                } => {
                    if let Some(button) = map_keycode_to_button(key) {
                        mem.joypad.release_button(button);
                    }
                }
                _ => {}
            }
        }

        // Run CPU cycles for one frame
        let mut cycles = 0u32;
        while cycles < 70224 {
            let delta_cycles = cpu.step(&mut mem);
            cycles += delta_cycles;
            // Step PPU and Timer incrementally as in tests
            mem.ppu.step(delta_cycles);
            mem.timer.tick(delta_cycles as u16);
            // Progress OAM DMA timing: decrement remaining cycles and clear active when done
            if mem.dma_active {
                if delta_cycles >= mem.dma_cycles_remaining as u32 {
                    mem.dma_cycles_remaining = 0;
                    mem.dma_active = false;
                } else {
                    mem.dma_cycles_remaining =
                        mem.dma_cycles_remaining.wrapping_sub(delta_cycles as u16);
                }
            }
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
        texture
            .update(None, framebuffer, (SCREEN_WIDTH * 3) as usize)
            .expect("Failed to update texture");

        // Render to screen
        canvas.clear();
        let dst_rect = Rect::new(0, 0, WINDOW_WIDTH, WINDOW_HEIGHT);
        canvas
            .copy(&texture, None, Some(dst_rect))
            .expect("Failed to copy texture");
        canvas.present();

        // Frame timing this isnt optimal since we present before checking time, I think it would be better to do timing before presenting so we can properly have consistent frame pacing but this is good enough for now :)
        let frame_time = last_frame.elapsed();
        if frame_time < frame_duration {
            std::thread::sleep(frame_duration - frame_time);
        }
        last_frame = Instant::now();
    }
}
