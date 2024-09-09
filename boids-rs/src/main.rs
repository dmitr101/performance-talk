use std::cell::RefCell;

use ggez::event::{self, EventHandler};
use ggez::graphics::{self, Color, DrawParam, Text};
use ggez::input::keyboard::KeyCode;
use ggez::{Context, ContextBuilder, GameResult};
use glam::Vec2;
use rand::{Rng, SeedableRng};
use std::env;

const BOID_SIZE: f32 = 10.0;
const MAX_SPEED: f32 = 100.0;
const MAX_FORCE: f32 = 80.0;
const PERCEPTION: f32 = 100.0;
const SEPARATION: f32 = 100.0;

macro_rules! tracy_scope {
    ($name:literal) => {
        let _tracy_span = tracy_client::span!($name);
    };
}

#[repr(C)]
struct Boid {
    position: Vec2,
    velocity: Vec2,
    acceleration: Vec2,

    #[cfg(not(feature = "no_life_history"))]
    life_history: [i32; 512],
}

type BoidCell = RefCell<Boid>;

#[cfg(not(feature = "no_boxing"))]
type BoidRef = Box<BoidCell>;

#[cfg(feature = "no_boxing")]
type BoidRef = BoidCell;

impl Boid {
    fn new(position: Vec2, velocity: Vec2) -> Self {
        Boid {
            position,
            velocity,
            acceleration: Vec2::ZERO,

            #[cfg(not(feature = "no_life_history"))]
            life_history: [0; 512],
        }
    }

    #[cfg(not(feature = "pre_square"))]
    #[inline(always)]
    fn is_close_enough(&self, other: &Boid, max_dist: f32) -> bool {
        let distance = self.position.distance(other.position);
        distance < max_dist && distance > 0.0
    }

    #[cfg(feature = "pre_square")]
    #[inline(always)]
    fn is_close_enough(&self, other: &Boid, max_dist: f32) -> bool {
        let distance = self.position.distance_squared(other.position);
        distance < (max_dist * max_dist) && distance > 0.0
    }

    #[inline(never)]
    fn alignment(&self, boids: &[BoidRef], self_idx: usize) -> Vec2 {
        let mut alignment = Vec2::ZERO;
        let mut total = 0;

        for other_idx in 0..boids.len() {
            if other_idx == self_idx {
                continue;
            }

            let other = boids[other_idx].borrow();
            if self.is_close_enough(&other, PERCEPTION) {
                alignment += other.velocity;
                total += 1;
            }
        }

        if total > 0 {
            alignment /= total as f32;
            alignment = alignment.normalize() * MAX_SPEED;
            alignment -= self.velocity;
            alignment = alignment.clamp_length_max(MAX_FORCE);
        }
        alignment
    }

    #[inline(never)]
    fn cohesion(&self, boids: &[BoidRef], self_idx: usize) -> Vec2 {
        let mut cohesion = Vec2::ZERO;
        let mut total = 0;

        for other_idx in 0..boids.len() {
            if other_idx == self_idx {
                continue;
            }

            let other = boids[other_idx].borrow();
            if self.is_close_enough(&other, PERCEPTION) {
                cohesion += other.position;
                total += 1;
            }
        }

        if total > 0 {
            cohesion /= total as f32;
            cohesion -= self.position;
            cohesion = cohesion.normalize() * MAX_SPEED;
            cohesion -= self.velocity;
            cohesion = cohesion.clamp_length_max(MAX_FORCE);
        }

        cohesion
    }

    #[inline(never)]
    fn separation(&self, boids: &[BoidRef], self_idx: usize) -> Vec2 {
        let mut separation = Vec2::ZERO;
        let mut total_separation = 0;

        for other_idx in 0..boids.len() {
            if other_idx == self_idx {
                continue;
            }

            let other = boids[other_idx].borrow();
            let distance = self.position.distance(other.position);

            if distance < SEPARATION && distance > 0.0 {
                let diff = (self.position - other.position).normalize() / distance;
                separation += diff;
                total_separation += 1;
            }
        }

        if total_separation > 0 {
            separation /= total_separation as f32;
            separation = separation.normalize() * MAX_SPEED;
            separation -= self.velocity;
            separation = separation.clamp_length_max(MAX_FORCE);
        }

        separation
    }

    #[inline(never)]
    fn apply_behavior(
        &mut self,
        self_idx: usize,
        boids: &[BoidRef],
        mouse_pos: Vec2,
        is_attracted: bool,
    ) {
        let alignment = self.alignment(boids, self_idx);
        let cohesion = self.cohesion(boids, self_idx);
        let separation = self.separation(boids, self_idx);

        self.acceleration = alignment;
        self.acceleration += cohesion;
        self.acceleration += separation;

        if is_attracted {
            let attraction = (mouse_pos - self.position).normalize() * MAX_SPEED;
            self.acceleration += attraction;
        }
        assert!(self.acceleration.is_finite());
    }

    fn update(&mut self, dt: f32, rng: &mut rand_chacha::ChaCha8Rng) {
        let this_frame_acceleration = std::hint::black_box(self.acceleration * dt);
        #[cfg(feature = "static_update")]
        let this_frame_acceleration = Vec2::ZERO;

        self.velocity += this_frame_acceleration;
        assert!(self.velocity.is_finite());

        let this_frame_velocity = std::hint::black_box(self.velocity * dt);
        #[cfg(feature = "static_update")]
        let this_frame_velocity = Vec2::ZERO;

        self.position += this_frame_velocity;
        assert!(self.position.is_finite());

        #[cfg(not(feature = "no_life_history"))]
        {
            std::hint::black_box(self.life_history[rng.gen_range(0..self.life_history.len())] += 1);
        }
    }

    fn edges(&mut self, screen_width: f32, screen_height: f32) {
        if self.position.x > screen_width {
            self.position.x = 0.0;
        } else if self.position.x < 0.0 {
            self.position.x = screen_width;
        }

        if self.position.y > screen_height {
            self.position.y = 0.0;
        } else if self.position.y < 0.0 {
            self.position.y = screen_height;
        }
    }

    fn draw(&self, canvas: &mut graphics::Canvas, boid_mesh: &graphics::Mesh) -> GameResult {
        let angle = self.velocity.y.atan2(self.velocity.x);
        canvas.draw(
            boid_mesh,
            graphics::DrawParam::new()
                .dest(self.position)
                .rotation(angle),
        );
        Ok(())
    }
}

struct MainState {
    boids: Vec<BoidRef>,
    unused_boids: Vec<BoidRef>,
    is_attracted: bool,
    rect_max: Vec2,
    rng: rand_chacha::ChaCha8Rng,
}

impl MainState {
    fn new(num_boids: u16, rect_max: Vec2) -> GameResult<MainState> {
        let mut rng = rand_chacha::ChaCha8Rng::from_seed([0; 32]);
        let mut boids = vec![];
        let mut unesed_boids = vec![];
        for _ in 0..num_boids {
            boids.push(Self::new_random_boid(rect_max, &mut rng));
            // For each boid, create 8-15 unused boids to test the performance of the memory allocator
            for _ in 0..rng.gen_range(8..16) {
                unesed_boids.push(Self::new_random_boid(rect_max, &mut rng));
            }
        }
        Ok(MainState {
            boids,
            unused_boids: unesed_boids,
            is_attracted: false,
            rect_max,
            rng,
        })
    }

    fn new_random_boid(rect_max: Vec2, rng: &mut rand_chacha::ChaCha8Rng) -> BoidRef {
        let new_boid = |position: Vec2, vel_angle: f32| {
            let boid = Boid::new(
                position,
                Vec2::new(vel_angle.cos(), vel_angle.sin()) * MAX_SPEED / 2.0,
            );
            let boid_cell = RefCell::new(boid);

            #[cfg(not(feature = "no_boxing"))]
            return Box::new(boid_cell);

            #[cfg(feature = "no_boxing")]
            return boid_cell;
        };

        new_boid(
            Vec2::new(
                rng.gen_range(0.0..rect_max.x),
                rng.gen_range(0.0..rect_max.y),
            ),
            rng.gen_range(0.0..std::f32::consts::TAU),
        )
    }

    fn make_boid_mesh(&self, ctx: &mut Context) -> GameResult<graphics::Mesh> {
        let p1 = Vec2::new(BOID_SIZE, 0f32);
        let p2 = Vec2::new(0f32, BOID_SIZE / 2.0f32);
        let p3 = Vec2::new(0f32, -BOID_SIZE / 2.0f32);
        graphics::Mesh::new_polygon(
            ctx,
            graphics::DrawMode::fill(),
            &[p1, p2, p3],
            if self.is_attracted {
                Color::BLUE
            } else {
                Color::RED
            },
        )
    }
}

impl EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        tracy_scope!("update");
        if ctx.keyboard.is_key_just_pressed(KeyCode::Up) {
            tracy_scope!("add_boids");
            self.unused_boids.clear();
            for _ in 0..10 {
                self.boids
                    .push(Self::new_random_boid(self.rect_max, &mut self.rng));
                for _ in 0..self.rng.gen_range(8..16) {
                    self.unused_boids
                        .push(Self::new_random_boid(self.rect_max, &mut self.rng));
                }
            }
        } else if ctx.keyboard.is_key_just_pressed(KeyCode::Down) {
            tracy_scope!("remove_boids");
            self.unused_boids.clear();
            for _ in 0..10 {
                self.boids.pop();
                for _ in 0..self.rng.gen_range(8..16) {
                    self.unused_boids
                        .push(Self::new_random_boid(self.rect_max, &mut self.rng));
                }
            }
        }

        let dt = ctx.time.delta().as_secs_f32();
        let mouse_pos = Vec2::new(ctx.mouse.position().x, ctx.mouse.position().y);
        {
            tracy_scope!("update_boids");
            for boid_idx in 0..self.boids.len() {
                let mut boid = self.boids[boid_idx].borrow_mut(); // Safety: we check the index to avoid borrowing self

                boid.apply_behavior(boid_idx, &self.boids, mouse_pos, self.is_attracted);
                boid.update(dt, &mut self.rng);
                boid.edges(self.rect_max.x, self.rect_max.y);
            }
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        tracy_scope!("draw");
        let mut canvas = graphics::Canvas::from_frame(ctx, Color::WHITE);

        {
            tracy_scope!("draw_boids");
            let boid_mesh = self.make_boid_mesh(ctx)?;
            for boid_cell in &self.boids {
                tracy_scope!("draw_boids");
                boid_cell.borrow().draw(&mut canvas, &boid_mesh)?;
            }
        }

        {
            tracy_scope!("draw_ui");
            let fps_text = Text::new(format!("FPS: {:.2}", ctx.time.fps()));
            canvas.draw(
                &fps_text,
                DrawParam::new()
                    .dest(Vec2::new(10.0, 10.0))
                    .color(Color::BLACK),
            );

            let frametime_text = Text::new(format!(
                "Frame time: {:.2} us",
                ctx.time.delta().as_micros()
            ));
            canvas.draw(
                &frametime_text,
                DrawParam::new()
                    .dest(Vec2::new(10.0, 20.0))
                    .color(Color::BLACK),
            );

            let boid_count_text = Text::new(format!("Boids: {}", self.boids.len()));
            canvas.draw(
                &boid_count_text,
                DrawParam::new()
                    .dest(Vec2::new(10.0, 50.0))
                    .color(Color::BLACK),
            );
        }

        canvas.finish(ctx)?;

        tracy_client::frame_mark();
        Ok(())
    }
}

fn main() -> GameResult {
    tracy_client::Client::start();

    let num_boids: u16 = env::args()
        .nth(1)
        .and_then(|n| n.parse::<u16>().ok())
        .unwrap_or(100);

    let dim_x = 1080.0;
    let dim_y = 800.0;
    let (ctx, event_loop) = ContextBuilder::new("boids", "Author")
        .window_setup(
            ggez::conf::WindowSetup::default()
                .title("Boids")
                .vsync(false),
        )
        .window_mode(ggez::conf::WindowMode::default().dimensions(dim_x, dim_y))
        .build()?;

    let state = MainState::new(num_boids, Vec2::new(dim_x, dim_y))?;
    event::run(ctx, event_loop, state)
}
