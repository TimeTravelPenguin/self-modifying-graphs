//! A lattice resource field and a mutable graph with vertex resource uptake.
//!
//! Use [`simulation::SimulationRun`] for seeded, fixed-timestep runs and
//! [`simulation::SnapshotReader`] to display completed ticks independently of the model.
//! [`SimulationPlugin`] provides Bevy schedules for applications that step a resource
//! field directly in Bevy, including the standalone diffusion example.

pub mod field;
pub mod graph;
pub mod normalization;
pub mod plugin;
pub mod rendering;
pub mod simulation;

mod position;
mod viewer;

pub use plugin::{
    DiffusionConfig, SimulationControls, SimulationPlugin, SimulationSet, SimulationStartup,
    SimulationStep, SimulationTime,
};
pub use position::Position;
pub use viewer::ViewerPlugin;
