use serde::{Deserialize, Serialize};
use std::cell::RefCell;
#[cfg(test)]
use std::cmp::Ordering;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use wasm_bindgen::prelude::*;

mod api;
mod boundaries;
mod core;
mod ecology;
mod factory_delta;
mod factory_preview;
mod geomorphology;
mod ground;
mod ground_grade;
mod ground_spine;
/// Phase 8 slice 4: departure from generated water equilibrium, and the bounded solve that settles
/// it.
/// Movement, construction, earthwork, pumps, commands and snapshots read it; frontier departure
/// waits without claiming world and resumes when survey exposes its chunk.
#[allow(dead_code)]
mod hydrology;
mod recipes;
mod runtime;
mod save_migrations;
use boundaries::*;
use ground::*;
use ground_spine::*;
mod skills;
use skills::*;
/// The binary encoding the snapshot delta crosses the worker boundary in.
mod wire;

use runtime::RuntimeIndex;

/// Derived economy figures: what the shipped numbers actually say the curve is.
///
/// Measurement code like the capacity ladder and the survey, and native only for the same reason:
/// nothing here runs a tick, and the wasm artifact the game ships must not carry it.
#[cfg(not(target_arch = "wasm32"))]
pub mod balance;

/// The Phase 8 physical scale contract. New worlds read it for cadence, height, walking and
/// construction; a save from before the 25 m² hex is refused rather than reinterpreted.
pub mod scale;

/// The Phase 8 drainage-first world generator. Slice 3 selects it for new worlds.
pub mod terra;

/// The rock the generator carves: strength by place and by depth, and what that costs a river.
pub mod terra_rock;

/// What that generator actually produced, counted. Read by `npm run terra` and by the tests.
pub mod terra_survey;

include!("model/constants.rs");
include!("model/definitions.rs");
include!("model/state.rs");
include!("model/snapshots.rs");
include!("model/commands.rs");

include!("model/core_state.rs");

include!("model/save_api.rs");

include!("model/validation_definitions.rs");
include!("model/validation_progression.rs");

include!("model/geometry.rs");
include!("model/world_types.rs");
include!("model/world_sites.rs");
include!("model/world_repair.rs");

include!("model/utility.rs");

/// Reproducible Phase 8 disturbed-water measurement.
///
/// The active case records the solver's own bounded work counters. The quiet case advances a
/// settled world and checks that no water dirty mark or state change appears: settled water has no
/// scheduled kernel, so its measured water work is exactly zero rather than a small per-cell cost.
#[cfg(not(target_arch = "wasm32"))]
pub mod water_bench;

/// Reproducible accelerated proof of the exact coarse geomorphic sequence production runs hourly.
#[cfg(not(target_arch = "wasm32"))]
pub mod erosion_bench;

/// Deterministic headless capacity measurement.
///
/// The roadmap gates finer dirty tracking, any renderer decision, and every scale claim behind
/// measured tiers. This module builds synthetic steady-state factories from the shipped
/// definitions, drives them through the same entry points the worker uses, and reports per-phase
/// cost so capacity is measured instead of asserted.
///
/// The same measurement code runs natively and in the browser worker: only the clock differs, so
/// the two records are comparable by construction rather than by re-implementation. The wasm build
/// is behind the `bench` feature, so the deployed game artifact still never carries it.
#[cfg(any(not(target_arch = "wasm32"), feature = "bench"))]
pub mod capacity;

/// What a parameter set actually generates, counted rather than estimated.
///
/// Value noise is not uniformly distributed, so **a threshold is not a proportion**: nothing about
/// `water_level: 26_000` says what share of a world is water, and a preset that claimed one from
/// arithmetic would be guessing. This module samples a disc of hexes for a parameter set and
/// reports the band histogram, the field density per material, how far the landing site is from
/// each of them, and the shape of the water — the same measured-before-claimed rule the frame
/// budget and the capacity ladder already live under, applied to the generator.
///
/// Measurement code, like the capacity ladder: native only, never compiled into the wasm artifact,
/// and never a dependency of the game or the production build. The acceptance tests use it because
/// the claims they check are claims about proportions, which is exactly what it counts.
#[cfg(not(target_arch = "wasm32"))]
pub mod survey;

#[cfg(test)]
mod tests;
