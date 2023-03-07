pub struct Ppu {
    pub vram: [u8; 0x2000],
    pub oam: [u8; 0xA0],

    // LCD Control registers
    pub lcdc: u8,
    pub stat: u8,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub lyc: u8,
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
    pub wy: u8,
    pub wx: u8,

    // RGB framebuffer for SDL2
    pub framebuffer: [u8; 160 * 144 * 3],

    // Internal state
    pub mode_cycles: u32,
    pub vblank_interrupt: bool,
}

// LCD Modes
const MODE_HBLANK: u8 = 0;
const MODE_VBLANK: u8 = 1;
const MODE_OAM_SCAN: u8 = 2;
const MODE_DRAWING: u8 = 3;

// LCDC flags
const LCDC_LCD_ENABLE: u8 = 0b10000000;
const LCDC_BG_TILEMAP: u8 = 0b00001000;
const LCDC_BG_WINDOW_TILES: u8 = 0b00010000;
const LCDC_BG_ENABLE: u8 = 0b00000001;

// STAT flags
const STAT_MODE_MASK: u8 = 0b00000011;

// Timing (in CPU cycles)
const OAM_SCAN_CYCLES: u32 = 80;
const DRAWING_CYCLES: u32 = 172;
const HBLANK_CYCLES: u32 = 204;
const SCANLINE_CYCLES: u32 = 456;

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            lcdc: 0x91,
            stat: 0x02,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            framebuffer: [0; 160 * 144 * 3],
            mode_cycles: 0,
            vblank_interrupt: false,
        }
    }

    /// Step PPU by given CPU cycles, returns true if VBlank interrupt requested
    pub fn step(&mut self, cycles: u32) -> bool {
        if (self.lcdc & LCDC_LCD_ENABLE) == 0 {
            println!("PPU disabled: LCDC=0x{:02X}", self.lcdc);
            return false;
        }

        // Log VRAM contents for debugging
        if self.ly == 0 && self.mode_cycles == 0 {
            println!("VRAM[0..16]: {:?}", &self.vram[0..16]);
        }

        self.mode_cycles += cycles;
        let current_mode = self.stat & STAT_MODE_MASK;
        let mut vblank = false;

        // Debug PPU step calls
        static mut STEP_COUNT: u32 = 0;
        unsafe {
            STEP_COUNT += 1;
            if STEP_COUNT % 1000 == 0 {
                use std::fs::OpenOptions;
                use std::io::Write;
                let debug_info = format!("PPU_STEP: count={} cycles={} mode_cycles={} ly={} mode={}\n",
                    STEP_COUNT, cycles, self.mode_cycles, self.ly, current_mode);
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("ppu_debug.txt") {
                    let _ = file.write_all(debug_info.as_bytes());
                }
            }
        }

        match current_mode {
            MODE_OAM_SCAN => {
                if self.mode_cycles >= OAM_SCAN_CYCLES {
                    self.mode_cycles -= OAM_SCAN_CYCLES;
                    self.set_mode(MODE_DRAWING);
                }
            }
            MODE_DRAWING => {
                if self.mode_cycles >= DRAWING_CYCLES {
                    self.mode_cycles -= DRAWING_CYCLES;
                    self.set_mode(MODE_HBLANK);
                    self.render_scanline();
                }
            }
            MODE_HBLANK => {
                if self.mode_cycles >= HBLANK_CYCLES {
                    self.mode_cycles -= HBLANK_CYCLES;
                    self.ly += 1;

                    if self.ly == 144 {
                        self.set_mode(MODE_VBLANK);
                        vblank = true;
                        self.vblank_interrupt = true;
                    } else if self.ly > 153 {
                        self.ly = 0;
                        self.set_mode(MODE_OAM_SCAN);
                    }
                }
            }
            MODE_VBLANK => {
                if self.mode_cycles >= SCANLINE_CYCLES {
                    self.mode_cycles -= SCANLINE_CYCLES;

                    // Hold at LY=144 for the first VBlank scanline to give games time to detect it
                    if self.ly == 144 {
                        self.ly = 145;  // Move to next scanline after one full scanline at 144
                        // println!("PPU: V-Blank - LY advanced to {}", self.ly);
                    } else {
                        let old_ly = self.ly;
                        self.ly += 1;

                        // Log critical LY values during V-Blank
                        // if self.ly >= 148 && old_ly < 148 {
                        //     println!("PPU: V-Blank - LY reached {} (0x{:02X}) - Castlevania target reached!", self.ly, self.ly);
                        // } else if self.ly % 10 == 0 {
                        //     println!("PPU: V-Blank - LY = {}", self.ly);
                        // }
                    }

                    if self.ly > 153 {
                        // println!("PPU: V-Blank complete - returning to LY=0");
                        self.ly = 0;
                        self.set_mode(MODE_OAM_SCAN);
                    }
                }
            }
            _ => {}
        }

        vblank
    }

    fn set_mode(&mut self, mode: u8) {
        let old_mode = self.stat & STAT_MODE_MASK;
        self.stat = (self.stat & !STAT_MODE_MASK) | (mode & STAT_MODE_MASK);

        // Debug mode transitions (file) and limited stdout
        if old_mode != mode {
            use std::fs::OpenOptions;
            use std::io::Write;
            let debug_info = format!("MODE_TRANSITION: {} -> {} (ly={})\n", old_mode, mode, self.ly);
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("ppu_mode_debug.txt") {
                let _ = file.write_all(debug_info.as_bytes());
            }

            static mut MODE_PRINT_COUNT: u32 = 0;
            unsafe {
                if MODE_PRINT_COUNT < 50 {
                    println!("PPU MODE: {} -> {} (LY={})", old_mode, mode, self.ly);
                }
                MODE_PRINT_COUNT += 1;
            }
        }
    }

    fn render_scanline(&mut self) {
        let ly = self.ly as usize;
        if ly >= 144 {
            return;
        }

        // Debug: print first few scanlines being rendered
        static mut RENDER_COUNT: u32 = 0;
        unsafe {
            if RENDER_COUNT < 20 {
                println!("PPU: render_scanline called for LY={}", ly);
            }
            RENDER_COUNT += 1;
        }

        let palette = self.get_palette(self.bgp);

        if (self.lcdc & LCDC_BG_ENABLE) != 0 {
            self.render_background_line(ly, &palette);
        } else {
            // BG disabled - fill with white
            for x in 0..160 {
                let idx = (ly * 160 + x) * 3;
                self.framebuffer[idx] = 0x9B;
                self.framebuffer[idx + 1] = 0xBC;
                self.framebuffer[idx + 2] = 0x0F;
            }
        }
    }

    fn render_background_line(&mut self, ly: usize, palette: &[(u8, u8, u8); 4]) {
        let y = (ly as u8).wrapping_add(self.scy);
        let tile_y = ((y / 8) % 32) as u16;  // Wrap at 32 tiles
        let tile_y_offset = (y % 8) as u16;

        let tilemap_base = if (self.lcdc & LCDC_BG_TILEMAP) != 0 {
            0x9C00u16
        } else {
            0x9800u16
        };

        let signed_addressing = (self.lcdc & LCDC_BG_WINDOW_TILES) == 0;

        for screen_x in 0..160 {
            let x = (screen_x as u8).wrapping_add(self.scx);
            let tile_x = ((x / 8) % 32) as u16;  // Wrap at 32 tiles
            let tile_x_offset = 7 - (x % 8);

            // Calculate tilemap address with bounds checking
            let tilemap_offset = tile_y * 32 + tile_x;
            if tilemap_offset >= 1024 {
                // Out of bounds, skip this pixel
                continue;
            }

            let tilemap_addr = tilemap_base + tilemap_offset;
            let vram_index = (tilemap_addr - 0x8000) as usize;

            if vram_index >= 0x2000 {
                // Out of VRAM bounds, skip
                continue;
            }

            let tile_num = self.vram[vram_index];

            let tile_addr = if signed_addressing {
                let offset = (tile_num as i8 as i16 + 128) as u16;
                0x8800u16 + offset * 16
            } else {
                0x8000u16 + (tile_num as u16) * 16
            };

            // Bounds check tile data access
            let tile_data_offset = (tile_addr + tile_y_offset * 2 - 0x8000) as usize;
            if tile_data_offset >= 0x1FFF {
                // Out of bounds, use color 0
                let fb_idx = (ly * 160 + screen_x) * 3;
                let color = palette[0];
                self.framebuffer[fb_idx] = color.0;
                self.framebuffer[fb_idx + 1] = color.1;
                self.framebuffer[fb_idx + 2] = color.2;
                continue;
            }

            let byte1 = self.vram[tile_data_offset];
            let byte2 = self.vram[tile_data_offset + 1];

            let color_low = (byte1 >> tile_x_offset) & 1;
            let color_high = (byte2 >> tile_x_offset) & 1;
            let color_id = (color_high << 1) | color_low;

            let fb_idx = (ly * 160 + screen_x) * 3;
            let color = palette[color_id as usize];
            self.framebuffer[fb_idx] = color.0;
            self.framebuffer[fb_idx + 1] = color.1;
            self.framebuffer[fb_idx + 2] = color.2;
        }
    }

    fn get_palette(&self, palette_byte: u8) -> [(u8, u8, u8); 4] {
        const COLORS: [(u8, u8, u8); 4] = [
            (0x9B, 0xBC, 0x0F), // Lightest
            (0x8B, 0xAC, 0x0F), // Light
            (0x30, 0x62, 0x30), // Dark
            (0x0F, 0x38, 0x0F), // Darkest
        ];

        let mut result = [(0, 0, 0); 4];
        for i in 0..4 {
            let color_id = (palette_byte >> (i * 2)) & 0x03;
            result[i] = COLORS[color_id as usize];
        }
        result
    }

    pub fn read(&self, address: u16) -> u8 {
        let value = match address {
            0xFF40 => self.lcdc,
            0xFF41 => self.stat,
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => {
                // Log when LY reaches critical value to track if CPU continues
                if self.ly >= 148 {
                    use std::fs::OpenOptions;
                    use std::io::Write;
                    let debug_info = format!("CPU reads LY={} at high value\n", self.ly);
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("ly_high_reads.txt") {
                        let _ = file.write_all(debug_info.as_bytes());
                    }
                }

                self.ly
            },
            0xFF45 => self.lyc,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => 0xFF,
        };
        value
    }

    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0xFF40 => {
                let lcd_was_off = (self.lcdc & LCDC_LCD_ENABLE) == 0;
                self.lcdc = value;
                let lcd_is_on = (self.lcdc & LCDC_LCD_ENABLE) != 0;

                // Debug: log writes to LCDC
                println!("PPU WRITE: LCDC <= 0x{:02X} (was_off={}, now_on={})", value, lcd_was_off, lcd_is_on);

                // When LCD is turned on, reset PPU timing
                if lcd_was_off && lcd_is_on {
                    self.ly = 0;
                    self.mode_cycles = 0;
                    self.set_mode(MODE_OAM_SCAN);
                    // println!("PPU: LCD turned ON, resetting LY and mode cycles.");
                }
            },
            0xFF41 => self.stat = (self.stat & 0x07) | (value & 0xF8),
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => {}, // LY is read-only
            0xFF45 => self.lyc = value,
            0xFF47 => self.bgp = value,
            0xFF48 => self.obp0 = value,
            0xFF49 => self.obp1 = value,
            0xFF4A => self.wy = value,
            0xFF4B => self.wx = value,
            _ => {}
        }
    }

    /// Check if a frame is ready
    pub fn frame_ready(&self) -> bool {
        self.vblank_interrupt
    }

    /// Get the framebuffer data
    pub fn get_framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }
}
