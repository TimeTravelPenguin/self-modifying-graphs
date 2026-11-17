//! Bevy setup and fixed-step scheduling for a resource field.

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
    /// Leave the field unchanged during the diffusion phase.
    #[default]
    None,
    /// Apply the selected stencil on each simulation step.
    Enabled {
        /// Cells that contribute to a diffusion update.
        neighbourhood: Neighbourhood,
        /// Finite, nonnegative diffusion coefficient in squared distance per unit of model time.
        coefficient: f64,
        /// Finite, positive distance between adjacent cells along a grid axis.
        spacing: f64,
    },
}

impl DiffusionConfig {
    /// Checks the configuration against the chosen model timestep in seconds.
    ///
    /// # Errors
    ///
    /// Returns a [`DiffusionError`] when enabled settings contain an invalid
    /// coefficient, timestep or spacing, or exceed the stencil's stability limit.
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
///
/// The model timestep measures how far the simulation advances. Bevy's
/// `Time<Fixed>` resource controls when a fixed step runs.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct SimulationTime(
    /// Fixed clock containing elapsed model time and the most recent model timestep.
    pub Time<Fixed>,
);

/// Samples the initial field bounds before Bevy's `Startup` schedule.
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

/// Creates a seeded [`ResourceField`] and installs its Bevy simulation schedules.
///
/// Install alongside Bevy's `TimePlugin`, which is included in `MinimalPlugins`
/// and `DefaultPlugins`. The default uses a 100 by 100 grid, a fresh seed, and
/// disabled diffusion. Add model rules to [`SimulationSet::Step`] in
/// [`SimulationStep`] to apply them before the display bounds are sampled.
///
/// # Examples
///
/// ```no_run,standalone_crate
/// use bevy::prelude::*;
/// use simulation::{
///     DiffusionConfig, SimulationPlugin, SimulationSet, SimulationStep,
///     field::{Neighbourhood, ResourceField},
/// };
///
/// fn replenish(mut field: ResMut<ResourceField>) {
///     for amount in field.resources_mut() {
///         *amount += 0.01;
///     }
/// }
///
/// let mut app = App::new();
/// app.add_plugins((
///     MinimalPlugins,
///     SimulationPlugin::default()
///         .with_seed(42)
///         .with_diffusion(DiffusionConfig::Enabled {
///             neighbourhood: Neighbourhood::VonNeumann,
///             coefficient: 0.8,
///             spacing: 1.0,
///         }),
/// ));
/// app.add_systems(SimulationStep, replenish.in_set(SimulationSet::Step));
/// ```
///
/// # Panics
///
/// Plugin installation panics on a zero timestep, invalid field settings, or
/// enabled diffusion that fails [`DiffusionConfig::validate`]. Validate runtime
/// changes to enabled diffusion against [`SimulationTime`] before applying them.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationPlugin {
    /// Number of columns in the resource field.
    pub width: u32,
    /// Number of rows in the resource field.
    pub height: u32,
    /// Seed for the initial field and Bevy RNG. `None` draws a fresh seed.
    pub seed: Option<u32>,
    /// Perlin noise scale measured in grid cells.
    pub perlin_scale: f64,
    /// Upper bound on the initial resource amount in a cell.
    pub max_resource: f64,
    /// Normalizer copied into the Bevy world and initialized from the field.
    pub bounds_normalizer: BoundsNormalizer,
    /// Initial settings for the mutable [`DiffusionConfig`] resource.
    pub diffusion: DiffusionConfig,
    /// Bevy fixed interval. `None` keeps Bevy's configured interval.
    pub step_interval: Option<Duration>,
    /// Model time advanced per tick. `None` uses the fixed interval at installation.
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
    /// Sets the field size in columns and rows.
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Uses a repeatable seed for resource generation and the Bevy RNG.
    pub fn with_seed(mut self, seed: u32) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Sets the Perlin noise scale in grid cells. Larger values give broader features.
    pub fn with_perlin_scale(mut self, scale: f64) -> Self {
        self.perlin_scale = scale;
        self
    }

    /// Sets the upper bound on the initial resource amount in a cell.
    pub fn with_max_resource(mut self, max_resource: f64) -> Self {
        self.max_resource = max_resource;
        self
    }

    /// Gives mutable access to the bounds normalizer before plugin installation.
    pub fn bounds_normalizer(&mut self) -> &mut BoundsNormalizer {
        &mut self.bounds_normalizer
    }

    /// Sets whether and how the field diffuses on each step.
    pub fn with_diffusion(mut self, diffusion: DiffusionConfig) -> Self {
        self.diffusion = diffusion;
        self
    }

    /// Sets the nonzero interval used by Bevy's fixed clock.
    pub fn with_step_interval(mut self, interval: Duration) -> Self {
        self.step_interval = Some(interval);
        self
    }

    /// Sets the nonzero model time advanced per tick. Defaults to Bevy's fixed timestep.
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
