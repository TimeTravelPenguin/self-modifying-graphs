//! Resource-field and graph evolution, seeded runs, and snapshot publication.

use rand::{RngExt, SeedableRng, rngs::StdRng};
use thiserror::Error;

use crate::{
    Position,
    effects::{EffectMultipliers, EffectsConfig},
    field::{Diffuser, DiffusionError, ResourceField},
    graph::{AddEdgeError, Edge, EdgeId, Graph, Vertex, VertexConfig, VertexConfigError, VertexId},
};

mod run;
mod snapshot;

pub use crate::normalization as bounds;
pub use run::{SimulationRun, SimulationView};
pub use snapshot::{SimulationSnapshot, SnapshotPublisher, SnapshotReader, VertexSnapshot};

/// Re-exports of the field diffusion solver and its stencils.
pub mod diffusion {
    pub use crate::field::{Diffuser, Neighbourhood};
}

/// Re-export of the resource field type.
pub mod resources {
    pub use crate::field::ResourceField;
}

/// Errors from placing vertices, connecting a graph, or configuring a run.
#[derive(Debug, Error)]
pub enum SimulationError {
    /// An effect replacement addressed a vertex that is no longer alive.
    #[error("vertex {0:?} is absent from the simulation")]
    UnknownVertex(VertexId),

    /// The replacement effects are incompatible with the vertex's base quantities.
    #[error(transparent)]
    VertexConfig(#[from] VertexConfigError),

    /// A seeded run requires a cell for its first vertex.
    #[error("a starting vertex requires a grid with positive width and height")]
    EmptyGrid,

    /// The requested vertex position lies outside the field.
    #[error("position ({}, {}) is outside the field", .0.x, .0.y)]
    VertexOutOfBounds(Position),

    /// The requested cell already contains a vertex.
    #[error("position ({}, {}) is occupied", .0.x, .0.y)]
    CellOccupied(Position),

    /// The graph rejected an edge insertion.
    #[error(transparent)]
    AddEdge(#[from] AddEdgeError),

    /// The configured timestep or diffusion settings are invalid.
    #[error(transparent)]
    Diffusion(#[from] DiffusionError),
}

/// Settings for generating a seeded field and configuring its diffuser.
pub struct SimulationConfig {
    /// Number of columns along the x-axis.
    pub width: u32,
    /// Number of rows along the y-axis.
    pub height: u32,
    /// Physical distance between adjacent cell centres.
    pub spacing: f64,
    /// Upper bound on the initial resource amount in each cell.
    pub initial_resource: f64,
    /// Perlin noise scale in grid cells.
    pub resource_noise_scale: f64,
    /// Diffusion rate in squared distance per unit of model time.
    pub diffusion_coefficient: f64,
    /// Stencil used to exchange resource between cells.
    pub neighbourhood: diffusion::Neighbourhood,
}

/// A resource field and its graph, with at most one vertex per cell.
///
/// Place and remove vertices through this type to keep cell occupancy and graph
/// topology in sync. A step applies diffusion, vertex uptake, and living costs in order.
pub struct Simulation {
    field: ResourceField,
    graph: Graph,
    diffuser: Diffuser,
    diffusion_coefficient: f64,
    spacing: f64,
    occupancy: Vec<Option<VertexId>>,
    tick_effects: Vec<(VertexId, EffectMultipliers)>,
    expired_vertices: Vec<VertexId>,
}

impl Simulation {
    /// Starts with an empty graph on a grid whose values represent resource amounts per cell.
    ///
    /// # Panics
    ///
    /// Panics if a resource amount or the diffusion coefficient is negative or non-finite,
    /// or if spacing is non-positive or non-finite.
    pub fn new(
        field: ResourceField,
        diffuser: Diffuser,
        diffusion_coefficient: f64,
        spacing: f64,
    ) -> Self {
        assert!(diffusion_coefficient.is_finite() && diffusion_coefficient >= 0.0);
        assert!(spacing.is_finite() && spacing > 0.0);
        assert!(
            field
                .resources()
                .iter()
                .all(|amount| amount.is_finite() && *amount >= 0.0)
        );

        let occupancy = vec![None; field.resources().len()];

        Self {
            field,
            graph: Graph::new(),
            diffuser,
            diffusion_coefficient,
            spacing,
            occupancy,
            tick_effects: Vec::new(),
            expired_vertices: Vec::new(),
        }
    }

    /// Generates a seeded resource field and starts with an empty graph.
    ///
    /// # Panics
    ///
    /// Panics on invalid field or diffusion settings. See
    /// [`ResourceField::new_random`] and [`Simulation::new`].
    pub fn from_seed(config: SimulationConfig, seed: u32) -> Self {
        let field = ResourceField::new_random(
            config.height,
            config.width,
            seed,
            config.resource_noise_scale,
            config.initial_resource,
        );
        let diffuser = Diffuser::new(config.neighbourhood);

        Self::new(
            field,
            diffuser,
            config.diffusion_coefficient,
            config.spacing,
        )
    }

    /// Generates a seeded resource field and places one vertex in a uniformly sampled cell.
    ///
    /// The seed determines both the field and the starting position.
    /// Pass a [`VertexConfig`] to set a living cost for the starting vertex.
    ///
    /// # Errors
    ///
    /// Returns [`SimulationError::EmptyGrid`] if either dimension is zero.
    ///
    /// # Panics
    ///
    /// Panics on invalid field or diffusion settings as in [`Self::from_seed`].
    pub fn single_vertex_from_seed(
        config: SimulationConfig,
        seed: u32,
        vertex_config: VertexConfig,
    ) -> Result<Self, SimulationError> {
        if config.width == 0 || config.height == 0 {
            return Err(SimulationError::EmptyGrid);
        }

        let mut simulation = Self::from_seed(config, seed);
        let mut rng = StdRng::seed_from_u64(u64::from(seed));
        let x = rng.random_range(0..simulation.field.width());
        let y = rng.random_range(0..simulation.field.height());

        simulation.add_vertex((x, y), vertex_config)?;
        Ok(simulation)
    }

    /// Borrows the current resource field.
    pub fn field(&self) -> &ResourceField {
        &self.field
    }

    /// Borrows the current graph, including vertex balances and edges.
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Finds the vertex at a cell, if the cell is within the field and occupied.
    pub fn vertex_at(&self, position: impl Into<Position>) -> Option<VertexId> {
        let index = self.field.cell_index(position.into())?;
        self.occupancy[index]
    }

    /// Places a vertex with an initial balance of zero.
    /// Pass a [`VertexConfig`] to set a living cost for this vertex.
    ///
    /// # Errors
    ///
    /// Returns an error if the requested cell is outside the field or occupied.
    pub fn add_vertex(
        &mut self,
        position: impl Into<Position>,
        config: VertexConfig,
    ) -> Result<VertexId, SimulationError> {
        let position = position.into();
        let index = self
            .field
            .cell_index(position)
            .ok_or(SimulationError::VertexOutOfBounds(position))?;

        if self.occupancy[index].is_some() {
            return Err(SimulationError::CellOccupied(position));
        }

        let vertex_id = self.graph.add_vertex(position, config);
        self.occupancy[index] = Some(vertex_id);
        Ok(vertex_id)
    }

    /// Removes a vertex and its incident edges, freeing its cell for placement.
    ///
    /// Returns `None` if the vertex ID is stale or absent from the graph.
    pub fn remove_vertex(&mut self, vertex_id: VertexId) -> Option<Vertex> {
        let vertex = self.graph.remove_vertex(vertex_id)?;
        let index = self
            .field
            .cell_index(vertex.position())
            .expect("vertex is in the field");

        self.occupancy[index] = None;
        Some(vertex)
    }

    /// Adds an undirected edge between two vertices in this simulation.
    ///
    /// # Errors
    ///
    /// Returns [`SimulationError::AddEdge`] if an endpoint is absent or the
    /// endpoint pair already has an edge.
    pub fn add_edge(&mut self, from: VertexId, to: VertexId) -> Result<EdgeId, SimulationError> {
        self.graph.add_edge(from, to).map_err(Into::into)
    }

    /// Removes an edge by ID, if it exists.
    pub fn remove_edge(&mut self, edge_id: EdgeId) -> Option<Edge> {
        self.graph.remove_edge(edge_id)
    }

    /// Replaces one vertex's effects between steps and returns its new revision.
    ///
    /// Age, health, resource balance, and base configuration are preserved.
    ///
    /// # Errors
    ///
    /// Returns an error for an absent vertex, amplified quantities that would
    /// overflow, or an exhausted revision counter. Rejection leaves the vertex intact.
    pub fn set_vertex_effects(
        &mut self,
        vertex_id: VertexId,
        effects: EffectsConfig,
    ) -> Result<u64, SimulationError> {
        self.graph
            .vertex_mut(vertex_id)
            .ok_or(SimulationError::UnknownVertex(vertex_id))?
            .set_effects(effects)
            .map_err(Into::into)
    }

    /// Diffuses the field, then applies uptake, costs, health changes, aging, and death.
    ///
    /// `dt` is the elapsed model time. Each vertex applies its uptake law to
    /// the cell amount after diffusion, caps withdrawal at that amount, and
    /// stores the efficiency-adjusted share before paying its living cost.
    /// Effects read beginning-of-tick age and health. Every vertex completes
    /// uptake before payments begin. Living cost and health changes occur once
    /// per call, including when `dt` is zero. Age advances by `dt` units of model time.
    /// Expired vertices are removed after all updates, freeing their cells and edges.
    ///
    /// # Panics
    ///
    /// Panics before updating the field if `dt` is negative, non-finite, or exceeds the
    /// stability limit for the configured diffuser, or if a vertex's age would overflow.
    pub fn step(&mut self, dt: f64) {
        assert!(
            self.graph
                .vertices()
                .all(|(_, vertex)| vertex.can_advance_age(dt)),
            "model timestep must be finite, nonnegative, and keep vertex ages finite"
        );

        self.diffuser.diffuse(
            &mut self.field,
            self.diffusion_coefficient,
            dt,
            self.spacing,
        );

        self.tick_effects.clear();
        self.expired_vertices.clear();

        for (id, vertex) in self.graph.vertices_mut() {
            self.tick_effects.push((id, vertex.effective_effects()));
        }

        for &(id, effects) in &self.tick_effects {
            let vertex = self.graph.vertex_mut(id).expect("tick vertex is alive");
            let available = self
                .field
                .get_resource_at_mut(vertex.position())
                .expect("vertex is in the field");

            vertex.consume(available, dt, effects.uptake_capacity);
        }

        for &(id, effects) in &self.tick_effects {
            let vertex = self.graph.vertex_mut(id).expect("tick vertex is alive");
            let payment = vertex.pay_living_cost();
            vertex.finish_tick(payment, effects, dt);

            if vertex.is_expired() {
                self.expired_vertices.push(id);
            }
        }

        while let Some(id) = self.expired_vertices.pop() {
            self.remove_vertex(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        field::Neighbourhood,
        graph::{HealthConfig, Uptake},
    };

    use super::*;

    #[test]
    fn vertices_consume_resources_and_removal_frees_the_cell_and_edge() {
        let mut field = ResourceField::new_random(1, 2, 0, 1.0, 0.0);
        field.resources_mut().copy_from_slice(&[3.0, 8.0]);

        let mut simulation =
            Simulation::new(field, Diffuser::new(Neighbourhood::VonNeumann), 0.0, 1.0);
        let config =
            VertexConfig::new(Uptake::new(5.0, 0.8).unwrap(), 0.0, HealthConfig::Disabled).unwrap();
        let left = simulation.add_vertex((0, 0), config.clone()).unwrap();
        let right = simulation.add_vertex((1, 0), config.clone()).unwrap();
        let edge = simulation.add_edge(left, right).unwrap();

        assert!(matches!(
            simulation.add_vertex((0, 0), config.clone()),
            Err(SimulationError::CellOccupied(Position { x: 0, y: 0 }))
        ));
        assert!(matches!(
            simulation.add_vertex((2, 0), config.clone()),
            Err(SimulationError::VertexOutOfBounds(Position { x: 2, y: 0 }))
        ));
        assert!(matches!(
            simulation.add_edge(right, left),
            Err(SimulationError::AddEdge(AddEdgeError::DuplicateEdge(edge_id))) if edge_id == edge
        ));
        assert_eq!(simulation.vertex_at((0, 0)), Some(left));
        assert_eq!(simulation.graph().vertices().count(), 2);

        simulation.step(1.0);
        assert_eq!(simulation.field().resources(), &[0.0, 3.0]);
        assert!((simulation.graph().vertex(left).unwrap().stored_resource() - 2.4).abs() < 1e-12);
        assert_eq!(
            simulation.graph().vertex(right).unwrap().stored_resource(),
            4.0
        );

        simulation.step(1.0);
        assert_eq!(simulation.field().resources(), &[0.0, 0.0]);
        assert!((simulation.graph().vertex(left).unwrap().stored_resource() - 2.4).abs() < 1e-12);
        assert!((simulation.graph().vertex(right).unwrap().stored_resource() - 6.4).abs() < 1e-12);

        simulation.remove_vertex(left).unwrap();
        assert_eq!(simulation.vertex_at((0, 0)), None);
        assert!(simulation.graph().edge(edge).is_none());
        assert!(matches!(
            simulation.add_edge(left, right),
            Err(SimulationError::AddEdge(AddEdgeError::UnknownVertex(vertex_id))) if vertex_id == left
        ));
        assert!(
            simulation
                .graph()
                .vertex(right)
                .unwrap()
                .incident_edges()
                .is_empty()
        );

        let replacement = simulation.add_vertex((0, 0), config.clone()).unwrap();
        assert_ne!(replacement, left);
        assert!(simulation.remove_vertex(left).is_none());
        assert_eq!(simulation.vertex_at((0, 0)), Some(replacement));
    }

    #[test]
    fn uptake_sees_resources_diffused_in_the_same_step() {
        let mut field = ResourceField::new_random(1, 2, 0, 1.0, 0.0);
        field.resources_mut().copy_from_slice(&[0.0, 4.0]);

        let mut simulation =
            Simulation::new(field, Diffuser::new(Neighbourhood::VonNeumann), 0.25, 1.0);
        let vertex = simulation
            .add_vertex(
                (0, 0),
                VertexConfig::new(Uptake::new(2.0, 0.5).unwrap(), 0.0, HealthConfig::Disabled)
                    .unwrap(),
            )
            .unwrap();

        simulation.step(1.0);
        assert_eq!(simulation.field().resources(), &[0.0, 3.0]);
        assert_eq!(
            simulation.graph().vertex(vertex).unwrap().stored_resource(),
            0.5
        );
    }
}
