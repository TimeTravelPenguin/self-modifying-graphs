use crate::{
    effects::EffectsConfig,
    field::{DiffusionError, Neighbourhood, ResourceField},
    graph::{Graph, VertexConfig, VertexId},
};

use super::{Simulation, SimulationConfig, SimulationError, SimulationSnapshot, VertexSnapshot};

/// A seeded simulation driven one fixed timestep at a time.
///
/// Publish tick zero to make the initial field available before stepping. A
/// reader can keep that snapshot while the run advances.
///
/// # Examples
///
/// ```no_run,standalone_crate
/// use simulation::{
///     field::Neighbourhood,
///     graph::{HealthConfig, Uptake, VertexConfig},
///     simulation::{SimulationConfig, SimulationRun, SnapshotPublisher},
/// };
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = SimulationConfig {
///     width: 8,
///     height: 6,
///     spacing: 1.0,
///     initial_resource: 100.0,
///     resource_noise_scale: 10.0,
///     diffusion_coefficient: 0.8,
///     neighbourhood: Neighbourhood::Moore,
/// };
///
/// let uptake = Uptake::new(1.0, 0.8).unwrap();
/// let vertex = VertexConfig::new(uptake, 0.0, HealthConfig::default())?;
/// let mut run = SimulationRun::single_vertex_from_seed(config, 42, vertex, 0.1)?;
/// let mut publisher = SnapshotPublisher::new(run.snapshot());
/// let reader = publisher.reader();
/// let initial = reader.latest();
///
/// run.advance();
/// publisher.publish(run.snapshot());
///
/// assert_eq!(initial.tick, 0);
/// assert_eq!(reader.latest().tick, 1);
/// # Ok(())
/// # }
/// ```
pub struct SimulationRun {
    simulation: Simulation,
    seed: u32,
    field_seed: u32,
    dt: f64,
    tick: u64,
}

/// A borrowed view of a run at its most recent completed tick.
pub struct SimulationView<'a> {
    /// Seed used to generate the initial field and choose the starting vertex position.
    pub seed: u32,
    /// Number of completed ticks, starting at zero.
    pub tick: u64,
    /// Elapsed model time, calculated as the completed tick count times the timestep.
    pub simulation_time: f64,
    /// Resource field at this tick.
    pub field: &'a ResourceField,
    /// Graph at this tick, including current vertex balances.
    pub graph: &'a Graph,
}

impl SimulationRun {
    /// Creates a run with one vertex and a finite, positive, stable timestep.
    /// Pass a [`VertexConfig`] to set the starting vertex's living cost.
    ///
    /// # Errors
    ///
    /// Returns [`SimulationError::Diffusion`] for invalid or unstable diffusion
    /// settings, including a non-positive timestep, and
    /// [`SimulationError::EmptyGrid`] if either field dimension is zero.
    ///
    /// # Panics
    ///
    /// Panics on invalid field settings as in [`ResourceField::new_random`].
    pub fn single_vertex_from_seed(
        config: SimulationConfig,
        seed: u32,
        vertex_config: VertexConfig,
        dt: f64,
    ) -> Result<Self, SimulationError> {
        if dt == 0.0 {
            return Err(DiffusionError::InvalidTimestep(dt).into());
        }

        config
            .neighbourhood
            .diffusion_number(config.diffusion_coefficient, dt, config.spacing)?;
        let simulation = Simulation::single_vertex_from_seed(config, seed, vertex_config)?;

        Ok(Self {
            simulation,
            seed,
            field_seed: seed,
            dt,
            tick: 0,
        })
    }

    /// Completes exactly one simulation tick.
    ///
    /// Applies diffusion, uptake, costs, health, aging, and death before advancing the tick count.
    ///
    /// # Panics
    ///
    /// Panics before stepping if the tick count would overflow.
    pub fn advance(&mut self) {
        let next_tick = self.tick.checked_add(1).expect("simulation tick overflow");
        self.simulation.step(self.dt);
        self.tick = next_tick;
    }

    /// Replaces a live vertex's effects for the next call to [`Self::advance`].
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Simulation::set_vertex_effects`].
    pub fn set_vertex_effects(
        &mut self,
        vertex_id: VertexId,
        effects: EffectsConfig,
    ) -> Result<u64, SimulationError> {
        self.simulation.set_vertex_effects(vertex_id, effects)
    }

    /// Number of completed ticks.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Switches to the other diffusion stencil if it is stable at this run's timestep.
    pub(crate) fn toggle_neighbourhood(&mut self) -> Result<Neighbourhood, SimulationError> {
        let neighbourhood = match self.simulation.diffuser.neighbourhood() {
            Neighbourhood::VonNeumann => Neighbourhood::Moore,
            Neighbourhood::Moore => Neighbourhood::VonNeumann,
        };

        neighbourhood.diffusion_number(
            self.simulation.diffusion_coefficient,
            self.dt,
            self.simulation.spacing,
        )?;
        self.simulation.diffuser.set_neighbourhood(neighbourhood);
        Ok(neighbourhood)
    }

    /// Regenerates the field while preserving the graph and current tick.
    pub(crate) fn reseed_field(&mut self, seed: u32, scale: f64, max_resource: f64) {
        self.simulation.field.randomize(seed, scale, max_resource);
        self.field_seed = seed;
    }

    /// Borrows the current field and graph so the run cannot advance during a read.
    pub fn view(&self) -> SimulationView<'_> {
        SimulationView {
            seed: self.seed,
            tick: self.tick,
            simulation_time: self.tick as f64 * self.dt,
            field: self.simulation.field(),
            graph: self.simulation.graph(),
        }
    }

    /// Copies the completed tick into an owned snapshot that can outlive the run's next tick.
    ///
    /// Copies resource amounts and vertex state. The borrowed [`Self::view`]
    /// also provides access to the graph's edges.
    pub fn snapshot(&self) -> SimulationSnapshot {
        let view = self.view();
        let vertices = view
            .graph
            .vertices()
            .map(|(id, vertex)| VertexSnapshot {
                id,
                position: vertex.position(),
                stored_resource: vertex.stored_resource(),
                age: vertex.age(),
                health: vertex.health(),
                config: vertex.config().clone(),
                effects_revision: vertex.effects_revision(),
                effective_effects: vertex.effective_effects(),
                effective_recovery: vertex.effective_recovery(),
            })
            .collect();

        SimulationSnapshot {
            seed: view.seed,
            field_seed: self.field_seed,
            tick: view.tick,
            simulation_time: view.simulation_time,
            field: view.field.clone(),
            vertices,
        }
    }
}
