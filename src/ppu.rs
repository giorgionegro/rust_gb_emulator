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
    // Per-pixel background color index (0..3) used to implement OBJ priority
    pub bg_color_index: [u8; 160 * 144],

    // Internal state
    pub mode_cycles: u32,
    pub vblank_interrupt: bool,

    // track previous LCD enabled state to avoid spam
    prev_lcd_enabled: bool,
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
// LCDC OBJ size bit
const LCDC_OBJ_SIZE: u8 = 0b00000100;

// Track last OAM checksum so we only dump when it changes
static mut LAST_OAM_CHECKSUM: u32 = 0;
static mut OAM_DUMP_INDEX: u32 = 0;

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
            bg_color_index: [0; 160 * 144],
            mode_cycles: 0,
            vblank_interrupt: false,
            // track previous LCD enabled state to avoid spam
            prev_lcd_enabled: true,
        }
    }

    /// Step PPU by given CPU cycles, returns true if VBlank interrupt requested
    pub fn step(&mut self, cycles: u32) -> bool {
        let lcd_enabled = (self.lcdc & LCDC_LCD_ENABLE) != 0;
        if !lcd_enabled {
            // When LCD is off, PPU doesn't run, but we need to track state
            // Reset to safe state
            if self.prev_lcd_enabled {
                // LCD just turned off
                self.ly = 0;
                self.mode_cycles = 0;
            }
            self.prev_lcd_enabled = false;
            return false;
        } else {
            self.prev_lcd_enabled = true;
        }

        self.mode_cycles += cycles;

        // Determine current mode early for tracing
        let current_mode = self.stat & STAT_MODE_MASK;

        let mut vblank = false;

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
                    } else if self.ly < 144 {
                        // Normal scanline 0-143: return to OAM scan for next line
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


                    }

                    if self.ly > 153 {
                        // println!("PPU: V-Blank complete - returning to LY=0");
                        self.ly = 0;
                        self.set_mode(MODE_OAM_SCAN);//I am stupid this was obviously needed
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

        // Log mode transitions to file, but print only a few to stdout
        if old_mode != mode {
            use std::fs::OpenOptions;
            use std::io::Write;
            let debug_info = format!("MODE_TRANSITION: {} -> {} (ly={})\n", old_mode, mode, self.ly);
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("ppu_mode_debug.txt") {
                let _ = file.write_all(debug_info.as_bytes());
            }
        }
    }

    fn render_scanline(&mut self) {
        let ly = self.ly as usize;
        if ly >= 144 {
            return;
        }

        // At start of frame (LY==0) check if OAM changed and dump for debugging
        if ly == 0 {
            use std::fs::OpenOptions;
            use std::io::Write;
            let mut checksum: u32 = 0;
            for &b in self.oam.iter() { checksum = checksum.wrapping_add(b as u32); }
            unsafe {
                if checksum != LAST_OAM_CHECKSUM {
                    LAST_OAM_CHECKSUM = checksum;
                    let mut f = OpenOptions::new().create(true).append(true).open("oam_debug.txt");
                    if let Ok(ref mut file) = f {
                        let _ = write!(file, "=== OAM_DUMP #{} CHECKSUM=0x{:08X} LCDC=0x{:02X} OBP0=0x{:02X} OBP1=0x{:02X}\n", OAM_DUMP_INDEX, checksum, self.lcdc, self.obp0, self.obp1);
                        for i in 0..40 {
                            let base = i * 4;
                            let y = self.oam[base];
                            let x = self.oam[base + 1];
                            let tile = self.oam[base + 2];
                            let attr = self.oam[base + 3];
                            let _ = write!(file, "oam[{}]: Y=0x{:02X} X=0x{:02X} T=0x{:02X} A=0x{:02X}\n", i, y, x, tile, attr);
                        }
                        let _ = write!(file, "\n");
                    }
                    OAM_DUMP_INDEX = OAM_DUMP_INDEX.wrapping_add(1);
                }
            }
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
                self.bg_color_index[ly * 160 + x] = 0;
            }
        }

        // Render sprites for this scanline (after background) so they overlay correctly
        self.render_sprites_line(ly);
        
        //window rendering not implemented TODO
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
                self.bg_color_index[ly * 160 + screen_x] = 0;
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
            // Save bg color_id for sprite priority decisions
            self.bg_color_index[ly * 160 + screen_x] = color_id;
        }
    }

    fn render_sprites_line(&mut self, ly: usize) {
        // Each OAM entry: Y, X, tile, attributes
        let obj_size = if (self.lcdc & LCDC_OBJ_SIZE) != 0 { 16 } else { 8 };

        // Collect up to 10 sprites on this line in OAM order
        let mut sprites_on_line: Vec<usize> = Vec::new();
        for i in 0..40 {
            let base = i * 4;
            let sprite_y = (self.oam[base] as i16) - 16;
            let sprite_x = (self.oam[base + 1] as i16) - 8;
            let tile = self.oam[base + 2];
            // attributes = self.oam[base+3]; -- used later

            if (ly as i16) >= sprite_y && (ly as i16) < (sprite_y + obj_size as i16) {
                sprites_on_line.push(i);
                if sprites_on_line.len() >= 10 { break; }
            }
        }

        // Draw sprites in OAM order (lower index has priority)
        for &i in sprites_on_line.iter() {
            let base = i * 4;
            let sprite_y = (self.oam[base] as i16) - 16;
            let sprite_x = (self.oam[base + 1] as i16) - 8;
            let mut tile = self.oam[base + 2];
            let attr = self.oam[base + 3];

            let y_in_sprite = (ly as i16 - sprite_y) as u8;
            let y = y_in_sprite as usize;
            let y_eff = if (attr & 0x40) != 0 { // Y flip
                (obj_size - 1) - y
            } else {
                y
            } as usize;

            // For 8x16 mode, tile number LSB is ignored (tile & 0xFE)
            if obj_size == 16 {
                tile &= 0xFE;
            }

            // Determine which tile within the sprite (for 8x16 may need second tile)
            let tile_index = (tile as u16) + (y_eff as u16 / 8) as u16;
            let tile_line = (y_eff % 8) as u16;
            let tile_addr = 0x8000u16 + tile_index * 16u16;
            let tile_offset = (tile_addr + tile_line * 2 - 0x8000) as usize;

            if tile_offset + 1 >= self.vram.len() { continue; }
            let byte1 = self.vram[tile_offset];
            let byte2 = self.vram[tile_offset + 1];

            for px in 0..8 {
                let bit_index = if (attr & 0x20) != 0 { // X flip
                    px
                } else {
                    7 - px
                };
                let color_low = (byte1 >> bit_index) & 1;
                let color_high = (byte2 >> bit_index) & 1;
                let color_id = (color_high << 1) | color_low;

                if color_id == 0 { continue; } // transparent

                let x = sprite_x + px as i16;
                if x < 0 || x >= 160 { continue; }
                let x_usize = x as usize;

                // OBJ priority: if bit 7 set and bg color != 0 => bg has priority
                if (attr & 0x80) != 0 {
                    let bg_color = self.bg_color_index[ly * 160 + x_usize];
                    if bg_color != 0 { continue; }
                }

                // Choose palette
                let palette = if (attr & 0x10) != 0 { self.get_palette(self.obp1) } else { self.get_palette(self.obp0) };
                let color = palette[color_id as usize];

                let fb_idx = (ly * 160 + x_usize) * 3;
                self.framebuffer[fb_idx] = color.0;
                self.framebuffer[fb_idx + 1] = color.1;
                self.framebuffer[fb_idx + 2] = color.2;
            }
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

                // When LCD is turned on, reset PPU timing
                if lcd_was_off && lcd_is_on {
                    self.ly = 0;
                    self.mode_cycles = 0;
                    self.set_mode(MODE_OAM_SCAN);
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
