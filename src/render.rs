//! Hi-res framebuffer drawing, sprites and text for ALIEN SPACE DROP.
//!
//! Everything is drawn into a 280x192 colour-index canvas (an abstraction of
//! the Apple II high-resolution graphics page), then mapped to the Apple II
//! colour palette and upscaled with a CRT-shaped shader in the app.

use crate::game::{
    Explosion, Game, C_BLACK, C_BLUE, C_GREEN, C_ORANGE, C_VIOLET, C_WHITE, EXPLOSION_TIME,
    GROUND_Y, PADS, SHIP_H, SHIP_W,
};

/// The Apple II hi-res palette (rough NTSC composite values).
pub const PALETTE: [[u8; 3]; 6] = [
    [0, 0, 0],       // black
    [255, 255, 255], // white
    [240, 56, 168],  // violet
    [0, 168, 76],    // green
    [76, 48, 252],   // blue
    [255, 120, 0],   // orange
];

/// A 280x192 colour-index framebuffer.
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub buf: Vec<u8>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas {
            w,
            h,
            buf: vec![C_BLACK; w * h],
        }
    }

    #[inline]
    pub fn px(&mut self, x: i32, y: i32, c: u8) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        self.buf[(y as usize) * self.w + x as usize] = c;
    }

    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, c: u8) {
        for x in x0..=x1 {
            self.px(x, y, c);
        }
    }

    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, c: u8) {
        for y in y0..=y1 {
            self.px(x, y, c);
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8) {
        for cy in y..y + h {
            self.hline(x, x + w - 1, cy, c);
        }
    }

    pub fn rect_ring(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8) {
        self.hline(x, x + w - 1, y, c);
        self.hline(x, x + w - 1, y + h - 1, c);
        self.vline(x, y, y + h - 1, c);
        self.vline(x + w - 1, y, y + h - 1, c);
    }

    /// Filled ellipse (integer scanline approximation).
    pub fn ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, c: u8) {
        if rx <= 0 || ry <= 0 {
            return;
        }
        for dy in -ry..=ry {
            let t = dy as f32 / ry as f32;
            let half = ((rx as f32) * (1.0 - t * t).sqrt().max(0.0)).round() as i32;
            self.hline(cx - half, cx + half, cy + dy, c);
        }
    }

    /// A ring: an outer filled ellipse with an inner one punched out.
    pub fn ellipse_ring(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, thickness: i32, c: u8) {
        self.ellipse(cx, cy, rx, ry, c);
        self.ellipse(cx, cy, rx - thickness, ry - thickness, C_BLACK);
    }

    /// Shift the whole framebuffer by (dx, dy), filling the exposed edge black
    /// (used for the crash screen-shake).
    pub fn scroll(&mut self, dx: i32, dy: i32) {
        if dx == 0 && dy == 0 {
            return;
        }
        let mut out = vec![C_BLACK; self.buf.len()];
        for y in 0..self.h as i32 {
            for x in 0..self.w as i32 {
                let nx = x - dx;
                let ny = y - dy;
                if nx < 0 || ny < 0 || nx >= self.w as i32 || ny >= self.h as i32 {
                    continue;
                }
                let from = (y as usize) * self.w + x as usize;
                let to = (ny as usize) * self.w + nx as usize;
                out[to] = self.buf[from];
            }
        }
        self.buf = out;
    }

    pub fn to_rgba(&self) -> Vec<u8> {
        let mut rgba = vec![0u8; self.buf.len() * 4];
        for (i, &ci) in self.buf.iter().enumerate() {
            let p = PALETTE[ci as usize];
            let o = i * 4;
            rgba[o] = p[0];
            rgba[o + 1] = p[1];
            rgba[o + 2] = p[2];
            rgba[o + 3] = 255;
        }
        rgba
    }
}

// ---- Sprites ---------------------------------------------------------------

/// A sprite is a list of rows; every character maps to a colour, `'.'` is
/// transparent. Rows shorter than the sprite width are centred (filled with
/// transparent), which makes the ASCII art easy to keep symmetric.
pub struct Sprite {
    rows: Vec<Vec<u8>>,
}

fn color_of(ch: char) -> Option<u8> {
    Some(match ch {
        'W' => C_WHITE,
        'V' => C_VIOLET,
        'G' => C_GREEN,
        'B' => C_BLUE,
        'O' => C_ORANGE,
        _ => return None,
    })
}

impl Sprite {
    pub fn parse(lines: &[&str]) -> Sprite {
        let h = lines.len();
        let max_w = lines.iter().map(|l| l.len()).max().unwrap_or(1);
        let mut rows = Vec::with_capacity(h);
        for line in lines {
            let mut row = vec![0u8; max_w];
            let lead = (max_w - line.len()) / 2;
            for (ci, ch) in line.chars().enumerate() {
                row[lead + ci] = color_of(ch).unwrap_or(C_BLACK);
            }
            rows.push(row);
        }
        Sprite { rows }
    }

    /// Blit at the given top-left corner. Optionally remaps every drawn colour
    /// through `remap` so one sprite serves multiple colour schemes.
    pub fn blit(&self, c: &mut Canvas, x: i32, y: i32, remap: Option<fn(u8) -> u8>) {
        for (dy, row) in self.rows.iter().enumerate() {
            for (dx, &col) in row.iter().enumerate() {
                if col == C_BLACK {
                    continue;
                }
                let out = match remap {
                    Some(f) => f(col),
                    None => col,
                };
                c.px(x + dx as i32, y + dy as i32, out);
            }
        }
    }
}

/// The player's saucer: white dome, green glass, violet hull, blue windows,
/// orange under-lights.
pub fn player_ship() -> Sprite {
    Sprite::parse(&[
        "W",
        "WWWWWW",
        "WWGGGGGGWW",
        "WWGGGGGGWW",
        "WWGGGGGGWW",
        "WWWWWWWWWW",
        ".WWWVVVVVVVWWW.",
        "WVVVVVVVVVVVVVW",
        "WVVVVVVVVVVVVVW",
        "WBBVVVVVVVBBW",
        "WVVVVVVVVVVVVVW",
        ".WWWVVVVVVVWWW.",
        "..WWWWWWWWWW..",
        "....O.........O....",
    ])
}

/// The obstacle saucer (same shape, tinted per kind by the renderer).
pub fn obstacle_ship() -> Sprite {
    Sprite::parse(&[
        "..WWWW..",
        "WWWGGGGWWW",
        "WVVVVVVVVVW",
        "WVVVVVVVVVW",
        "WVVVBBBBVVW",
        "WVVVVVVVVVW",
        "WWWVVVVVWWW",
        "..OO....OO..",
    ])
}

fn kind_remap(kind: u8) -> Option<fn(u8) -> u8> {
    match kind % 3 {
        0 => None, // violet
        1 => Some(|c| if c == C_VIOLET { C_BLUE } else { c }),
        _ => Some(|c| if c == C_VIOLET { C_ORANGE } else { c }),
    }
}

// ---- 5x7 text (the Apple II ROM style) --------------------------------------

fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        'A' => [0x04, 0x0A, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x01, 0x01, 0x01, 0x01, 0x11, 0x11, 0x0E],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '-' => [0, 0, 0, 0x1F, 0, 0, 0],
        '+' => [0, 0, 0x04, 0x1F, 0x04, 0, 0],
        ':' => [0, 0, 0x04, 0, 0, 0x04, 0],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0, 0x04],
        '.' => [0, 0, 0, 0, 0, 0, 0x04],
        ',' => [0, 0, 0, 0, 0, 0x04, 0x08],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '?' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0, 0x04],
        '\'' => [0x04, 0x08, 0, 0, 0, 0, 0],
        '*' => [0, 0x0A, 0x04, 0x0E, 0x04, 0x0A, 0],
        '>' => [0, 0x08, 0x04, 0x02, 0x04, 0x08, 0],
        '<' => [0, 0x02, 0x04, 0x08, 0x04, 0x02, 0],
        _ => return None,
    })
}

/// Rendered width of `s` in hi-res pixels at the given scale.
pub fn text_width(s: &str, scale: i32) -> i32 {
    s.chars().count() as i32 * 6 * scale
}

pub fn draw_text(c: &mut Canvas, x: i32, y: i32, s: &str, color: u8, scale: i32) {
    for (ci, ch) in s.chars().enumerate() {
        let g = glyph(ch).unwrap_or([0; 7]);
        let ox = x + ci as i32 * 6 * scale;
        for (ry, &bits) in g.iter().enumerate() {
            for rx in 0..5 {
                if bits & (0x10 >> rx) == 0 {
                    continue;
                }
                if scale == 1 {
                    c.px(ox + rx, y + ry as i32, color);
                } else {
                    c.rect(ox + rx * scale, y + ry as i32 * scale, scale, scale, color);
                }
            }
        }
    }
}

/// Centred text: `cx` is the text's centre x.
pub fn draw_text_cx(c: &mut Canvas, cx: i32, y: i32, s: &str, color: u8, scale: i32) {
    let x = cx - text_width(s, scale) / 2;
    draw_text(c, x, y, s, color, scale);
}

// ---- Scene drawing ----------------------------------------------------------

fn star_visible(clock: f32, phase: f32, speed: f32) -> bool {
    let v = (clock * speed * 4.0 + phase) as i32;
    (v % 11) != 0
}

pub fn draw_stars(c: &mut Canvas, g: &Game) {
    for s in &g.stars {
        if star_visible(g.clock, s.phase, s.speed) {
            c.px(s.x as i32, s.y as i32, s.color);
        }
    }
}

fn draw_ground(c: &mut Canvas, g: &Game) {
    let gy = GROUND_Y as i32;
    // Ground line + fill.
    c.hline(0, 279, gy, C_WHITE);
    c.hline(0, 279, gy + 1, C_WHITE);
    c.rect(0, gy + 2, 280, 192 - gy - 2, C_BLUE);

    for (i, p) in PADS.iter().enumerate() {
        let x0 = (p.cx - p.half) as i32;
        let x1 = (p.cx + p.half) as i32;
        // Hatched pad surface in the pad's colour.
        for y in gy + 2..192 {
            for x in x0..=x1 {
                if (x + y) % 6 < 3 {
                    c.px(x, y, p.color);
                }
            }
        }
        // White outline.
        c.rect_ring(x0, gy, x1 - x0 + 1, 192 - gy, C_WHITE);
        // Beacon light on the top edge (flashes).
        let blink_on = (g.clock as i32 * 4 + i as i32 * 7) % 5 != 2;
        let beacon = if blink_on { C_WHITE } else { p.color };
        c.px((p.cx - 1.0) as i32, gy - 1, beacon);
        c.px(p.cx as i32, gy - 1, beacon);
        c.px((p.cx + 1.0) as i32, gy - 1, beacon);
        // Points label above each pad (tiny, like a BASIC DATA listing).
        let pts = format!("{}", p.points);
        draw_text_cx(c, p.cx as i32, gy - 9, &pts, p.color, 1);
    }
}

/// Draw a small downward flame under the ship when boosting.
fn draw_flame(c: &mut Canvas, ship: &crate::game::Ship) {
    let cx = ship.x as i32;
    let by = (ship.y + SHIP_H) as i32;
    let t = (ship.y * 13.0 + ship.x * 7.0) as i32;
    c.px(cx - 2, by, C_ORANGE);
    c.px(cx + 2, by, C_ORANGE);
    if t % 2 == 0 {
        c.px(cx - 1, by, C_ORANGE);
        c.px(cx + 1, by, C_ORANGE);
    }
    c.px(cx, by, C_ORANGE);
    c.px(cx, by + 1, C_WHITE);
}

fn draw_explosion(c: &mut Canvas, e: &Explosion) {
    let t = (e.t / EXPLOSION_TIME).clamp(0.0, 1.0);
    let (x, y) = (e.x as i32, e.y as i32);
    // Outer shock ring.
    let ring_r = (2.0 + t * 17.0) as i32;
    c.ellipse_ring(x, y, ring_r, (ring_r * 6 / 10).max(1), 2, C_ORANGE);
    // White-hot core that shrinks as it fades.
    let core_r = ((14.0 * (1.0 - t)).max(1.0)) as i32;
    c.ellipse(x, y, core_r, (core_r * 8 / 10).max(1), C_WHITE);
    c.ellipse(
        x,
        y,
        (core_r - 3).max(0),
        (core_r - 3).max(0) * 8 / 10,
        C_ORANGE,
    );
    // Deterministic debris sparks.
    for k in 0..8u8 {
        let a = t * 7.0 + k as f32 * 0.85;
        let rr = (3.0 + t * (14.0 + (k % 3) as f32 * 4.0)) as i32;
        let dx = (a.cos() * rr as f32) as i32;
        let dy = (a.sin() * rr as f32) as i32;
        let col = if k % 2 == 0 { C_VIOLET } else { C_WHITE };
        c.px(x + dx, y + dy, col);
    }
}

fn draw_obstacles(c: &mut Canvas, g: &Game) {
    let spr = obstacle_ship();
    for o in &g.obstacles {
        spr.blit(c, o.x as i32, o.y as i32, kind_remap(o.kind));
    }
}

fn draw_ship(c: &mut Canvas, g: &Game) {
    let spr = player_ship();
    let x = (g.ship.x - SHIP_W / 2.0) as i32;
    let y = g.ship.y as i32;
    if g.ship.boosting {
        draw_flame(c, &g.ship);
    }
    spr.blit(c, x, y, None);
}

/// Draw the playfield world (ground, pads, obstacles, ship/explosion, stars).
pub fn draw_world(c: &mut Canvas, g: &Game) {
    draw_stars(c, g);

    // Obstacles fly behind the ground strip.
    draw_obstacles(c, g);
    draw_ground(c, g);

    if let Some(e) = &g.explosion {
        draw_explosion(c, e);
    } else if let Some(f) = &g.flash {
        // The landed ship rests on its pad during the celebration.
        draw_ship(c, g);
        // Beacon ring + "+points" popup.
        let p = &PADS[f.pad];
        let x0 = (p.cx - p.half) as i32;
        let x1 = (p.cx + p.half) as i32;
        let gy = GROUND_Y as i32;
        let flash_on = (f.t * 24.0) as i32 % 2 == 0;
        if flash_on {
            c.rect_ring(x0 - 2, gy - 2, x1 - x0 + 5, 3, C_WHITE);
        }
        let label = format!("+{}", f.gained);
        draw_text_cx(c, p.cx as i32, gy - 25, &label, C_WHITE, 1);
        if f.life_up {
            draw_text_cx(c, p.cx as i32, gy - 9, "LIFE UP", C_GREEN, 1);
        }
    } else {
        draw_ship(c, g);
    }

    // Crash screen shake.
    if let Some(e) = &g.explosion {
        let t = (e.t / EXPLOSION_TIME).clamp(0.0, 1.0);
        let a = (1.0 - t) * 3.0;
        let dx = (a * (t * 91.0).sin()) as i32;
        let dy = (a * (t * 83.0).cos()) as i32;
        c.scroll(dx, dy);
    }
}

/// HUD drawn on top of the world (never shaken).
pub fn draw_hud(c: &mut Canvas, g: &Game) {
    let hi = g.hi_score.max(g.score);
    let score_line = format!("SCORE {:06}", g.score);
    draw_text(c, 3, 2, &score_line, C_WHITE, 1);
    let hi_line = format!("HI {:06}", hi);
    let hx = 280 - 3 - text_width(&hi_line, 1);
    draw_text(c, hx, 2, &hi_line, C_VIOLET, 1);

    let drop_line = format!("DROP {:02}  LIVES {}", g.landings + 1, g.lives);
    draw_text(c, 3, 10, &drop_line, C_GREEN, 1);
    // Small pad-colour swatches as a scoring hint.
    for (i, p) in PADS.iter().enumerate() {
        let x = 160 + i as i32 * 38;
        c.rect(x, 10, 34, 7, C_BLACK);
        c.rect(x + 1, 11, 10, 5, p.color);
        c.rect_ring(x + 1, 11, 10, 5, C_WHITE);
        draw_text(c, x + 14, 10, &format!("{}", p.points), p.color, 1);
        let _ = i;
    }
}

/// Draw everything for Playing / GameOver.
pub fn draw_game(c: &mut Canvas, g: &Game) {
    draw_world(c, g);
    draw_hud(c, g);
}

fn blink(clock: f32, period: f32) -> bool {
    (clock / period) as i32 % 2 == 0
}

// ---- Splash -----------------------------------------------------------------

fn splash_ship(c: &mut Canvas, clock: f32) {
    // Big stylised saucer.
    let cx = 140;
    let cy = 104;

    // Hull: violet disc with a white ring.
    c.ellipse(cx, cy, 57, 18, C_VIOLET);
    c.ellipse_ring(cx, cy, 54, 16, 2, C_WHITE);
    // Inner dome shadow.
    c.ellipse(cx, cy, 46, 13, C_VIOLET);

    // Dome: white outline + green glass.
    c.ellipse(cx, cy - 10, 26, 14, C_WHITE);
    c.ellipse(cx, cy - 10, 24, 12, C_GREEN);
    // A dark inner recess so it reads as glass.
    c.ellipse(cx, cy - 9, 17, 9, C_WHITE);
    c.ellipse(cx, cy - 9, 15, 7, C_BLUE);
    // Highlight crescent.
    c.ellipse(cx - 6, cy - 15, 8, 4, C_WHITE);
    c.ellipse(cx - 6, cy - 14, 6, 3, C_BLACK);
    c.ellipse(cx - 6, cy - 15, 9, 4, C_BLACK); // tidy

    // Window row along the hull.
    for (i, dx) in [-34i32, -22, -10, 2, 14, 26].iter().enumerate() {
        let w = if i % 2 == 0 { 8 } else { 14 };
        c.rect(cx + dx, cy - 2, w, 4, C_WHITE);
        c.rect(cx + dx + 1, cy - 1, (w - 2).max(1), 2, C_BLUE);
    }

    // Blinking running lights around the rim.
    for (i, ang) in [
        0.6f32,
        -0.6,
        std::f32::consts::PI - 0.4,
        std::f32::consts::PI + 0.4,
        1.2,
        -1.2,
    ]
    .iter()
    .enumerate()
    {
        let x = cx + (ang.cos() * 56.0) as i32;
        let y = cy + (ang.sin() * 17.0) as i32;
        let col = if blink(clock, 0.5 + i as f32 * 0.13) {
            C_ORANGE
        } else {
            C_WHITE
        };
        c.px(x, y, col);
    }

    // Engine glow below.
    if blink(clock, 0.22) {
        c.ellipse(cx, cy + 20, 12, 4, C_ORANGE);
        c.ellipse(cx, cy + 20, 8, 3, C_WHITE);
    }
}

pub fn draw_splash(c: &mut Canvas, g: &Game) {
    let clock = g.clock;
    // Starfield (the game's own star list).
    for s in &g.stars {
        if star_visible(clock, s.phase, s.speed) {
            c.px(s.x as i32, s.y as i32, s.color);
        }
    }

    // Corner brackets (drawn in white like a hi-res frame).
    for i in 0..14i32 {
        c.px(2 + i, 2, C_WHITE);
        c.px(2, 2 + i, C_WHITE);
        c.px(277 - i, 2, C_WHITE);
        c.px(277, 2 + i, C_WHITE);
        c.px(2 + i, 189, C_WHITE);
        c.px(2, 189 - i, C_WHITE);
        c.px(277 - i, 189, C_WHITE);
        c.px(277, 189 - i, C_WHITE);
    }

    // Title.
    draw_text_cx(c, 140, 14, "ALIEN SPACE DROP", C_WHITE, 2);
    draw_text_cx(c, 140, 33, "A 1980S APPLE ][ HIRES TRIBUTE", C_VIOLET, 1);

    // Big saucer.
    splash_ship(c, clock);

    // Message + controls.
    if blink(clock, 0.7) {
        draw_text_cx(c, 140, 158, "PRESS ANY KEY", C_WHITE, 1);
    }
    draw_text_cx(c, 140, 172, "A / S MOVE      D BOOST", C_GREEN, 1);
    draw_text_cx(
        c,
        140,
        184,
        "LAND ON THE PADS - SMALLER PAD = MORE POINTS",
        C_ORANGE,
        1,
    );
}

// ---- Game over --------------------------------------------------------------

pub fn draw_game_over(c: &mut Canvas, g: &Game) {
    // Dim the world with a hatched dark panel.
    for y in 58..142 {
        for x in 20..260 {
            if (x + y) % 4 == 0 {
                continue;
            }
            c.px(x, y, C_BLACK);
        }
    }
    c.rect_ring(20, 58, 240, 84, C_WHITE);

    draw_text_cx(c, 140, 66, "GAME OVER", C_WHITE, 2);
    let score_line = format!("SCORE  {:06}", g.score);
    draw_text_cx(c, 140, 86, &score_line, C_WHITE, 1);
    let hi_line = format!("HI     {:06}", g.hi_score);
    draw_text_cx(c, 140, 95, &hi_line, C_VIOLET, 1);
    if g.new_high {
        draw_text_cx(c, 140, 106, "NEW HIGH SCORE!", C_ORANGE, 1);
    }
    if blink(g.clock, 0.6) {
        draw_text_cx(c, 140, 124, "PRESS SPACE TO PLAY AGAIN", C_GREEN, 1);
    }
}

// ---- Frame composition for the headless --save path -------------------------

/// Render a full frame for a given mode.
pub fn render_frame(g: &Game) -> Canvas {
    let mut c = Canvas::new(crate::game::GAME_W as usize, crate::game::GAME_H as usize);
    match g.mode {
        crate::game::Mode::Splash => draw_splash(&mut c, g),
        crate::game::Mode::Playing | crate::game::Mode::GameOver => {
            draw_game(&mut c, g);
            if g.mode == crate::game::Mode::GameOver {
                draw_game_over(&mut c, g);
            }
        }
    }
    c
}
