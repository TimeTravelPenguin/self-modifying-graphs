use std::sync::{Arc, Mutex};

use crate::{Position, field::ResourceField, graph::VertexId};

/// Owned field and vertex state at a completed tick.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationSnapshot {
    pub seed: u32,
    pub tick: u64,
    pub simulation_time: f64,
    pub field: ResourceField,
    pub vertices: Vec<VertexSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VertexSnapshot {
    pub id: VertexId,
    pub position: Position,
    pub stored_resource: f64,
}

/// Publishes completed states from a single run, keeping the latest one available.
pub struct SnapshotPublisher {
    reader: SnapshotReader,
}

/// Reads the latest publication and can retain it across updates.
#[derive(Clone)]
pub struct SnapshotReader {
    current: Arc<Mutex<Arc<SimulationSnapshot>>>,
}

impl SnapshotPublisher {
    /// Makes the initial state available before the first tick.
    pub fn new(initial: SimulationSnapshot) -> Self {
        let current = Arc::new(Mutex::new(Arc::new(initial)));

        Self {
            reader: SnapshotReader { current },
        }
    }

    pub fn reader(&self) -> SnapshotReader {
        self.reader.clone()
    }

    /// Makes a completed tick available to every reader.
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
    pub fn latest(&self) -> Arc<SimulationSnapshot> {
        Arc::clone(&self.current.lock().expect("snapshot lock poisoned"))
    }
}
