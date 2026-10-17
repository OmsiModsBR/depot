//! What the wheels stand on: the ground at a point ([`Ground`], [`GroundProbe`]) and a face a
//! tyre cannot climb ([`WallFace`]).

use glam::{DVec2, Vec2};

/// The ground a wheel finds at one point: the highest face at or below the probe's top and
/// the lowest face above it (world heights).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GroundProbe {
    pub below: Option<f64>,
    pub above: Option<f64>,
}

/// What the wheels of a vehicle stand on.
pub trait Ground: Send + Sync {
    /// The faces at world (x, y): the highest at or below `top` and the lowest above it.
    fn probe(&self, x: f64, y: f64, top: f64) -> GroundProbe;

    /// A prober for one step of one vehicle - many points close together. It may keep what
    /// it looked up (the tiles under the vehicle) until it is dropped.
    fn session(&self) -> Box<dyn Fn(f64, f64, f64) -> GroundProbe + '_> {
        Box::new(move |x, y, top| self.probe(x, y, top))
    }
}

impl<F: Fn(f64, f64, f64) -> GroundProbe + Send + Sync> Ground for F {
    fn probe(&self, x: f64, y: f64, top: f64) -> GroundProbe {
        self(x, y, top)
    }
}

/// A face a tyre cannot climb, kept while the tyre is up against it: the samples that found
/// it are lost as soon as the hub is over it, and a wheel pushed that far went on through.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallFace {
    /// A point of the face (world, ground plane) and its normal, towards the tyre.
    pub point: DVec2,
    pub normal: Vec2,
}
