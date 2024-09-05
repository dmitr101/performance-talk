use std::cell::RefCell;

use ggez::event::{self, EventHandler};
use ggez::graphics::{self, Color, DrawParam, Text};
use ggez::input::keyboard::KeyCode;
use ggez::{Context, ContextBuilder, GameResult};
use glam::Vec2;
use rand::Rng;
use std::env;

const BOID_SIZE: f32 = 10.0;
const MAX_SPEED: f32 = 100.0;
const MAX_FORCE: f32 = 80.0;
const PERCEPTION: f32 = 100.0;
const SEPARATION: f32 = 100.0;
const MOUSE_FACTOR: f32 = 100.0;

struct Boid {
    position: Vec2,
    velocity: Vec2,
    acceleration: Vec2,
}

impl Boid {
    fn new(position: Vec2, velocity: Vec2) -> Self {
        Boid {
            position,
            velocity,
            acceleration: Vec2::ZERO,
        }
    }

    fn alignment(&self, boids: &[Box<RefCell<Boid>>], self_idx: usize) -> Vec2 {
        let mut alignment = Vec2::ZERO;
        let mut total = 0;

        for other_idx in 0..boids.len() {
            if other_idx == self_idx {
                continue;
            }

            let other = boids[other_idx].borrow();
            let distance = self.position.distance(other.position);
            if distance < PERCEPTION && distance > 0.0 {
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

    fn cohesion(&self, boids: &[Box<RefCell<Boid>>], self_idx: usize) -> Vec2 {
        let mut cohesion = Vec2::ZERO;
        let mut total = 0;

        for other_idx in 0..boids.len() {
            if other_idx == self_idx {
                continue;
            }

            let other = boids[other_idx].borrow();
            let distance = self.position.distance(other.position);
            if distance < PERCEPTION && distance > 0.0 {
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

    fn separation(&self, boids: &[Box<RefCell<Boid>>], self_idx: usize) -> Vec2 {
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

    fn apply_behavior(
        &mut self,
        self_idx: usize,
        boids: &[Box<RefCell<Boid>>],
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

    fn update(&mut self, dt: f32) {
        self.velocity += self.acceleration * dt;
        assert!(self.velocity.is_finite());

        self.position += self.velocity * dt;
        assert!(self.position.is_finite());
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
    boids: Vec<Box<RefCell<Boid>>>,
    is_attracted: bool,
    rect_max: Vec2,
}

impl MainState {
    fn new(num_boids: u16, rect_max: Vec2) -> GameResult<MainState> {
        let mut rng = rand::thread_rng();
        let new_boid = |position: Vec2, vel_angle: f32| {
            let mut rng = rand::thread_rng();
            Box::new(RefCell::new(Boid::new(
                position,
                Vec2::new(vel_angle.cos(), vel_angle.sin()) * rng.gen_range(0.0..MAX_SPEED),
            )))
        };

        let boids = (0..num_boids)
            .map(|_| {
                new_boid(
                    Vec2::new(
                        rng.gen_range(0.0..rect_max.x),
                        rng.gen_range(0.0..rect_max.y),
                    ),
                    rng.gen_range(0.0..std::f32::consts::TAU),
                )
            })
            .collect();
        Ok(MainState {
            boids,
            is_attracted: false,
            rect_max,
        })
    }

    fn new_random_boid(&self) -> Box<RefCell<Boid>> {
        let mut rng = rand::thread_rng();
        let new_boid = |position: Vec2, vel_angle: f32| {
            let mut rng = rand::thread_rng();
            Box::new(RefCell::new(Boid::new(
                position,
                Vec2::new(vel_angle.cos(), vel_angle.sin()) * rng.gen_range(0.0..MAX_SPEED),
            )))
        };

        new_boid(
            Vec2::new(
                rng.gen_range(0.0..self.rect_max.x),
                rng.gen_range(0.0..self.rect_max.y),
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
        if ctx.keyboard.is_key_just_pressed(KeyCode::Up) {
            for _ in 0..10 {
                self.boids.push(self.new_random_boid());
            }
        } else if ctx.keyboard.is_key_just_pressed(KeyCode::Down) {
            for _ in 0..10 {
                self.boids.pop();
            }
        }

        let dt = ctx.time.delta().as_secs_f32();
        let mouse_pos = Vec2::new(ctx.mouse.position().x, ctx.mouse.position().y);

        for boid_idx in 0..self.boids.len() {
            let mut boid = self.boids[boid_idx].borrow_mut();
            boid.apply_behavior(boid_idx, &self.boids, mouse_pos, self.is_attracted);
            boid.update(dt);
            boid.edges(self.rect_max.x, self.rect_max.y);
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas = graphics::Canvas::from_frame(ctx, Color::WHITE);

        let boid_mesh = self.make_boid_mesh(ctx)?;
        for boid_cell in &self.boids {
            boid_cell.borrow().draw(&mut canvas, &boid_mesh)?;
        }

        let fps_text = Text::new(format!("FPS: {:.2}", ctx.time.fps()));
        canvas.draw(
            &fps_text,
            DrawParam::new()
                .dest(Vec2::new(10.0, 10.0))
                .color(Color::BLACK),
        );

        let boid_count_text = Text::new(format!("Boids: {}", self.boids.len()));
        canvas.draw(
            &boid_count_text,
            DrawParam::new()
                .dest(Vec2::new(10.0, 50.0))
                .color(Color::BLACK),
        );

        canvas.finish(ctx)?;
        Ok(())
    }
}

fn main() -> GameResult {
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
