#![feature(portable_simd)]
use ggez::event::{self};
use ggez::{ContextBuilder, GameResult};
use glam::Vec2;
mod boids_impl;

type MainState = boids_impl::MainState;

fn main() -> GameResult {
    tracy_client::Client::start();

    let num_boids: u16 = 4000;

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
