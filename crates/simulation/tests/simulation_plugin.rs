use std::time::Duration;

use bevy::{
    app::FixedMain,
    prelude::*,
    time::{TimePlugin, TimeUpdateStrategy},
};
use simulation::{
    DiffusionConfig, SimulationPlugin, SimulationSet, SimulationStartup, SimulationStep,
    SimulationTime,
    field::{Neighbourhood, ResourceField},
    normalization::BoundsNormalizer,
};

#[derive(Resource, Default)]
struct ScheduleRuns {
    startup: bool,
    steps: u32,
}

#[test]
fn default_diffusion_keeps_the_field_while_the_simulation_runs() {
    let mut app = App::new();
    app.add_plugins((
        TimePlugin,
        SimulationPlugin::default()
            .with_dimensions(2, 1)
            .with_seed(7),
    ))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(0))
    .init_resource::<ScheduleRuns>()
    .add_systems(SimulationStartup, |mut runs: ResMut<ScheduleRuns>| {
        runs.startup = true;
    })
    .add_systems(
        Startup,
        |runs: Res<ScheduleRuns>, bounds: Res<BoundsNormalizer>| {
            assert!(runs.startup);
            assert_eq!(bounds.bounds(), Some((8.0, 16.0)));
        },
    )
    .add_systems(
        SimulationStep,
        (|mut runs: ResMut<ScheduleRuns>| {
            runs.steps += 1;
        })
        .in_set(SimulationSet::Step),
    );

    app.world_mut()
        .resource_mut::<ResourceField>()
        .resources_mut()
        .copy_from_slice(&[8.0, 16.0]);
    app.update();

    assert_eq!(
        *app.world().resource::<DiffusionConfig>(),
        DiffusionConfig::None
    );
    assert_eq!(
        app.world().resource::<SimulationTime>().elapsed(),
        Duration::ZERO
    );

    app.insert_resource(TimeUpdateStrategy::FixedTimesteps(2));
    app.update();

    let world = app.world();
    assert_eq!(world.resource::<ScheduleRuns>().steps, 2);
    let time = world.resource::<SimulationTime>();
    assert_eq!(time.elapsed(), time.timestep() * 2);
    assert_eq!(world.resource::<ResourceField>().resources(), &[8.0, 16.0]);
    assert_eq!(
        world.resource::<BoundsNormalizer>().bounds(),
        Some((8.0, 16.0))
    );
}

#[test]
fn fixed_main_diffuses_then_runs_the_step_hook_and_updates_bounds() {
    let model_timestep = Duration::from_millis(500);
    let step_interval = Duration::from_millis(20);
    let mut app = App::new();
    app.add_plugins(
        SimulationPlugin::default()
            .with_dimensions(2, 1)
            .with_seed(7)
            .with_diffusion(DiffusionConfig::Enabled {
                neighbourhood: Neighbourhood::VonNeumann,
                coefficient: 0.5,
                spacing: 1.0,
            })
            .with_model_timestep(model_timestep)
            .with_step_interval(step_interval),
    )
    .add_systems(FixedUpdate, |mut field: ResMut<ResourceField>| {
        field.resources_mut().copy_from_slice(&[0.0, 8.0]);
    })
    .add_systems(
        SimulationStep,
        (|mut field: ResMut<ResourceField>, bounds: Res<BoundsNormalizer>| {
            assert_eq!(field.resources(), &[2.0, 6.0]);
            assert_eq!(bounds.bounds(), Some((0.0, 0.0)));
            field.resources_mut()[0] = 10.0;
        })
        .in_set(SimulationSet::Step),
    );

    app.world_mut()
        .resource_mut::<ResourceField>()
        .resources_mut()
        .fill(0.0);
    app.update();
    app.world_mut().run_schedule(FixedMain);

    let world = app.world();
    assert_eq!(world.resource::<Time<Fixed>>().timestep(), step_interval);
    let time = world.resource::<SimulationTime>();
    assert_eq!(time.timestep(), model_timestep);
    assert_eq!(time.elapsed(), model_timestep);
    assert_eq!(world.resource::<ResourceField>().resources(), &[10.0, 6.0]);
    assert_eq!(
        world.resource::<BoundsNormalizer>().bounds(),
        Some((6.0, 10.0))
    );
}
