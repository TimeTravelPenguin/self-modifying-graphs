//! Bevy viewer for a seeded single-vertex simulation.
//!
//! Every launch starts a new run and prints its seed. The simulation worker owns
//! the model and its clock, and the viewer draws completed snapshots.

use std::{error::Error, io, time::Duration};

use bevy::{prelude::*, window::WindowResizeConstraints};
use simulation::{
    field::Neighbourhood,
    graph::Uptake,
    simulation::{SimulationConfig, SimulationRun},
};

mod viewer;
mod worker;

use worker::SimulationWorker;

const STEP_INTERVAL: Duration = Duration::from_millis(100);

fn main() -> Result<AppExit, Box<dyn Error>> {
    let config = SimulationConfig {
        width: 100,
        height: 100,
        spacing: 1.0,
        initial_resource: 100.0,
        resource_noise_scale: 10.0,
        diffusion_coefficient: 0.8,
        neighbourhood: Neighbourhood::Moore,
    };

    let seed = rand::random::<u32>();
    let uptake = Uptake::new(10.0, 0.75).expect("invalid uptake parameters");
    let run =
        SimulationRun::single_vertex_from_seed(config, seed, uptake, STEP_INTERVAL.as_secs_f64())?;

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Graph simulation".into(),
            resolution: (1040, 820).into(),
            resize_constraints: WindowResizeConstraints {
                min_width: 760.0,
                min_height: 740.0,
                ..default()
            },
            ..default()
        }),
        ..default()
    }));

    let worker = SimulationWorker::start(run, STEP_INTERVAL)?;
    println!("Simulation seed {seed}");
    viewer::configure(&mut app, worker.reader());

    let exit = app.run();
    worker
        .stop()
        .map_err(|_| io::Error::other("simulation worker panicked"))?;

    Ok(exit)
}
