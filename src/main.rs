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
        String::from("roms/test_roms/instr_timing.gb")
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

    // FPS counter
    let mut fps_counter = 0u32;
    let mut fps_timer = Instant::now();
    let mut current_fps;

    // Dynamic estimate for presentation time (exponential moving average)
    let mut estimated_present_time = Duration::from_micros(0);
    const PRESENT_TIME_ALPHA: f64 = 0.2; // Smoothing factor for EMA

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

            /*if mem.dma_active {
                let m_cycles = (delta_cycles) as u16;
                if mem.dma_cycles_remaining > m_cycles {
                    mem.dma_cycles_remaining -= m_cycles;
                } else {
                    mem.dma_cycles_remaining = 0;
                    mem.dma_active = false;
                }
            }*/
            // NOTE: Timer/PPU ticking now happens INSIDE instructions via tick_components()
            // We no longer tick here to avoid double-ticking
            // DMA still needs to be progressed based on cycles

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

        // Prepare rendering
        canvas.clear();
        let dst_rect = Rect::new(0, 0, WINDOW_WIDTH, WINDOW_HEIGHT);
        canvas
            .copy(&texture, None, Some(dst_rect))
            .expect("Failed to copy texture");

        // Update FPS counter
        fps_counter += 1;
        if fps_timer.elapsed() >= Duration::from_secs(1) {
            current_fps = fps_counter;
            fps_counter = 0;
            fps_timer = Instant::now();

            // Update window title with FPS
            canvas
                .window_mut()
                .set_title(&format!("Game Boy Emulator - {} FPS", current_fps))
                .expect("Failed to set window title");
        }

        // Frame timing with dynamic presentation time estimate
        // Calculate sleep time accounting for estimated present() duration
        let frame_time = last_frame.elapsed();
        let target_sleep = frame_duration
            .saturating_sub(frame_time)
            .saturating_sub(estimated_present_time);

        if target_sleep > Duration::from_micros(100) {
            std::thread::sleep(target_sleep);
        }

        // Measure actual present time and update estimate
        let present_start = Instant::now();
        canvas.present();
        let actual_present_time = present_start.elapsed();

        let new_estimate_micros = (PRESENT_TIME_ALPHA * actual_present_time.as_micros() as f64)
            + ((1.0 - PRESENT_TIME_ALPHA) * estimated_present_time.as_micros() as f64);
        estimated_present_time = Duration::from_micros(new_estimate_micros as u64);

        last_frame = Instant::now();
    }
}
