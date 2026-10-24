//! ALIEN SPACE DROP — a 1980s Apple II hi-res style arcade game.
//!
//! The window shows a 280x192 hi-res framebuffer upscaled 3x through a CRT-ish
//! shader (scanlines + RGB phosphor stripes). The pure simulation lives in
//! `game`, drawing in `render`, and the beeper output in `audio`.
//!
//! Controls (Apple II-faithful simple keys):
//!   A = move left, S = move right, D = boost descent (or arrow keys).
//!   SPACE / ENTER = start or restart. ESC = quit.

mod audio;
mod game;
mod render;

use game::{Game, Input, Sfx};
use miniquad::conf::Conf;
use miniquad::*;
#[cfg(not(target_arch = "wasm32"))]
use render::Canvas;
use std::collections::HashSet;

const SCREEN_W: usize = 280;
const SCREEN_H: usize = 192;
const SCALE: i32 = 3;

const VERT: &str = r#"#version 100
attribute vec2 in_pos;
attribute vec2 in_uv;
varying lowp vec2 texcoord;
void main() {
    gl_Position = vec4(in_pos, 0.0, 1.0);
    texcoord = in_uv;
}
"#;

// CRT-ish fragment shader: masks the nearest-scaled texel with a scanline
// shadow on the first of every 3 rows and a faint RGB phosphor stripe mask
// across each logical pixel column.
const FRAG: &str = r#"#version 100
precision mediump float;
varying lowp vec2 texcoord;
uniform sampler2D tex;
void main() {
    vec4 c = texture2D(tex, texcoord);
    vec2 f = floor(gl_FragCoord.xy);
    float scan = smoothstep(0.0, 0.55, mod(f.y, 3.0));
    float shx = mod(f.x, 3.0);
    vec3 stripe = vec3(
        shx < 1.0 ? 1.16 : 0.92,
        shx >= 1.0 && shx < 2.0 ? 1.16 : 0.92,
        shx >= 2.0 ? 1.16 : 0.92
    );
    c.rgb = clamp(c.rgb * (0.72 + 0.28 * scan) * stripe, 0.0, 1.0);
    gl_FragColor = c;
}
"#;

#[repr(C)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

struct Stage {
    ctx: Box<dyn RenderingBackend>,
    pipeline: Pipeline,
    index_buffer: BufferId,
    game: Game,
    audio: audio::SfxOut,
    pressed: HashSet<KeyCode>,
    texture: Option<TextureId>,
    last_time: f64,
}

impl Stage {
    fn new(seed: u64) -> Stage {
        let mut ctx: Box<dyn RenderingBackend> = window::new_rendering_backend();

        let shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: VERT,
                    fragment: FRAG,
                },
                ShaderMeta {
                    images: vec!["tex".to_string()],
                    uniforms: UniformBlockLayout { uniforms: vec![] },
                },
            )
            .unwrap();
        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float2),
                VertexAttribute::new("in_uv", VertexFormat::Float2),
            ],
            shader,
            PipelineParams::default(),
        );

        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&indices),
        );

        let now = miniquad::date::now();
        let mut game = Game::new(seed);
        // Jump straight into gameplay when launched with --play (dev aid).
        if std::env::args().any(|a| a == "--play") {
            game.start();
        }

        Stage {
            ctx,
            pipeline,
            index_buffer,
            game,
            audio: audio::init(),
            pressed: HashSet::new(),
            texture: None,
            last_time: now,
        }
    }

    fn play_sfx(&self, sfx: Sfx) {
        self.audio.play(audio::tones_for(sfx));
    }
}

impl EventHandler for Stage {
    fn update(&mut self) {
        let now = miniquad::date::now();
        let dt = ((now - self.last_time) as f32).clamp(0.0, 0.1);
        self.last_time = now;

        // Any of the obvious keys leaves the splash; Space/Enter restarts after
        // game over. ESC quits the whole game.
        let start_key = {
            let any = |k: KeyCode| self.pressed.contains(&k);
            any(KeyCode::Space) || any(KeyCode::Enter)
        };
        let skip_key = {
            let any = |k: KeyCode| self.pressed.contains(&k);
            any(KeyCode::A) || any(KeyCode::S) || any(KeyCode::D)
        };

        match self.game.mode {
            game::Mode::Splash => {
                if start_key || skip_key {
                    self.game.start();
                    self.play_sfx(Sfx::Start);
                }
            }
            game::Mode::GameOver => {
                if start_key {
                    self.game.start();
                    self.play_sfx(Sfx::Start);
                }
            }
            game::Mode::Playing => {}
        }

        if self.pressed.contains(&KeyCode::Escape) {
            window::quit();
            return;
        }

        let inp = Input {
            left: self.pressed.contains(&KeyCode::A) || self.pressed.contains(&KeyCode::Left),
            right: self.pressed.contains(&KeyCode::S) || self.pressed.contains(&KeyCode::Right),
            boost: self.pressed.contains(&KeyCode::D) || self.pressed.contains(&KeyCode::Down),
        };

        let mut sfx = Vec::new();
        self.game.update(dt, &inp, &mut sfx);
        for s in sfx {
            self.play_sfx(s);
        }
    }

    fn draw(&mut self) {
        self.ctx
            .begin_default_pass(PassAction::clear_color(0.0, 0.0, 0.0, 1.0));

        // Full-screen quad, fixed clip space.
        let vertices = [
            Vertex {
                pos: [-1.0, -1.0],
                uv: [0.0, 1.0],
            },
            Vertex {
                pos: [1.0, -1.0],
                uv: [1.0, 1.0],
            },
            Vertex {
                pos: [1.0, 1.0],
                uv: [1.0, 0.0],
            },
            Vertex {
                pos: [-1.0, 1.0],
                uv: [0.0, 0.0],
            },
        ];
        let vertex_buffer = self.ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&vertices),
        );

        // Composite the current frame.
        let frame = render::render_frame(&self.game);
        // A (fast, cheap) implicit resize — the canvas struct is re-used as a
        // scratch buffer for the screenshot path; here we just need the RGBA.
        let rgba = frame.to_rgba();

        if let Some(tex) = self.texture.take() {
            self.ctx.delete_texture(tex);
        }
        let texture = self
            .ctx
            .new_texture_from_rgba8(SCREEN_W as u16, SCREEN_H as u16, &rgba);
        self.ctx
            .texture_set_filter(texture, FilterMode::Nearest, MipmapFilterMode::None);
        self.texture = Some(texture);

        self.ctx.apply_pipeline(&self.pipeline);
        let bindings = Bindings {
            vertex_buffers: vec![vertex_buffer],
            index_buffer: self.index_buffer,
            images: vec![texture],
        };
        self.ctx.apply_bindings(&bindings);
        self.ctx.draw(0, 6, 1);

        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }

    fn key_down_event(&mut self, keycode: KeyCode, _keymods: KeyMods, _repeat: bool) {
        self.pressed.insert(keycode);
    }

    fn key_up_event(&mut self, keycode: KeyCode, _keymods: KeyMods) {
        self.pressed.remove(&keycode);
    }
}

/// Headless simulation: build a game, optionally play some seconds, render a
/// frame, and write it as a PNG. Used by `--save`.
#[cfg(not(target_arch = "wasm32"))]
fn write_png(path: &str, canvas: &Canvas) {
    use std::io::Write;
    let rgba = canvas.to_rgba();
    let mut encoder = png::Encoder::new(
        std::fs::File::create(path).expect("create png"),
        canvas.w as u32,
        canvas.h as u32,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .expect("png header")
        .into_stream_writer()
        .expect("stream writer");
    writer.write_all(&rgba).expect("png write");
    writer.finish().expect("png finish");
}

/// Full-resolution ASCII dump of a canvas sub-rectangle for precise sprite
/// inspection.
#[cfg(not(target_arch = "wasm32"))]
fn dump_region(c: &Canvas, x: usize, y: usize, w: usize, h: usize) {
    use game::{C_BLUE, C_GREEN, C_ORANGE, C_VIOLET, C_WHITE};
    let ch = |ci: u8| match ci {
        C_WHITE => 'W',
        C_VIOLET => 'V',
        C_GREEN => 'G',
        C_BLUE => 'B',
        C_ORANGE => 'O',
        _ => '.',
    };
    for ry in y..(y + h).min(c.h) {
        let mut line = String::new();
        for rx in x..(x + w).min(c.w) {
            line.push(ch(c.buf[ry * c.w + rx]));
        }
        println!("{line}");
    }
}

/// Print a 2x-downsampled colour-map of the canvas as ASCII for headless
/// visual inspection (no image viewer needed).
#[cfg(not(target_arch = "wasm32"))]
fn dump_ascii(c: &Canvas) {
    use game::{C_BLUE, C_GREEN, C_ORANGE, C_VIOLET, C_WHITE};
    let ch = |ci: u8| match ci {
        C_WHITE => 'W',
        C_VIOLET => 'V',
        C_GREEN => 'G',
        C_BLUE => 'B',
        C_ORANGE => 'O',
        _ => '.',
    };
    for y in (0..c.h).step_by(2) {
        let mut line = String::new();
        for x in (0..c.w).step_by(2) {
            line.push(ch(c.buf[y * c.w + x]));
        }
        println!("{line}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Desktop-only verification: render a frame to PNG / ASCII and exit.
    if run_headless(&args) {
        return;
    }

    let seed = {
        let bits = (miniquad::date::now() * 1_000_000.0) as u64;
        (bits >> 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
    };
    miniquad::start(
        Conf {
            window_title: "ALIEN SPACE DROP - Apple II hi-res tribute".to_string(),
            window_width: (SCREEN_W * SCALE as usize) as i32,
            window_height: (SCREEN_H * SCALE as usize) as i32,
            high_dpi: false,
            ..Default::default()
        },
        move || Box::new(Stage::new(seed)),
    );
}

/// Desktop-only headless helpers (`--save`, `--ascii`, `--region`). Parses the
/// command line and, when a render request is recognised, writes the output and
/// returns `true`. Returns `false` to start the real window instead.
#[cfg(not(target_arch = "wasm32"))]
fn run_headless(args: &[String]) -> bool {
    let mut headless: Option<String> = None;
    let mut play = false;
    let mut gameover = false;
    let mut seconds: f32 = 0.0;
    let mut ascii = false;
    let mut hold = false;
    let mut ship_pos: Option<(f32, f32)> = None;
    let mut flash: Option<usize> = None;
    let mut boom: Option<(f32, f32)> = None;
    let mut region: Option<(usize, usize, usize, usize)> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--save" => {
                if i + 1 < args.len() {
                    headless = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--ascii" => ascii = true,
            "--region" => {
                if i + 4 < args.len() {
                    region = Some((
                        args[i + 1].parse().unwrap_or(0),
                        args[i + 2].parse().unwrap_or(0),
                        args[i + 3].parse().unwrap_or(0),
                        args[i + 4].parse().unwrap_or(0),
                    ));
                    i += 4;
                }
            }
            "--playing" => play = true,
            "--gameover" => gameover = true,
            "--hold" => hold = true,
            "--ship" => {
                if i + 2 < args.len() {
                    ship_pos = Some((
                        args[i + 1].parse().unwrap_or(0.0),
                        args[i + 2].parse().unwrap_or(0.0),
                    ));
                    i += 2;
                }
            }
            "--flash" => {
                if i + 1 < args.len() {
                    flash = Some(args[i + 1].parse().unwrap_or(0));
                    i += 1;
                }
            }
            "--boom" => {
                if i + 2 < args.len() {
                    boom = Some((
                        args[i + 1].parse().unwrap_or(0.0),
                        args[i + 2].parse().unwrap_or(0.0),
                    ));
                    i += 2;
                }
            }
            "--t" => {
                if i + 1 < args.len() {
                    seconds = args[i + 1].parse().unwrap_or(0.0);
                    i += 1;
                }
            }
            "--play" => {}
            _ => {}
        }
        i += 1;
    }

    if let Some(path) = headless {
        let mut g = Game::new(0xABCD);
        let mut s: Vec<Sfx> = Vec::new();
        if play || gameover || hold {
            g.start();
            let dt = 1.0 / 60.0;
            if !hold {
                // Float a bit with a mild drift so obstacles show up on screen.
                for n in 0..(seconds * 60.0) as u32 {
                    let inp = Input {
                        right: (n / 40) % 2 == 0,
                        boost: n % 180 == 0,
                        ..Input::default()
                    };
                    g.update(dt, &inp, &mut s);
                }
            }
            if let Some((x, y)) = ship_pos {
                g.ship.x = x;
                g.ship.y = y;
            }
            if let Some(p) = flash {
                g.lives = game::START_LIVES;
                g.landings = 3;
                g.level = 1 + g.landings / 4;
                g.score = game::PADS[p].points;
                g.ship.y = game::GROUND_Y - game::SHIP_H;
                g.ship.x = game::PADS[p].cx;
                g.flash = Some(game::PadFlash {
                    pad: p,
                    t: 0.0,
                    gained: game::PADS[p].points,
                    life_up: p == 0,
                });
            }
            if let Some((x, y)) = boom {
                g.explosion = Some(game::Explosion { x, y, t: 0.4 });
            }
            if gameover {
                // Crash into the ship's own lane to force a game over.
                g.lives = 1;
                g.obstacles.push(game::Obstacle {
                    x: g.ship.x - game::SHIP_W / 2.0,
                    y: (g.ship.y + 2.0).max(4.0),
                    vx: 0.0,
                    kind: 0,
                });
                for _ in 0..120 {
                    g.update(dt, &Input::default(), &mut s);
                }
            }
        }
        let canvas = render::render_frame(&g);
        if let Some((x, y, w, h)) = region {
            dump_region(&canvas, x, y, w, h);
        } else if ascii {
            dump_ascii(&canvas);
        } else {
            write_png(&path, &canvas);
            println!("wrote {path} (mode={:?}, t={seconds:.1}s)", g.mode);
        }
        return true;
    }
    false
}

#[cfg(target_arch = "wasm32")]
fn run_headless(_args: &[String]) -> bool {
    false
}
