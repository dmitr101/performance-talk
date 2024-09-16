use std::cell::UnsafeCell;
use std::num::NonZero;

use ggez::event::EventHandler;
use ggez::graphics::{self, Color, DrawParam, Text};
use ggez::{Context, GameResult};
use glam::Vec2;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;

use crate::util::*;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct Boid {
    position: Vec2,
    velocity: Vec2,
}

impl Boid {
    fn new(position: Vec2, velocity: Vec2) -> Self {
        Boid { position, velocity }
    }

    #[inline(always)]
    fn is_close_enough(&self, other: &Boid, max_dist: f32) -> bool {
        let distance = self.position.distance_squared(other.position);
        distance < (max_dist * max_dist) && distance > 0.0
    }

    #[inline(never)]
    fn alignment(&self, boids: &[Boid], self_idx: usize) -> Vec2 {
        let mut alignment = Vec2::ZERO;
        let mut total = 0;

        for other_idx in 0..boids.len() {
            if other_idx == self_idx {
                continue;
            }

            let other = &boids[other_idx];
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
    fn cohesion(&self, boids: &[Boid], self_idx: usize) -> Vec2 {
        let mut cohesion = Vec2::ZERO;
        let mut total = 0;

        for other_idx in 0..boids.len() {
            if other_idx == self_idx {
                continue;
            }

            let other = &boids[other_idx];
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
    fn separation(&self, boids: &[Boid], self_idx: usize) -> Vec2 {
        let mut separation = Vec2::ZERO;
        let mut total_separation = 0;

        for other_idx in 0..boids.len() {
            if other_idx == self_idx {
                continue;
            }

            let other = &boids[other_idx];
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
    fn calc_acceleration(
        &self,
        self_idx: usize,
        boids: &[Boid],
        mouse_pos: Vec2,
        is_attracted: bool,
    ) -> Vec2 {
        let alignment = self.alignment(boids, self_idx);
        let cohesion = self.cohesion(boids, self_idx);
        let separation = self.separation(boids, self_idx);

        let mut acceleration = alignment;
        acceleration += cohesion;
        acceleration += separation;

        if is_attracted {
            let attraction = (mouse_pos - self.position).normalize() * MAX_SPEED;
            acceleration += attraction;
        }
        assert!(acceleration.is_finite());
        acceleration
    }

    fn update(&mut self, dt: f32, source: &Boid, acceleration: Vec2) {
        self.position = source.position;
        self.velocity = source.velocity;

        let this_frame_acceleration = std::hint::black_box(acceleration * dt);
        #[cfg(feature = "static_update")]
        let this_frame_acceleration = Vec2::ZERO;

        self.velocity += this_frame_acceleration;
        assert!(self.velocity.is_finite());

        let this_frame_velocity = std::hint::black_box(self.velocity * dt);
        #[cfg(feature = "static_update")]
        let this_frame_velocity = Vec2::ZERO;

        self.position += this_frame_velocity;
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

struct BoidsDoubleBuffer {
    boids: [UnsafeCell<Vec<Boid>>; 2],
    current_idx: usize,
}

impl BoidsDoubleBuffer {
    fn new(active_boids: Vec<Boid>) -> Self {
        let len = active_boids.len();
        BoidsDoubleBuffer {
            boids: [
                UnsafeCell::new(active_boids),
                UnsafeCell::new(vec![Boid::default(); len]),
            ],
            current_idx: 0,
        }
    }

    fn get_current_boids(&self) -> &[Boid] {
        unsafe { &*self.boids[self.current_idx].get() }
    }

    fn get_next_boids(&self) -> &mut [Boid] {
        unsafe { &mut *self.boids[self.current_idx ^ 1].get() }
    }

    fn swap(&mut self) {
        self.current_idx ^= 1;
    }
}

unsafe impl Sync for BoidsDoubleBuffer {}

pub struct MainState {
    boids: BoidsDoubleBuffer,
    is_attracted: bool,
    rect_max: Vec2,
}

impl MainState {
    pub fn new(num_boids: u16, rect_max: Vec2) -> GameResult<MainState> {
        let mut rng = rand_chacha::ChaCha8Rng::from_seed([0; 32]);
        let mut active_boids = vec![];
        for _ in 0..num_boids {
            active_boids.push(Self::new_random_boid(rect_max, &mut rng));
        }
        Ok(MainState {
            boids: BoidsDoubleBuffer::new(active_boids),
            is_attracted: false,
            rect_max,
        })
    }

    fn new_random_boid(rect_max: Vec2, rng: &mut rand_chacha::ChaCha8Rng) -> Boid {
        let new_boid = |position: Vec2, vel_angle: f32| {
            let boid = Boid::new(
                position,
                Vec2::new(vel_angle.cos(), vel_angle.sin()) * MAX_SPEED / 2.0,
            );
            boid
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
        let dt = ctx.time.delta().as_secs_f32();
        let mouse_pos = Vec2::new(ctx.mouse.position().x, ctx.mouse.position().y);
        {
            tracy_scope!("update_boids");
            let boids_len = self.boids.get_current_boids().len();
            #[cfg(not(feature = "no_false_sharing"))]
            {
                let core_count: usize = std::thread::available_parallelism()
                    .unwrap_or(NonZero::new(1).unwrap())
                    .into();
                let num_chunks = (boids_len) / core_count;
                (0..core_count).into_par_iter().for_each(|core_idx| {
                    tracy_scope!("update_boids_thread");
                    for chunk_idx in 0..num_chunks {
                        let boid_idx = chunk_idx * core_count + core_idx;
                        let current_boids = self.boids.get_current_boids();
                        let next_boids = self.boids.get_next_boids();
                        let boid = &current_boids[boid_idx];
                        let acc = boid.calc_acceleration(
                            boid_idx,
                            &current_boids,
                            mouse_pos,
                            self.is_attracted,
                        );
                        next_boids[boid_idx].update(dt, &boid, acc);
                        next_boids[boid_idx].edges(self.rect_max.x, self.rect_max.y);
                    }
                });
            }
            #[cfg(feature = "no_false_sharing")]
            {
                (0..boids_len)
                    .into_par_iter()
                    .with_min_len(8)
                    .for_each(|boid_idx| {
                        tracy_scope!("update_boids_thread");
                        let current_boids = self.boids.get_current_boids();
                        let next_boids = self.boids.get_next_boids();
                        let boid = &current_boids[boid_idx];
                        let acc = boid.calc_acceleration(
                            boid_idx,
                            &current_boids,
                            mouse_pos,
                            self.is_attracted,
                        );
                        next_boids[boid_idx].update(dt, &boid, acc);
                        next_boids[boid_idx].edges(self.rect_max.x, self.rect_max.y);
                    });
            }
            self.boids.swap();
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        tracy_scope!("draw");
        let mut canvas = graphics::Canvas::from_frame(ctx, Color::WHITE);

        {
            tracy_scope!("draw_boids");
            let boid_mesh = self.make_boid_mesh(ctx)?;
            let current_boids = self.boids.get_current_boids();
            for boid_cell in current_boids {
                tracy_scope!("draw_boids");
                boid_cell.draw(&mut canvas, &boid_mesh)?;
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

            let boid_count_text =
                Text::new(format!("Boids: {}", self.boids.get_current_boids().len()));
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
