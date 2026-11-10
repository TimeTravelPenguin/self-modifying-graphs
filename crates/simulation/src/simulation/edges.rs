use bevy::{
    platform::collections::HashMap,
    prelude::{Deref, DerefMut, Entity},
};

use crate::simulation::vertices::VertexEntity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deref, DerefMut)]
pub struct EdgeEntity(Entity);

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EdgeEntityMap {
    edges: HashMap<VertexEntity, HashMap<VertexEntity, EdgeEntity>>,
}

impl EdgeEntityMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, from: VertexEntity, to: VertexEntity, entity: EdgeEntity) {
        let fst = self.edges.entry(from).or_default().insert(to, entity);
        let snd = self.edges.entry(to).or_default().insert(from, entity);

        debug_assert_eq!(fst, snd, "EdgeEntityMap should be symmetric");
    }

    pub fn get(&self, from: &VertexEntity, to: &VertexEntity) -> Option<&EdgeEntity> {
        self.edges.get(from).and_then(|inner| inner.get(to))
    }

    pub fn remove(&mut self, from: &VertexEntity, to: &VertexEntity) -> Option<EdgeEntity> {
        let fst = self.edges.get_mut(from).and_then(|inner| inner.remove(to));
        let snd = self.edges.get_mut(to).and_then(|inner| inner.remove(from));

        debug_assert_eq!(fst, snd, "EdgeEntityMap should be symmetric");

        fst
    }
}
