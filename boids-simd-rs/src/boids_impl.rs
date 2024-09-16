use std::cell::UnsafeCell;
use std::num::NonZero;
use std::simd::cmp::{SimdPartialEq, SimdPartialOrd};
use std::simd::num::SimdFloat;
use std::thread::current;

use ggez::event::EventHandler;
use ggez::graphics::{self, Color, DrawParam, Text};
use ggez::{Context, GameResult};
use glam::Vec2;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;

use std::simd::{f32x8, i32x8, Mask, StdFloat};

pub const BOID_SIZE: f32 = 10.0;
pub const MAX_SPEED: f32 = 100.0;
pub const MAX_FORCE: f32 = 80.0;
pub const PERCEPTION: f32 = 100.0;
pub const SEPARATION: f32 = 100.0;

const EPSILON: f32 = 0.0001;

macro_rules! tracy_scope {
    ($name:literal) => {
        let _tracy_span = tracy_client::span!($name);
    };
}

const CHUNK_SIZE: usize = 8;

#[derive(Debug, Clone, Copy)]
struct SimdVec2 {
    x: f32x8,
    y: f32x8,
}

impl SimdVec2 {
    fn new_splat_all(v: f32) -> Self {
        SimdVec2 {
            x: f32x8::splat(v),
            y: f32x8::splat(v),
        }
    }

    fn zero() -> Self {
        SimdVec2 {
            x: f32x8::splat(0.0),
            y: f32x8::splat(0.0),
        }
    }

    fn new(x: f32x8, y: f32x8) -> Self {
        SimdVec2 { x, y }
    }

    fn normalize(&self) -> Self {
        let length = self.length();
        let x = self.x / length;
        let y = self.y / length;
        SimdVec2 { x, y }
    }

    fn select(&self, mask: MaskType, other: Self) -> Self {
        let x = mask.select(self.x, other.x);
        let y = mask.select(self.y, other.y);
        SimdVec2 { x, y }
    }

    fn length(&self) -> f32x8 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    fn length_squared(&self) -> f32x8 {
        self.x * self.x + self.y * self.y
    }

    fn distance(&self, other: &Self) -> f32x8 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    fn distance_squared(&self, other: &Self) -> f32x8 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    fn clamp_length_max(&self, max: f32) -> Self {
        let length_sqr = self.length_squared();
        let max_simd = f32x8::splat(max);
        let mask = length_sqr.simd_gt(max_simd * max_simd);
        let x = mask.select(max_simd * (self.x / length_sqr.sqrt()), self.x);
        let y = mask.select(max_simd * (self.y / length_sqr.sqrt()), self.y);
        SimdVec2 { x, y }
    }
}

impl std::ops::Add for SimdVec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        SimdVec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl std::ops::Sub for SimdVec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        SimdVec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl std::ops::Mul<f32x8> for SimdVec2 {
    type Output = Self;
    fn mul(self, rhs: f32x8) -> Self {
        SimdVec2::new(self.x * rhs, self.y * rhs)
    }
}

impl std::ops::Div<f32x8> for SimdVec2 {
    type Output = Self;
    fn div(self, rhs: f32x8) -> Self {
        SimdVec2::new(self.x / rhs, self.y / rhs)
    }
}

impl std::ops::AddAssign for SimdVec2 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

type MaskType = Mask<i32, CHUNK_SIZE>;
fn simd_is_close_enough(lhs: &SimdVec2, rhs: &SimdVec2, max_dist: f32) -> MaskType {
    let distance_squared = (*lhs - *rhs).length_squared();
    let simd_dist_sqr = f32x8::splat(max_dist * max_dist);
    distance_squared.simd_le(simd_dist_sqr)
}

fn simd_epsilon_check(lhs: &SimdVec2, rhs: &SimdVec2) -> MaskType {
    let distance_squared = (*lhs - *rhs).length_squared();
    let simd_epsilon = f32x8::splat(EPSILON * EPSILON);
    distance_squared.simd_gt(simd_epsilon)
}

struct BoidsVec {
    pos_x: Vec<f32>,
    pos_y: Vec<f32>,
    vel_x: Vec<f32>,
    vel_y: Vec<f32>,
}

impl BoidsVec {
    fn new_from_scalar(scalar_vec: &[Boid]) -> Self {
        let mut pos_x = Vec::with_capacity(scalar_vec.len());
        let mut pos_y = Vec::with_capacity(scalar_vec.len());
        let mut vel_x = Vec::with_capacity(scalar_vec.len());
        let mut vel_y = Vec::with_capacity(scalar_vec.len());

        for boid in scalar_vec {
            pos_x.push(boid.position.x);
            pos_y.push(boid.position.y);
            vel_x.push(boid.velocity.x);
            vel_y.push(boid.velocity.y);
        }

        BoidsVec {
            pos_x,
            pos_y,
            vel_x,
            vel_y,
        }
    }

    fn new_with_length(len: usize) -> Self {
        BoidsVec {
            pos_x: vec![0.0; len],
            pos_y: vec![0.0; len],
            vel_x: vec![0.0; len],
            vel_y: vec![0.0; len],
        }
    }

    #[inline(never)]
    fn alignment(&self, chunk_idx: usize) -> SimdVec2 {
        let mut alignment: SimdVec2 = SimdVec2::new_splat_all(0.0);
        let mut total: f32x8 = f32x8::splat(0.0);

        let my_start = chunk_idx * CHUNK_SIZE;
        let my_end = my_start + CHUNK_SIZE;
        let this_pos = SimdVec2::new(
            f32x8::from_slice(&self.pos_x[my_start..my_end]),
            f32x8::from_slice(&self.pos_y[my_start..my_end]),
        );
        let this_vel = SimdVec2::new(
            f32x8::from_slice(&self.vel_x[my_start..my_end]),
            f32x8::from_slice(&self.vel_y[my_start..my_end]),
        );

        let num_chunks = self.pos_x.len() / CHUNK_SIZE;
        for other_chunk_idx in 0..num_chunks {
            let start = other_chunk_idx * CHUNK_SIZE;
            let end = start + CHUNK_SIZE;
            let other_pos = SimdVec2::new(
                f32x8::from_slice(&self.pos_x[start..end]),
                f32x8::from_slice(&self.pos_y[start..end]),
            );
            let other_vel = SimdVec2::new(
                f32x8::from_slice(&self.vel_x[start..end]),
                f32x8::from_slice(&self.vel_y[start..end]),
            );

            let is_close_mask = simd_is_close_enough(&this_pos, &other_pos, PERCEPTION);
            let epsilon_mask = simd_epsilon_check(&this_pos, &other_pos);
            let mask = is_close_mask & epsilon_mask;
            let one_or_zero = mask.select(f32x8::splat(1.0), f32x8::splat(0.0));
            alignment += other_vel * one_or_zero;
            total += one_or_zero;
        }

        let total_mask = total.simd_ne(f32x8::splat(0.0));
        alignment = alignment / total;
        alignment = alignment.normalize() * f32x8::splat(MAX_SPEED);
        alignment = alignment - this_vel;
        alignment = alignment.clamp_length_max(MAX_FORCE);
        alignment.select(total_mask, SimdVec2::zero())
    }

    #[inline(never)]
    fn cohesion(&self, chunk_idx: usize) -> SimdVec2 {
        let mut cohesion: SimdVec2 = SimdVec2::new_splat_all(0.0);
        let mut total: f32x8 = f32x8::splat(0.0);

        let my_start = chunk_idx * CHUNK_SIZE;
        let my_end = my_start + CHUNK_SIZE;
        let this_pos = SimdVec2::new(
            f32x8::from_slice(&self.pos_x[my_start..my_end]),
            f32x8::from_slice(&self.pos_y[my_start..my_end]),
        );
        let this_vel = SimdVec2::new(
            f32x8::from_slice(&self.vel_x[my_start..my_end]),
            f32x8::from_slice(&self.vel_y[my_start..my_end]),
        );

        let num_chunks = self.pos_x.len() / CHUNK_SIZE;
        for other_chunk_idx in 0..num_chunks {
            let start = other_chunk_idx * CHUNK_SIZE;
            let end = start + CHUNK_SIZE;
            let other_pos = SimdVec2::new(
                f32x8::from_slice(&self.pos_x[start..end]),
                f32x8::from_slice(&self.pos_y[start..end]),
            );

            let is_close_mask = simd_is_close_enough(&this_pos, &other_pos, PERCEPTION);
            let epsilon_mask = simd_epsilon_check(&this_pos, &other_pos);
            let mask = is_close_mask & epsilon_mask;
            let one_or_zero = mask.select(f32x8::splat(1.0), f32x8::splat(0.0));
            cohesion += other_pos * one_or_zero;
            total += one_or_zero;
        }

        let total_mask = total.simd_ne(f32x8::splat(0.0));
        cohesion = cohesion / total;
        cohesion = (cohesion - this_pos).normalize() * f32x8::splat(MAX_SPEED);
        cohesion = cohesion - this_vel;
        cohesion = cohesion.clamp_length_max(MAX_FORCE);
        cohesion.select(total_mask, SimdVec2::zero())
    }

    #[inline(never)]
    fn separation(&self, chunk_idx: usize) -> SimdVec2 {
        let mut separation: SimdVec2 = SimdVec2::new_splat_all(0.0);
        let mut total: f32x8 = f32x8::splat(0.0);

        let my_start = chunk_idx * CHUNK_SIZE;
        let my_end = my_start + CHUNK_SIZE;
        let this_pos = SimdVec2::new(
            f32x8::from_slice(&self.pos_x[my_start..my_end]),
            f32x8::from_slice(&self.pos_y[my_start..my_end]),
        );
        let this_vel = SimdVec2::new(
            f32x8::from_slice(&self.vel_x[my_start..my_end]),
            f32x8::from_slice(&self.vel_y[my_start..my_end]),
        );

        let num_chunks = self.pos_x.len() / CHUNK_SIZE;
        for other_chunk_idx in 0..num_chunks {
            let start = other_chunk_idx * CHUNK_SIZE;
            let end = start + CHUNK_SIZE;
            let other_pos = SimdVec2::new(
                f32x8::from_slice(&self.pos_x[start..end]),
                f32x8::from_slice(&self.pos_y[start..end]),
            );

            let diff = this_pos - other_pos;
            let distance = diff.length();

            let is_close_mask = distance.simd_le(f32x8::splat(SEPARATION));
            let epsilon_mask = distance.simd_gt(f32x8::splat(EPSILON));
            let mask = is_close_mask & epsilon_mask;
            let separation_acc =
                (diff.normalize() / distance).select(mask, SimdVec2::new_splat_all(0.0));
            separation += separation_acc;
            total += mask.select(f32x8::splat(1.0), f32x8::splat(0.0));
        }

        let total_mask = total.simd_ne(f32x8::splat(0.0));
        separation = separation / total;
        separation = separation.normalize() * f32x8::splat(MAX_SPEED);
        separation = separation - this_vel;
        separation = separation.clamp_length_max(MAX_FORCE);
        separation.select(total_mask, SimdVec2::zero())
    }

    #[inline(never)]
    fn calc_acceleration(&self, chunk_idx: usize) -> SimdVec2 {
        let alignment = self.alignment(chunk_idx);
        let cohesion = self.cohesion(chunk_idx);
        let separation = self.separation(chunk_idx);
        alignment + cohesion + separation
    }

    fn update(&mut self, chunk_idx: usize, dt: f32, source: &Self, screen_rect: Vec2) {
        let start = chunk_idx * CHUNK_SIZE;
        let end = start + CHUNK_SIZE;
        let mut this_pos = SimdVec2::new(
            f32x8::from_slice(&source.pos_x[start..end]),
            f32x8::from_slice(&source.pos_y[start..end]),
        );
        let mut this_vel = SimdVec2::new(
            f32x8::from_slice(&source.vel_x[start..end]),
            f32x8::from_slice(&source.vel_y[start..end]),
        );
        let acceleration: SimdVec2 = source.calc_acceleration(chunk_idx);

        let simd_dt = f32x8::splat(dt);
        let this_frame_acceleration = std::hint::black_box(acceleration * simd_dt);
        #[cfg(feature = "static_update")]
        let this_frame_acceleration = SimdVec2::new_splat_all(0.0);

        this_vel += this_frame_acceleration;

        let this_frame_velocity = std::hint::black_box(this_vel * simd_dt);
        #[cfg(feature = "static_update")]
        let this_frame_velocity = SimdVec2::new_splat_all(0.0);

        this_pos += this_frame_velocity;

        // Edges
        let mask_x = this_pos.x.simd_gt(f32x8::splat(screen_rect.x));
        let mask_y = this_pos.y.simd_gt(f32x8::splat(screen_rect.y));
        this_pos.x = mask_x.select(f32x8::splat(0.0), this_pos.x);
        this_pos.y = mask_y.select(f32x8::splat(0.0), this_pos.y);

        let mask_x = this_pos.x.simd_lt(f32x8::splat(0.0));
        let mask_y = this_pos.y.simd_lt(f32x8::splat(0.0));
        this_pos.x = mask_x.select(f32x8::splat(screen_rect.x), this_pos.x);
        this_pos.y = mask_y.select(f32x8::splat(screen_rect.y), this_pos.y);

        this_pos.x.copy_to_slice(&mut self.pos_x[start..end]);
        this_pos.y.copy_to_slice(&mut self.pos_y[start..end]);
        this_vel.x.copy_to_slice(&mut self.vel_x[start..end]);
        this_vel.y.copy_to_slice(&mut self.vel_y[start..end]);
    }

    fn iter_as_scalar(&self) -> impl Iterator<Item = Boid> + '_ {
        self.pos_x
            .iter()
            .zip(self.pos_y.iter())
            .zip(self.vel_x.iter().zip(self.vel_y.iter()))
            .map(|((&pos_x, &pos_y), (&vel_x, &vel_y))| Boid {
                position: Vec2::new(pos_x, pos_y),
                velocity: Vec2::new(vel_x, vel_y),
            })
    }

    fn len(&self) -> usize {
        self.pos_x.len()
    }

    fn num_chunks(&self) -> usize {
        self.pos_x.len() / CHUNK_SIZE
    }
}

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
    boids: [UnsafeCell<BoidsVec>; 2],
    current_idx: usize,
}

impl BoidsDoubleBuffer {
    fn new(active_boids: Vec<Boid>) -> Self {
        let len = active_boids.len();
        BoidsDoubleBuffer {
            boids: [
                UnsafeCell::new(BoidsVec::new_from_scalar(&active_boids)),
                UnsafeCell::new(BoidsVec::new_with_length(len)),
            ],
            current_idx: 0,
        }
    }

    fn get_current_boids(&self) -> &BoidsVec {
        unsafe { &*self.boids[self.current_idx].get() }
    }

    fn get_next_boids(&self) -> &mut BoidsVec {
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
        // let mouse_pos = Vec2::new(ctx.mouse.position().x, ctx.mouse.position().y);
        {
            tracy_scope!("update_boids");
            #[cfg(not(feature = "threaded"))]
            {
                let current_boids = self.boids.get_current_boids();
                let next_boids = self.boids.get_next_boids();
                for chunk_idx in 0..current_boids.num_chunks() {
                    next_boids.update(chunk_idx, dt, current_boids, self.rect_max);
                }
            }
            #[cfg(feature = "threaded")]
            {
                let num_chunks = self.boids.get_current_boids().num_chunks();
                (0..num_chunks).into_par_iter().for_each(|chunk_idx| {
                    self.boids
                        .get_next_boids()
                        .update(chunk_idx, dt, self.boids.get_current_boids(), self.rect_max);
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
            for boid in current_boids.iter_as_scalar() {
                tracy_scope!("draw_boids");
                boid.draw(&mut canvas, &boid_mesh)?;
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
