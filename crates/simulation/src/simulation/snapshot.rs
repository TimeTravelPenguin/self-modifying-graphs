use std::sync::{Arc, Mutex};

use bevy::ecs::resource::Resource;

use crate::{Position, field::ResourceField, graph::VertexId};

/// Owned field and vertex state at a completed tick.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationSnapshot {
    /// Seed used to generate the initial field and choose the starting vertex position.
    pub seed: u32,
    /// Seed used to generate the current field, including any reseed during the run.
    pub field_seed: u32,
    /// Number of completed ticks, starting at zero.
    pub tick: u64,
    /// Elapsed model time, calculated as the completed tick count times the timestep.
    pub simulation_time: f64,
    /// Owned copy of the resource field for this tick.
    pub field: ResourceField,
    /// Vertex state at this tick, in graph iteration order.
    pub vertices: Vec<VertexSnapshot>,
}

/// A vertex's state at a completed tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VertexSnapshot {
    /// Identifier scoped to the graph that issued it.
    pub id: VertexId,
    /// Cell occupied by this vertex.
    pub position: Position,
    /// Current resource balance after uptake and living costs.
    pub stored_resource: f64,
}

/// Publishes completed states from a single run, keeping the latest one available.
///
/// Publishing replaces the current snapshot. Readers holding the previous
/// snapshot keep it alive through their [`Arc`], and readers that poll less
/// often than the publisher runs can skip intermediate ticks.
pub struct SnapshotPublisher {
    reader: SnapshotReader,
}

/// Reads the latest publication and can retain it across updates.
#[derive(Clone, Resource)]
pub struct SnapshotReader {
    current: Arc<Mutex<Arc<SimulationSnapshot>>>,
}

impl SnapshotPublisher {
    /// Makes the supplied snapshot available to readers immediately.
    pub fn new(initial: SimulationSnapshot) -> Self {
        let current = Arc::new(Mutex::new(Arc::new(initial)));

        Self {
            reader: SnapshotReader { current },
        }
    }

    /// Returns a reader that can be cloned and shared across threads.
    pub fn reader(&self) -> SnapshotReader {
        self.reader.clone()
    }

    /// Makes a completed tick available to every reader.
    ///
    /// Publish snapshots from the same run in tick order.
    pub fn publish(&mut self, snapshot: SimulationSnapshot) {
        let mut next = Arc::new(snapshot);

        std::mem::swap(
            &mut *self.reader.current.lock().expect("snapshot lock poisoned"),
            &mut next,
        );
    }
}

impl SnapshotReader {
    /// Acquires the current snapshot for use across later publications.
    ///
    /// The returned [`Arc`] shares the published field and vertices.
    pub fn latest(&self) -> Arc<SimulationSnapshot> {
        Arc::clone(&self.current.lock().expect("snapshot lock poisoned"))
    }
}
