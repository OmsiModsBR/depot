//! Humans (`.hum`): skinned models posed procedurally from the joint positions of the
//! `[links]` block, like the original's `mc_human`. Bones are addressed by the engine ids
//! given by `[setbone]` (-2 … -14): thighs, shins, upper arms, forearms, hip, torso, head, hands.
//!
//! OMSI ships no animation clips, so every pose is made here. Every stock and add-on model
//! is stored in a T-pose (arms straight out at shoulder height) with the feet part of the
//! shin bones; the rig is measured from the `[links]` joints and the mesh itself (where the
//! soles, heels, toes and ankles are), and the feet get a bone of their own split off the
//! shins so that they can stay flat on the floor, strike with the heel and push off with
//! the toes.
//!
//! [`Pose`] is the per-person animation state, advanced every frame by [`Pose::advance`]
//! from a [`PoseInput`] (where the person stands, how fast they move, whether they sit,
//! pay, hold on or look at something); [`Pose::bones`] turns it into bone transforms and
//! [`skin`] deforms a mesh with them. Walking is a speed-driven gait with planted feet: a
//! foot on the floor stays where it was put (in the frame of the floor it stands on - the
//! ground or a bus) and the legs reach it by two-bone IK, so a stride always matches the
//! ground speed and nothing slides; the swinging foot flies to where the body will be when
//! it lands. The same stepping carries standing people through turns and shuffles, and the
//! arms reach the cash desk or a handrail by IK as well.
//!
//! - [`rig`]: bones, joints, the rig and loading a human type.
//! - [`pose`]: the per-person pose, its frame step and its bones.
//! - [`skin`]: skinning and the hands.
//! - [`omsi_anim`]: the original's own animation of its people.
//! - [`crowd`]: people on foot as a crowd.
//! - [`aboard`]: what the people aboard a vehicle weigh on its sections.

pub mod aboard;
pub mod crowd;
pub mod omsi_anim;
pub(crate) mod pose;
pub(crate) mod rig;
pub(crate) mod skin;

pub use aboard::payload_by_section;
pub use pose::{run_factor, Activity, Pose, PoseInput, Posed};
pub use rig::{
    slot_of, HumanMesh, HumanType, Influence, Joints, Rig, BONE_HAND_L, BONE_HAND_R, BONE_HEAD,
    BONE_HIP, BONE_MAIN, BONE_OA_L, BONE_OA_R, BONE_OS_L, BONE_OS_R, BONE_UA_L, BONE_UA_R,
    BONE_US_L, BONE_US_R, SLOTS,
};
pub use skin::{curl_hands, grip_centres, hand_slot, skin, skin_from, slots_from_omsi};
