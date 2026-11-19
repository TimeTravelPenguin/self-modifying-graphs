//! Interactive resource diffusion example driven by [`simulation::SimulationPlugin`].
//!
//! Press Space to switch between stencils. Press R to reseed the field and reset
//! its display bounds.

use std::time::Duration;

use bevy::prelude::*;
use bevy_rand::{global::GlobalRng, prelude::ChaCha12Rng};
use rand::RngExt;
use simulation::{
    DiffusionConfig, SimulationPlugin, SimulationTime,
    field::{Neighbourhood, ResourceField},
    normalization::{BoundsAveraging, BoundsNormalizer},
    palette,
};

mod heatmap;
mod info_panel;

use heatmap::{spawn_heatmap, update_heatmap};
use info_panel::{spawn_on_screen_info, update_on_screen_info};

const RESOURCE_NOISE_SCALE: f64 = 10.0;
const MAX_RESOURCE: f64 = 100.0;

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            resolution: (1280, 850).into(),
            ..default()
        }),
        ..default()
    }));

    configure_simulation(&mut app, 100, 100);

    app.run()
}

/// Shares the same simulation settings between the window and the headless test.
fn configure_simulation(app: &mut App, width: u32, height: u32) {
    let mut plugin = SimulationPlugin::default()
        .with_dimensions(width, height)
        .with_seed(43)
        .with_perlin_scale(RESOURCE_NOISE_SCALE)
        .with_max_resource(MAX_RESOURCE)
        .with_diffusion(DiffusionConfig::Enabled {
            neighbourhood: Neighbourhood::VonNeumann,
            coefficient: 0.8,
            spacing: 1.0,
        })
        .with_model_timestep(Duration::from_millis(100))
        .with_step_interval(Duration::from_millis(20));

    *plugin.bounds_normalizer() = BoundsNormalizer::new(BoundsAveraging::EMA { tau: 1.0 });

    app.add_plugins(plugin)
        .insert_resource(ClearColor(palette::BACKGROUND))
        .add_systems(Startup, setup)
        .add_systems(Update, handle_keyboard_input)
        .add_systems(
            Update,
            (update_heatmap, update_on_screen_info).after(handle_keyboard_input),
        );
}

fn setup(
    mut commands: Commands,
    resources: Res<ResourceField>,
    bounds: Res<BoundsNormalizer>,
    diffusion: Res<DiffusionConfig>,
    time: Res<SimulationTime>,
) {
    commands.spawn(Camera2d);

    spawn_heatmap(&mut commands, &resources, &bounds);
    spawn_on_screen_info(&mut commands, &diffusion, &bounds, &time);
}

fn handle_keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut resources: ResMut<ResourceField>,
    mut diffusion: ResMut<DiffusionConfig>,
    mut bounds: ResMut<BoundsNormalizer>,
    time: Res<SimulationTime>,
    mut rng: Single<&mut ChaCha12Rng, With<GlobalRng>>,
) {
    if keyboard_input.just_pressed(KeyCode::Space)
        && let DiffusionConfig::Enabled { neighbourhood, .. } = &mut *diffusion
    {
        *neighbourhood = match *neighbourhood {
            Neighbourhood::VonNeumann => Neighbourhood::Moore,
            Neighbourhood::Moore => Neighbourhood::VonNeumann,
        };

        println!("Switched to {neighbourhood:?}");
    }

    if keyboard_input.just_pressed(KeyCode::KeyR) {
        let seed = rng.random();
        resources.randomize(seed, RESOURCE_NOISE_SCALE, MAX_RESOURCE);
        bounds.reset_bounds();
        bounds.update_bounds(&resources, time.timestep().as_secs_f64());
        println!("Reset resource field with seed {seed}");
    }
}

#[cfg(test)]
mod tests {
    use simulation::SimulationStep;

    use super::heatmap::{Heatmap, heatmap_color};
    use super::info_panel::OnScreenInfo;
    use super::*;

    #[test]
    fn on_screen_info_tracks_diffusion_and_keyboard_input() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        configure_simulation(&mut app, 2, 2);
        app.world_mut()
            .resource_mut::<ResourceField>()
            .resources_mut()
            .copy_from_slice(&[20.0, 40.0, 60.0, 80.0]);

        app.update();
        let world = app.world_mut();
        let panel = world
            .query_filtered::<Entity, With<OnScreenInfo>>()
            .single(world)
            .unwrap();
        let text = world.get::<Text>(panel).unwrap();
        assert!(text.contains("Diffusion Neighborhood: VonNeumann"));
        assert!(text.contains("Diffusion Coefficient: 0.80"));
        assert!(text.contains("Bounds (min, max): (20.00, 80.00)"));

        world.run_schedule(SimulationStep);
        app.update();
        let text = app.world().get::<Text>(panel).unwrap();
        assert!(text.contains("Bounds (min, max): (20.46, 79.54)"));

        let previous_field = app.world().resource::<ResourceField>().resources().clone();
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.press(KeyCode::Space);
        keyboard.press(KeyCode::KeyR);
        app.update();

        let world = app.world();
        let bounds = world.resource::<BoundsNormalizer>();
        let (min, max) = bounds.bounds().unwrap();
        let text = world.get::<Text>(panel).unwrap();
        assert!(text.contains("Diffusion Neighborhood: Moore"));
        assert!(text.contains(&format!("Bounds (min, max): ({min:.2}, {max:.2})")));

        let resources = world.resource::<ResourceField>();
        assert_ne!(resources.resources(), &previous_field);

        let heatmap = world.resource::<Heatmap>();
        for (&entity, &value) in heatmap.tiles.iter().zip(resources.resources()) {
            let sprite = world.get::<Sprite>(entity).unwrap();
            assert_eq!(sprite.color, heatmap_color(bounds.normalize(value)));
        }
    }
}
