use std::cell::RefCell;

use ggez::event::{self, EventHandler};
use ggez::graphics::{self, Color, DrawParam, Text};
use ggez::input::keyboard::KeyCode;
use ggez::{Context, ContextBuilder, GameResult};
use glam::Vec2;
use rand::Rng;

const MAX_SPEED: f32 = 100.0;
const MAX_FORCE: f32 = 50.0;
const PERCEPTION: f32 = 100.0;
const SEPARATION: f32 = 50.0;
const MOUSE_FACTOR: f32 = 50.0;

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

    fn apply_behavior(
        &mut self,
        self_idx: usize,
        boids: &[Box<RefCell<Boid>>],
        mouse_pos: Vec2,
        is_attracted: bool,
    ) {
        let mut alignment = Vec2::ZERO;
        let mut cohesion = Vec2::ZERO;
        let mut separation = Vec2::ZERO;
        let mut total = 0;
        let mut total_separation = 0;

        for other_idx in 0..boids.len() {
            if other_idx == self_idx {
                continue;
            }

            let other = boids[other_idx].borrow();
            let distance = self.position.distance(other.position);
            if distance < PERCEPTION {
                alignment += other.velocity;
                cohesion += other.position;
                total += 1;

                if distance < SEPARATION {
                    let diff = (self.position - other.position).normalize() / distance;
                    separation += diff;
                    total_separation += 1;
                }
            }
        }

        if total > 0 {
            alignment /= total as f32;
            alignment = alignment.normalize() * MAX_SPEED;
            alignment -= self.velocity;
            alignment = alignment.clamp_length_max(MAX_FORCE);

            cohesion /= total as f32;
            cohesion -= self.position;
            cohesion = cohesion.normalize() * MAX_SPEED;
            cohesion -= self.velocity;
            cohesion = cohesion.clamp_length_max(MAX_FORCE);
        }

        if total_separation > 0 {
            separation /= total_separation as f32;
            separation = separation.normalize() * MAX_SPEED;
            separation -= self.velocity;
            separation = separation.clamp_length_max(MAX_FORCE);
        }

        // if is_attracted {
        //     let attraction = (mouse_pos - self.position).normalize() * MOUSE_FACTOR;
        //     self.velocity += attraction;
        // }

        self.acceleration = alignment + cohesion + separation;
    }

    fn update(&mut self, dt: f32) {
        self.velocity += self.acceleration * dt;
        self.position += self.velocity * dt;
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

    fn draw(
        &self,
        ctx: &mut Context,
        canvas: &mut graphics::Canvas,
        is_attracted: bool,
    ) -> GameResult {
        let circle = graphics::Mesh::new_circle(
            ctx,
            graphics::DrawMode::fill(),
            self.position,
            5.0,
            0.1,
            if is_attracted {
                Color::BLUE
            } else {
                Color::RED
            },
        )?;
        canvas.draw(&circle, graphics::DrawParam::default());
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
}

impl EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        if ctx.keyboard.is_key_pressed(KeyCode::Up) {
            self.boids.push(self.new_random_boid());
        } else if ctx.keyboard.is_key_pressed(KeyCode::Down) {
            self.boids.pop();
        }

        let dt = ctx.time.delta().as_secs_f32();
        let mouse_pos = Vec2::new(ctx.mouse.position().x, ctx.mouse.position().y);

        for boid_idx in 0..self.boids.len() {
            let mut boid = self.boids[boid_idx].borrow_mut();
            boid.apply_behavior(boid_idx, &self.boids, mouse_pos, self.is_attracted);
            boid.update(dt);
            boid.edges(800.0, 600.0); // Assuming screen size
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas = graphics::Canvas::from_frame(ctx, Color::WHITE);

        for boid_cell in &self.boids {
            boid_cell
                .borrow()
                .draw(ctx, &mut canvas, self.is_attracted)?;
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
    let dim_x = 800.0;
    let dim_y = 600.0;
    let (ctx, event_loop) = ContextBuilder::new("boids", "Author")
        .window_setup(ggez::conf::WindowSetup::default().title("Boids"))
        .window_mode(ggez::conf::WindowMode::default().dimensions(dim_x, dim_y))
        .build()?;

    let state = MainState::new(100, Vec2::new(dim_x, dim_y))?;
    event::run(ctx, event_loop, state)
}
