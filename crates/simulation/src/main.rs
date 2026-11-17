//! Bevy viewer for a seeded single-vertex simulation.
//!
//! Every launch starts a new run and prints its seed. The simulation worker owns
//! the model and its clock, and the viewer draws completed snapshots.

use std::time::Duration;

use bevy::{prelude::*, window::WindowResizeConstraints};
use bevy_rand::{global::GlobalRng, prelude::ChaCha12Rng};
use rand::RngExt;
use simulation::{
    DiffusionConfig, SimulationControls, SimulationPlugin,
    field::Neighbourhood,
    graph::{Uptake, VertexConfig},
    normalization::{BoundsAveraging, BoundsNormalizer},
};

mod viewer;

const STEP_INTERVAL: Duration = Duration::from_millis(100);
const MODEL_TIMESTEP: Duration = Duration::from_millis(100);

fn main() -> AppExit {
    let uptake = Uptake::saturating(20.0, 10.0, 0.75).expect("invalid uptake parameters");
    let vertex_config = VertexConfig::new(uptake, 1.2).expect("invalid vertex config parameters");
    let simulation = SimulationPlugin::default()
        .with_diffusion(DiffusionConfig::Enabled {
            neighbourhood: Neighbourhood::Moore,
            coefficient: 0.8,
            spacing: 1.0,
        })
        .with_step_interval(STEP_INTERVAL)
        .with_model_timestep(MODEL_TIMESTEP)
        .with_starting_vertex(vertex_config);

    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
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
        }),
        simulation,
        viewer::ViewerPlugin::new(BoundsNormalizer::new(BoundsAveraging::EMA { tau: 10.0 })),
    ));

    app.add_systems(Update, handle_keyboard_input);

    app.run()
}

fn handle_keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    controls: Res<SimulationControls>,
    mut rng: Single<&mut ChaCha12Rng, With<GlobalRng>>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) && !controls.toggle_neighbourhood() {
        warn!("Simulation worker has stopped");
    }

    if keyboard_input.just_pressed(KeyCode::KeyR) && !controls.reseed_field(rng.random()) {
        warn!("Simulation worker has stopped");
    }
}
