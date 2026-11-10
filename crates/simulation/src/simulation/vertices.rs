use bevy::{
    platform::collections::HashMap,
    prelude::{Deref, DerefMut, Entity},
};

use crate::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deref, DerefMut)]
pub struct VertexEntity(Entity);

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VertexEntityMap {
    vertices: HashMap<Position, VertexEntity>,
}

impl VertexEntityMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, position: Position, entity: VertexEntity) {
        self.vertices.insert(position, entity);
    }

    pub fn get(&self, position: &Position) -> Option<&VertexEntity> {
        self.vertices.get(position)
    }

    pub fn remove(&mut self, position: &Position) -> Option<VertexEntity> {
        self.vertices.remove(position)
    }
}
