use std::time::Duration;

use bevy::prelude::*;
use bevy_rand::{global::GlobalRng, plugin::EntropyPlugin, prelude::ChaCha12Rng};
use rand::RngExt;
use simulation::{
    field::{Diffuser, Neighbourhood, ResourceField},
    normalization::{BoundsAveraging, BoundsNormalizer},
};

mod heatmap;
mod info_panel;

use heatmap::{spawn_heatmap, update_heatmap};
use info_panel::{spawn_on_screen_info, update_on_screen_info};

const GRID_ROWS: u32 = 100;
const GRID_COLS: u32 = 100;
const RANDOM_SEED: u32 = 43;
const SCALE: f64 = 10.0;
const MAX_RESOURCE: f64 = 100.0;

const DIFFUSION_COEFFICIENT: f64 = 0.8;
const DIFFUSION_DT: f64 = 0.1;
const DIFFUSION_SPACING: f64 = 1.0;
const STEP_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Resource, Deref, DerefMut)]
struct DiffuserRes(Diffuser);

#[derive(Resource, Deref, DerefMut)]
struct ResourceFieldRes(ResourceField);

#[derive(Resource, Deref, DerefMut)]
struct BoundsNormalizerRes(BoundsNormalizer);

fn main() -> AppExit {
    let resources =
        ResourceField::new_random(GRID_ROWS, GRID_COLS, RANDOM_SEED, SCALE, MAX_RESOURCE);

    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1280, 850).into(),
                ..default()
            }),
            ..default()
        }),
        EntropyPlugin::<ChaCha12Rng>::default(),
    ));

    configure_simulation(&mut app, resources);

    println!("{:#?}", WindowPlugin::default().primary_window);
    app.run()
}

fn configure_simulation(app: &mut App, resources: ResourceField) {
    let diffuser = Diffuser::new(Neighbourhood::VonNeumann);
    let mut bounds_norm = BoundsNormalizer::new(BoundsAveraging::EMA { tau: 1.0 });
    bounds_norm.update_bounds(&resources, DIFFUSION_DT);

    app.insert_resource(Time::<Fixed>::from_duration(STEP_INTERVAL))
        .insert_resource(DiffuserRes(diffuser))
        .insert_resource(BoundsNormalizerRes(bounds_norm))
        .insert_resource(ResourceFieldRes(resources))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, apply_diffusion)
        .add_systems(Update, handle_keyboard_input)
        .add_systems(
            Update,
            (update_heatmap, update_on_screen_info).after(handle_keyboard_input),
        );
}

fn setup(
    mut commands: Commands,
    resources: Res<ResourceFieldRes>,
    bounds: Res<BoundsNormalizerRes>,
    diffuser: Res<DiffuserRes>,
) {
    commands.spawn(Camera2d);

    spawn_heatmap(&mut commands, &resources, &bounds);
    spawn_on_screen_info(&mut commands, &diffuser, &bounds);
}

fn apply_diffusion(
    mut diffuser: ResMut<DiffuserRes>,
    mut resources: ResMut<ResourceFieldRes>,
    mut bounds: ResMut<BoundsNormalizerRes>,
) {
    diffuser.diffuse(
        &mut resources,
        DIFFUSION_COEFFICIENT,
        DIFFUSION_DT,
        DIFFUSION_SPACING,
    );

    bounds.update_bounds(&resources, DIFFUSION_DT);
}

fn handle_keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut resources: ResMut<ResourceFieldRes>,
    mut diffuser: ResMut<DiffuserRes>,
    mut bounds: ResMut<BoundsNormalizerRes>,
    mut rng: Single<&mut ChaCha12Rng, With<GlobalRng>>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        let neighbourhood = match diffuser.neighbourhood() {
            Neighbourhood::VonNeumann => Neighbourhood::Moore,
            Neighbourhood::Moore => Neighbourhood::VonNeumann,
        };

        diffuser.set_neighbourhood(neighbourhood);
        println!("Switched to {:?}", neighbourhood);
    }

    if keyboard_input.just_pressed(KeyCode::KeyR) {
        let seed = rng.random();
        resources.randomize(seed, SCALE, MAX_RESOURCE);
        bounds.reset_bounds();
        bounds.update_bounds(&resources, DIFFUSION_DT);
        println!("Reset resource field with seed {seed}");
    }
}

#[cfg(test)]
mod tests {
    use super::heatmap::{Heatmap, heatmap_color};
    use super::info_panel::OnScreenInfo;
    use super::*;

    #[test]
    fn on_screen_info_tracks_diffusion_and_keyboard_input() {
        let mut resources = ResourceField::new_random(2, 2, RANDOM_SEED, SCALE, MAX_RESOURCE);
        resources
            .resources_mut()
            .copy_from_slice(&[20.0, 40.0, 60.0, 80.0]);

        let mut app = App::new();
        app.add_plugins(EntropyPlugin::<ChaCha12Rng>::with_seed([0; 32]))
            .init_resource::<ButtonInput<KeyCode>>();
        configure_simulation(&mut app, resources);

        app.update();
        let world = app.world_mut();
        let panel = world
            .query_filtered::<Entity, With<OnScreenInfo>>()
            .single(world)
            .unwrap();
        let text = world.get::<Text>(panel).unwrap();
        assert!(text.contains("Diffusion Neighborhood: VonNeumann"));
        assert!(text.contains("Bounds (min, max): (20.00, 80.00)"));

        world.run_schedule(FixedUpdate);
        app.update();
        let text = app.world().get::<Text>(panel).unwrap();
        assert!(text.contains("Bounds (min, max): (20.46, 79.54)"));

        let previous_field = app
            .world()
            .resource::<ResourceFieldRes>()
            .resources()
            .clone();
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.press(KeyCode::Space);
        keyboard.press(KeyCode::KeyR);
        app.update();

        let world = app.world();
        let bounds = world.resource::<BoundsNormalizerRes>();
        let (min, max) = bounds.bounds().unwrap();
        let text = world.get::<Text>(panel).unwrap();
        assert!(text.contains("Diffusion Neighborhood: Moore"));
        assert!(text.contains(&format!("Bounds (min, max): ({min:.2}, {max:.2})")));

        let resources = world.resource::<ResourceFieldRes>();
        assert_ne!(resources.resources(), &previous_field);

        let heatmap = world.resource::<Heatmap>();
        for (&entity, &value) in heatmap.tiles.iter().zip(resources.resources()) {
            let sprite = world.get::<Sprite>(entity).unwrap();
            assert_eq!(sprite.color, heatmap_color(bounds.normalize(value)));
        }
    }
}
