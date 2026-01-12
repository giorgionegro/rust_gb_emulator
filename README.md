# gbemu_rust

A Game Boy (DMG-01) emulator written in Rust. This project implements the core hardware components of the original handheld.

## Features

- CPU: Emulation of the Sharp LR35902 instruction set.
- PPU: Tile-based background/window and sprite rendering.
- MMU: 16-bit address space management and I/O mapping.
- Timer: System clock and internal timer synchronization.
- Joypad: Keyboard input mapping.

## Requirements

- Rust (latest stable toolchain)
- SDL2

## Usage

To run the emulator with a ROM file:

```bash
cargo run -- path/to/rom.gb
