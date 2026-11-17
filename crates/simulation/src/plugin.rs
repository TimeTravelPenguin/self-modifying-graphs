use std::time::Duration;

use bevy::{
    app::{FixedMainScheduleOrder, MainScheduleOrder},
    ecs::schedule::ScheduleLabel,
    prelude::*,
};
use bevy_rand::{plugin::EntropyPlugin, prelude::ChaCha12Rng};

use crate::{
    field::{Diffuser, DiffusionError, Neighbourhood, ResourceField},
    normalization::BoundsNormalizer,
};

/// Runtime diffusion settings for the Bevy simulation schedule.
#[derive(Debug, Clone, Copy, Default, PartialEq, Resource)]
pub enum DiffusionConfig {
    #[default]
    None,
    Enabled {
        neighbourhood: Neighbourhood,
        coefficient: f64,
        spacing: f64,
    },
}

impl DiffusionConfig {
    /// Checks the configuration against the chosen model timestep in seconds.
    pub fn validate(self, dt: f64) -> Result<(), DiffusionError> {
        if let Self::Enabled {
            neighbourhood,
            coefficient,
            spacing,
        } = self
        {
            neighbourhood.diffusion_number(coefficient, dt, spacing)?;
        }

        Ok(())
    }
}

/// Model time advanced by each [`SimulationStep`].
#[derive(Resource, Default, Deref, DerefMut)]
pub struct SimulationTime(pub Time<Fixed>);

/// Initializes the simulation before Bevy's `Startup` schedule.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SimulationStartup;

/// Runs once per Bevy fixed tick, after `FixedUpdate`.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SimulationStep;

/// Ordered phases of a simulation step.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SimulationSet {
    /// Diffuses the field when enabled and advances model time.
    Diffusion,
    /// Applies additional model rules after diffusion.
    Step,
    /// Samples the resulting field for display.
    Observe,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationPlugin {
    pub width: u32,
    pub height: u32,
    pub seed: Option<u32>,
    pub perlin_scale: f64,
    pub max_resource: f64,
    pub bounds_normalizer: BoundsNormalizer,
    pub diffusion: DiffusionConfig,
    pub step_interval: Option<Duration>,
    pub model_timestep: Option<Duration>,
}

impl Default for SimulationPlugin {
    fn default() -> Self {
        Self {
            width: 100,
            height: 100,
            seed: None,
            perlin_scale: 10.0,
            max_resource: 100.0,
            bounds_normalizer: BoundsNormalizer::default(),
            diffusion: DiffusionConfig::None,
            step_interval: None,
            model_timestep: None,
        }
    }
}

impl SimulationPlugin {
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_seed(mut self, seed: u32) -> Self {
        self.seed = Some(seed);
        self
    }

    pub fn with_perlin_scale(mut self, scale: f64) -> Self {
        self.perlin_scale = scale;
        self
    }

    pub fn with_max_resource(mut self, max_resource: f64) -> Self {
        self.max_resource = max_resource;
        self
    }

    pub fn bounds_normalizer(&mut self) -> &mut BoundsNormalizer {
        &mut self.bounds_normalizer
    }

    pub fn with_diffusion(mut self, diffusion: DiffusionConfig) -> Self {
        self.diffusion = diffusion;
        self
    }

    /// Sets how often Bevy runs a fixed tick.
    pub fn with_step_interval(mut self, interval: Duration) -> Self {
        self.step_interval = Some(interval);
        self
    }

    /// Sets the model time advanced per tick. Defaults to Bevy's fixed timestep.
    pub fn with_model_timestep(mut self, timestep: Duration) -> Self {
        self.model_timestep = Some(timestep);
        self
    }
}

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Time<Fixed>>();

        if let Some(interval) = self.step_interval {
            app.world_mut()
                .resource_mut::<Time<Fixed>>()
                .set_timestep(interval);
        }

        let model_timestep = self
            .model_timestep
            .unwrap_or_else(|| app.world().resource::<Time<Fixed>>().timestep());
        let time = SimulationTime(Time::from_duration(model_timestep));
        self.diffusion
            .validate(model_timestep.as_secs_f64())
            .unwrap_or_else(|error| panic!("invalid simulation diffusion settings: {error}"));

        let seed = self.seed.unwrap_or_else(rand::random);
        let resources = ResourceField::new_random(
            self.height,
            self.width,
            seed,
            self.perlin_scale,
            self.max_resource,
        );

        let seed_bytes = seed.to_le_bytes();
        let mut rng_seed = [0; 32];
        rng_seed[..seed_bytes.len()].copy_from_slice(&seed_bytes);

        app.init_schedule(SimulationStartup)
            .init_schedule(SimulationStep)
            .configure_sets(
                SimulationStep,
                (
                    SimulationSet::Diffusion,
                    SimulationSet::Step,
                    SimulationSet::Observe,
                )
                    .chain(),
            )
            .add_plugins(EntropyPlugin::<ChaCha12Rng>::with_seed(rng_seed))
            .insert_resource(time)
            .insert_resource(self.diffusion)
            .insert_resource(self.bounds_normalizer.clone())
            .insert_resource(resources)
            .add_systems(SimulationStartup, initialize_bounds)
            .add_systems(
                SimulationStep,
                (
                    apply_diffusion.in_set(SimulationSet::Diffusion),
                    update_bounds.in_set(SimulationSet::Observe),
                ),
            );

        app.world_mut()
            .resource_mut::<MainScheduleOrder>()
            .insert_startup_before(Startup, SimulationStartup);
        app.world_mut()
            .resource_mut::<FixedMainScheduleOrder>()
            .insert_after(FixedUpdate, SimulationStep);
    }
}

fn initialize_bounds(resources: Res<ResourceField>, mut bounds: ResMut<BoundsNormalizer>) {
    bounds.reset_bounds();
    bounds.update_bounds(&resources, 0.0);
}

fn apply_diffusion(
    mut diffuser: Local<Diffuser>,
    config: Res<DiffusionConfig>,
    mut resources: ResMut<ResourceField>,
    mut time: ResMut<SimulationTime>,
) {
    let timestep = time.timestep();

    if let DiffusionConfig::Enabled {
        neighbourhood,
        coefficient,
        spacing,
    } = *config
    {
        diffuser.set_neighbourhood(neighbourhood);
        diffuser.diffuse(&mut resources, coefficient, timestep.as_secs_f64(), spacing);
    }

    time.advance_by(timestep);
}

fn update_bounds(
    resources: Res<ResourceField>,
    time: Res<SimulationTime>,
    mut bounds: ResMut<BoundsNormalizer>,
) {
    bounds.update_bounds(&resources, time.delta_secs_f64());
}
