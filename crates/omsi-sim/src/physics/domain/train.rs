//! Bodies coupled by joints: the joint between two of them ([`Joint`], [`JointKind`]) and what
//! each is given for a step ([`BodyInputs`]).

use glam::Vec3;

/// What a joint lets turn besides the vertical and the transverse axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointKind {
    /// A lorry's hitch (`[coupling_front_character]` type 0, or no such section): free about
    /// every axis, with no limits.
    Ball,
    /// An articulated bus's turntable (any other type): the sections do not roll against
    /// each other, and the joint stops at `alpha_max`, `beta_min` and `beta_max`.
    Articulation,
}

/// The joint between body `lead` and body `follow` of a train.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Joint {
    pub lead: usize,
    pub follow: usize,
    /// The ball: the lead's `[coupling_back]` and the follower's `[coupling_front]`, each in its
    /// own model frame (`vehicle::coupling_points`).
    pub anchor_lead: Vec3,
    pub anchor_follow: Vec3,
    /// How far ahead of the ball, on the lead, a pusher's joint pitches (its
    /// `[coupling_pitch_offset]`; 0: a puller's, pitching at the ball). The ball then rides a
    /// link that turns with the lead and pitches with the follower (`TrailerPart::pitch_hinge`).
    pub pitch_offset: f32,
    /// `[coupling_front_character]`: the most the joint turns either way about the vertical
    /// axis, and its range about the transverse one (rad).
    pub alpha_max: f32,
    pub beta_min: f32,
    pub beta_max: f32,
    pub kind: JointKind,
}

/// What a body of a train is given for a step: its brake force per wheel (N), the steering
/// (−1..1) for the one that is steered - the others turn as their joint bends them -, and
/// whether the air acts on it (on the leading body alone: the sections behind are in its lee).
#[derive(Debug, Clone, Copy)]
pub struct BodyInputs<'a> {
    pub brake: &'a [f32],
    pub steer: Option<f32>,
    pub air: bool,
}
