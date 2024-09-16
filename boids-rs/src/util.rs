pub const BOID_SIZE: f32 = 10.0;
pub const MAX_SPEED: f32 = 100.0;
pub const MAX_FORCE: f32 = 80.0;
pub const PERCEPTION: f32 = 100.0;
pub const SEPARATION: f32 = 100.0;

macro_rules! tracy_scope {
    ($name:literal) => {
        let _tracy_span = tracy_client::span!($name);
    };
}

pub(crate) use tracy_scope;
