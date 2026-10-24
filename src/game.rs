//! Pure, deterministic game simulation for ALIEN SPACE DROP.
//!
//! No rendering or input-device dependencies live here: the app drives
//! [`Game::update`] each frame with the set of held keys, and the returned
//! [`Sfx`] events are turned into beeps by the audio layer.

/// Playfield width in hi-res pixels.
pub const GAME_W: f32 = 280.0;
/// Playfield height in hi-res pixels.
pub const GAME_H: f32 = 192.0;

/// The ground strip starts here (the three landing pads sit below it).
pub const GROUND_Y: f32 = 168.0;

/// Player saucer dimensions (matches the sprite in the renderer).
pub const SHIP_W: f32 = 22.0;
pub const SHIP_H: f32 = 14.0;

/// Descent speed (hi-res px per second) without/with the BOOST key.
pub const BASE_FALL: f32 = 62.0;
pub const BOOST_FALL: f32 = 205.0;
/// Lateral movement speed.
pub const MOVE_SPEED: f32 = 115.0;

/// Starting lives.
pub const START_LIVES: i32 = 3;
/// Upper bound on lives (a fifth landing gives back a life later).
pub const MAX_LIVES: i32 = 5;

/// Seconds the explosion plays before the next ship spawns / game over.
pub const EXPLOSION_TIME: f32 = 0.9;
/// Seconds a successful landing lingers before the next drop begins.
pub const FLASH_TIME: f32 = 0.9;

/// Front-end palette (indexes into the renderer's Apple II palette).
pub const C_BLACK: u8 = 0;
pub const C_WHITE: u8 = 1;
pub const C_VIOLET: u8 = 2;
pub const C_GREEN: u8 = 3;
pub const C_BLUE: u8 = 4;
pub const C_ORANGE: u8 = 5;

/// A landing pad. `half` is the half width and the landing target: the ship
/// scores the pad's points if its centre lines up within `half`.
#[derive(Clone, Copy)]
pub struct Pad {
    pub cx: f32,
    pub half: f32,
    pub points: u32,
    pub color: u8,
}

/// The three landing pads: wide/violet = easy but cheap, tiny/orange = hard but
/// worth the most points.
pub const PADS: [Pad; 3] = [
    Pad {
        cx: 50.0,
        half: 27.0,
        points: 10,
        color: C_VIOLET,
    },
    Pad {
        cx: 140.0,
        half: 14.0,
        points: 25,
        color: C_GREEN,
    },
    Pad {
        cx: 230.0,
        half: 6.0,
        points: 50,
        color: C_ORANGE,
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Splash,
    Playing,
    GameOver,
}

/// Which keys the player is holding this frame.
#[derive(Clone, Copy, Default)]
pub struct Input {
    pub left: bool,
    pub right: bool,
    pub boost: bool,
}

/// Short beep effects the game can ask for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sfx {
    Start,
    Land { pad: usize },
    Crash,
    LifeUp,
}

/// The descending player ship. `x` is the horizontal centre, `y` the top edge.
#[derive(Clone, Copy)]
pub struct Ship {
    pub x: f32,
    pub y: f32,
    pub boosting: bool,
}

/// A horizontally flying obstacle ship.
pub struct Obstacle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub kind: u8,
}

pub const OB_W: f32 = 20.0;
pub const OB_H: f32 = 8.0;

/// An expanding crash fireball.
#[derive(Clone, Copy)]
pub struct Explosion {
    pub x: f32,
    pub y: f32,
    pub t: f32,
}

/// The "you landed" celebration (beacon + "+points" popup) before the next drop.
pub struct PadFlash {
    pub pad: usize,
    pub t: f32,
    pub gained: u32,
    pub life_up: bool,
}

/// A twinkling background star.
pub struct Star {
    pub x: f32,
    pub y: f32,
    pub phase: f32,
    pub speed: f32,
    pub color: u8,
}

/// All mutable game state. Constructed with [`Game::new`], started with
/// [`Game::start`].
pub struct Game {
    pub mode: Mode,
    pub score: u32,
    pub hi_score: u32,
    pub lives: i32,
    pub landings: u32,
    pub level: u32,
    pub ship: Ship,
    pub obstacles: Vec<Obstacle>,
    pub explosion: Option<Explosion>,
    pub flash: Option<PadFlash>,
    pub stars: Vec<Star>,
    pub clock: f32,
    /// Set at game over when the run beat the pre-run high score.
    pub new_high: bool,
    spawn_timer: f32,
    start_hi: u32,
    rng: u64,
}

impl Game {
    /// A fresh game sat on the splash screen.
    pub fn new(seed: u64) -> Game {
        let mut g = Game {
            mode: Mode::Splash,
            score: 0,
            hi_score: 0,
            lives: START_LIVES,
            landings: 0,
            level: 1,
            ship: Ship {
                x: GAME_W / 2.0,
                y: -SHIP_H,
                boosting: false,
            },
            obstacles: Vec::new(),
            explosion: None,
            flash: None,
            stars: Vec::new(),
            clock: 0.0,
            new_high: false,
            spawn_timer: 1.2,
            start_hi: 0,
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).max(1),
        };
        for _ in 0..40u32 {
            let x = g.gen_range(0.0, GAME_W);
            let y = g.gen_range(0.0, GROUND_Y);
            let phase = g.gen_range(0.0, 12.0);
            let speed = 0.6 + g.gen_f() * 2.4;
            let color = match g.gen_index(6) {
                0 => C_VIOLET,
                1 => C_BLUE,
                2 => C_ORANGE,
                _ => C_WHITE,
            };
            g.stars.push(Star {
                x,
                y,
                phase,
                speed,
                color,
            });
        }
        g
    }

    /// Start a fresh run from the splash / game-over screens.
    pub fn start(&mut self) {
        self.score = 0;
        self.start_hi = self.hi_score;
        self.lives = START_LIVES;
        self.landings = 0;
        self.level = 1;
        self.mode = Mode::Playing;
        self.obstacles.clear();
        self.explosion = None;
        self.flash = None;
        self.new_high = false;
        self.spawn_timer = 1.2;
        self.respawn_ship();
    }

    /// Advance the simulation by `dt` seconds.
    pub fn update(&mut self, dt: f32, inp: &Input, sfx: &mut Vec<Sfx>) {
        if dt <= 0.0 || self.mode != Mode::Playing {
            return;
        }
        self.clock += dt;

        // Progress the explosion / landing celebration timers first. While one
        // is active the player is between drops and cannot steer.
        if let Some(e) = &mut self.explosion {
            e.t += dt;
            if e.t >= EXPLOSION_TIME {
                self.explosion = None;
                self.after_crash(sfx);
            }
        } else if let Some(f) = &mut self.flash {
            f.t += dt;
            if f.t >= FLASH_TIME {
                self.flash = None;
                self.respawn_ship();
            }
        }
        let between_drops = self.explosion.is_some() || self.flash.is_some();
        let ship_visible = !between_drops;

        // Obstacles keep flying during the in-between moments.
        self.move_obstacles(dt);

        if ship_visible {
            self.maybe_spawn_obstacle(dt);

            if inp.left && !inp.right {
                self.ship.x -= MOVE_SPEED * dt;
            } else if inp.right && !inp.left {
                self.ship.x += MOVE_SPEED * dt;
            }
            self.ship.x = self
                .ship
                .x
                .clamp(SHIP_W / 2.0 - 2.0, GAME_W - (SHIP_W / 2.0 - 2.0));

            self.ship.boosting = inp.boost;
            let fall = if inp.boost {
                BOOST_FALL
            } else {
                BASE_FALL + 5.0 * (self.level.saturating_sub(1)) as f32
            };
            self.ship.y += fall * dt;

            // Did we clip an obstacle?
            for o in &self.obstacles {
                if ship_hits_obstacle(&self.ship, o) {
                    self.crash(sfx);
                    return;
                }
            }

            // Reached the landing strip?
            if self.ship.y + SHIP_H >= GROUND_Y {
                self.ship.y = GROUND_Y - SHIP_H;
                let cy = self.ship.x;
                let mut hit: Option<usize> = None;
                for (i, p) in PADS.iter().enumerate() {
                    if (cy - p.cx).abs() <= p.half + 1.0 {
                        hit = Some(i);
                        break;
                    }
                }
                match hit {
                    Some(i) => self.land(i, sfx),
                    None => self.crash(sfx),
                }
            }
        }
    }

    fn move_obstacles(&mut self, dt: f32) {
        for o in &mut self.obstacles {
            o.x += o.vx * dt;
        }
        self.obstacles
            .retain(|o| o.x > -OB_W - 30.0 && o.x < GAME_W + 30.0);
    }

    fn maybe_spawn_obstacle(&mut self, dt: f32) {
        self.spawn_timer -= dt;
        if self.spawn_timer > 0.0 {
            return;
        }
        self.spawn_timer = self.spawn_interval();
        let cap = (1 + self.level as usize + self.landings as usize / 3).min(7);
        if self.obstacles.len() >= cap {
            return;
        }
        let dir: f32 = if self.gen_f() < 0.5 { -1.0 } else { 1.0 };
        let base = 22.0 + 5.0 * (self.level.saturating_sub(1)) as f32;
        let vx = dir * (base + self.gen_f() * 14.0);
        let kind = self.gen_index(3) as u8;

        // Pick a row, but retry if it would appear right on top of the ship.
        let ship_mid = self.ship.y + SHIP_H / 2.0;
        let mut y = 14.0;
        for _ in 0..8 {
            y = 14.0 + self.gen_f() * (GROUND_Y - 40.0 - 14.0);
            if (y - ship_mid).abs() > 30.0 {
                break;
            }
        }
        let x = if dir < 0.0 {
            GAME_W + 20.0
        } else {
            -20.0 - OB_W
        };
        self.obstacles.push(Obstacle { x, y, vx, kind });
    }

    fn spawn_interval(&self) -> f32 {
        (2.4 - 0.3 * (self.level.saturating_sub(1)) as f32).clamp(0.9, 2.4)
    }

    fn crash(&mut self, sfx: &mut Vec<Sfx>) {
        self.lives -= 1;
        self.explosion = Some(Explosion {
            x: self.ship.x,
            y: self.ship.y + SHIP_H / 2.0,
            t: 0.0,
        });
        sfx.push(Sfx::Crash);
    }

    fn after_crash(&mut self, sfx: &mut Vec<Sfx>) {
        if self.lives <= 0 {
            self.new_high = self.score > 0 && self.score >= self.start_hi;
            if self.score > self.hi_score {
                self.hi_score = self.score;
            }
            self.mode = Mode::GameOver;
            sfx.push(Sfx::Start); // little "end jingle" marker reuses the rising notes
            let _ = sfx.len();
        } else {
            self.respawn_ship();
        }
    }

    fn land(&mut self, pad: usize, sfx: &mut Vec<Sfx>) {
        let p = &PADS[pad];
        let gained = p.points * self.level;
        self.score += gained;
        if self.score > self.hi_score {
            self.hi_score = self.score;
        }
        self.landings += 1;
        let mut life_up = false;
        if self.landings.is_multiple_of(5) && self.lives < MAX_LIVES {
            self.lives += 1;
            life_up = true;
            sfx.push(Sfx::LifeUp);
        }
        self.level = 1 + self.landings / 4;
        self.flash = Some(PadFlash {
            pad,
            t: 0.0,
            gained,
            life_up,
        });
        sfx.push(Sfx::Land { pad });
    }

    /// Put a fresh ship at the top of the screen on a new horizontal position.
    fn respawn_ship(&mut self) {
        self.ship.y = -SHIP_H;
        self.ship.boosting = false;
        self.ship.x = self.gen_range(
            SHIP_W / 2.0 - 2.0 + 8.0,
            GAME_W - (SHIP_W / 2.0 - 2.0) - 8.0,
        );
    }

    // ---- xorshift64* rng ---------------------------------------------------

    fn next_u64(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn gen_f(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / ((1u64 << 24) as f32)
    }

    fn gen_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.gen_f() * (hi - lo)
    }

    fn gen_index(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() as usize % n).min(n - 1)
    }
}

/// Collision test between the player ship and an obstacle.
fn ship_hits_obstacle(ship: &Ship, o: &Obstacle) -> bool {
    let sx0 = ship.x - SHIP_W / 2.0 + 3.0;
    let sx1 = ship.x + SHIP_W / 2.0 - 3.0;
    let sy0 = ship.y + 1.0;
    let sy1 = ship.y + SHIP_H - 1.0;
    let ox0 = o.x + 4.0;
    let ox1 = o.x + OB_W - 4.0;
    let oy0 = o.y + 2.0;
    let oy1 = o.y + OB_H - 2.0;
    sx0 < ox1 && sx1 > ox0 && sy0 < oy1 && sy1 > oy0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(seed: u64) -> Game {
        Game::new(seed)
    }

    #[test]
    fn starts_in_splash_mode() {
        assert_eq!(game(1).mode, Mode::Splash);
    }

    #[test]
    fn start_puts_ship_above_the_top() {
        let mut g = game(1);
        g.start();
        assert_eq!(g.mode, Mode::Playing);
        assert!(g.ship.y <= -1.0);
        assert_eq!(g.lives, START_LIVES);
    }

    #[test]
    fn left_and_right_clamp_at_screen_edges() {
        let mut g = game(1);
        g.start();
        let mut sfx = Vec::new();
        // Hold left for a very long time.
        for _ in 0..600 {
            g.update(
                1.0 / 60.0,
                &Input {
                    left: true,
                    ..Input::default()
                },
                &mut sfx,
            );
        }
        assert!(g.ship.x >= SHIP_W / 2.0 - 2.0);
        for _ in 0..1200 {
            g.update(
                1.0 / 60.0,
                &Input {
                    right: true,
                    ..Input::default()
                },
                &mut sfx,
            );
        }
        assert!(g.ship.x <= GAME_W - (SHIP_W / 2.0 - 2.0));
    }

    #[test]
    fn boost_falls_faster_than_idle() {
        let mut g = game(1);
        g.start();
        g.update(0.5, &Input::default(), &mut Vec::new());
        let idle_y = g.ship.y;
        g.explosion = None;
        g.flash = None;
        g.ship.y = -SHIP_H;
        g.ship.x = 140.0;
        g.obstacles.clear();
        g.update(
            0.5,
            &Input {
                boost: true,
                ..Input::default()
            },
            &mut Vec::new(),
        );
        let boost_y = g.ship.y;
        assert!(
            boost_y > idle_y,
            "boost should descend further per second ({idle_y} idle vs {boost_y} boosted)"
        );
    }

    #[test]
    fn landing_on_the_wide_pad_scores_points() {
        let mut g = game(1);
        g.start();
        let mut sfx = Vec::new();
        // Aim at the wide pad.
        g.ship.x = PADS[0].cx;
        // Let it fall to the ground.
        let mut guard = 0;
        while g.flash.is_none() && guard < 600 {
            g.update(1.0 / 60.0, &Input::default(), &mut sfx);
            guard += 1;
        }
        assert!(g.flash.is_some(), "should have landed");
        assert_eq!(g.lives, START_LIVES);
        assert_eq!(g.landings, 1);
        assert_eq!(g.score, PADS[0].points * 1);
        assert!(sfx.contains(&Sfx::Land { pad: 0 }));
    }

    #[test]
    fn hitting_ground_between_pads_crashes() {
        let mut g = game(1);
        g.start();
        // Position between pad 0 and pad 1 where neither pad reaches.
        g.ship.x = PADS[0].cx + PADS[0].half + 4.0;
        let mut sfx = Vec::new();
        let mut guard = 0;
        while g.explosion.is_none() && guard < 600 {
            g.update(1.0 / 60.0, &Input::default(), &mut sfx);
            guard += 1;
        }
        assert!(g.explosion.is_some(), "should have crashed");
        assert_eq!(g.lives, START_LIVES - 1);
        assert!(sfx.contains(&Sfx::Crash));
    }

    #[test]
    fn collision_with_obstacle_kills_a_life() {
        let mut g = game(1);
        g.start();
        g.obstacles.push(Obstacle {
            x: 10.0,
            y: 20.0,
            vx: 0.0,
            kind: 0,
        });
        g.ship.x = 20.0;
        g.ship.y = 17.0;
        let mut sfx = Vec::new();
        g.update(1.0 / 60.0, &Input::default(), &mut sfx);
        assert!(g.explosion.is_some());
        assert_eq!(g.lives, START_LIVES - 1);
    }

    #[test]
    fn losing_all_lives_ends_the_game() {
        let mut g = game(1);
        g.start();
        g.lives = 1;
        g.obstacles.push(Obstacle {
            x: 10.0,
            y: 20.0,
            vx: 0.0,
            kind: 0,
        });
        g.ship.x = 20.0;
        g.ship.y = 17.0;
        let mut sfx = Vec::new();
        // Let the explosion finish.
        for _ in 0..90 {
            g.update(1.0 / 60.0, &Input::default(), &mut sfx);
        }
        assert_eq!(g.mode, Mode::GameOver);
        assert!(g.lives <= 0);
    }

    #[test]
    fn smaller_pads_score_more() {
        assert!(PADS[0].points < PADS[1].points && PADS[1].points < PADS[2].points);
        assert!(PADS[0].half > PADS[1].half && PADS[1].half > PADS[2].half);
    }

    #[test]
    fn explosion_and_flash_interleave_drops() {
        let mut g = game(1);
        g.start();
        let mut sfx = Vec::new();
        // Keep the ship locked over the wide pad and clear drones every frame
        // so nothing can crash us: we should rack up many landings in a row.
        let mut guard = 0;
        while g.landings < 3 && guard < 60 * 60 {
            g.obstacles.clear();
            g.ship.x = PADS[0].cx;
            g.update(1.0 / 60.0, &Input::default(), &mut sfx);
            guard += 1;
        }
        assert_eq!(g.mode, Mode::Playing, "should still be playing");
        assert_eq!(g.lives, START_LIVES, "no crashes allowed");
        assert!(g.landings >= 3, "should have landed repeatedly in 60s");
    }
}
