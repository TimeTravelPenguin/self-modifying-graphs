use crate::{
    field::{DiffusionError, ResourceField},
    graph::{Graph, Uptake},
};

use super::{Simulation, SimulationConfig, SimulationError, SimulationSnapshot, VertexSnapshot};

/// A seeded simulation driven one fixed timestep at a time.
pub struct SimulationRun {
    simulation: Simulation,
    seed: u32,
    dt: f64,
    tick: u64,
}

/// The state of a run after the indicated number of completed ticks.
pub struct SimulationView<'a> {
    pub seed: u32,
    pub tick: u64,
    pub simulation_time: f64,
    pub field: &'a ResourceField,
    pub graph: &'a Graph,
}

impl SimulationRun {
    /// Creates a run with one vertex and a finite, positive, stable timestep.
    ///
    /// # Panics
    ///
    /// Panics if the resource noise scale is non-positive or non-finite, or if
    /// the initial resource amount is negative or non-finite.
    pub fn single_vertex_from_seed(
        config: SimulationConfig,
        seed: u32,
        uptake: Uptake,
        dt: f64,
    ) -> Result<Self, SimulationError> {
        if dt == 0.0 {
            return Err(DiffusionError::InvalidTimestep(dt).into());
        }

        config
            .neighbourhood
            .diffusion_number(config.diffusion_coefficient, dt, config.spacing)?;
        let simulation = Simulation::single_vertex_from_seed(config, seed, uptake)?;

        Ok(Self {
            simulation,
            seed,
            dt,
            tick: 0,
        })
    }

    /// Completes exactly one simulation tick.
    pub fn advance(&mut self) {
        let next_tick = self.tick.checked_add(1).expect("simulation tick overflow");
        self.simulation.step(self.dt);
        self.tick = next_tick;
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
    pub fn snapshot(&self) -> SimulationSnapshot {
        let view = self.view();
        let vertices = view
            .graph
            .vertices()
            .map(|(id, vertex)| VertexSnapshot {
                id,
                position: vertex.position(),
                stored_resource: vertex.stored_resource(),
            })
            .collect();

        SimulationSnapshot {
            seed: view.seed,
            tick: view.tick,
            simulation_time: view.simulation_time,
            field: view.field.clone(),
            vertices,
        }
    }
}
