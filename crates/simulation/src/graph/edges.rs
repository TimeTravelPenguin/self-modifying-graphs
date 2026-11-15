use bevy::{
    platform::collections::HashMap,
    prelude::{Deref, DerefMut, Entity},
};

use super::VertexEntity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deref, DerefMut)]
pub struct EdgeEntity(Entity);

impl EdgeEntity {
    pub fn new(entity: Entity) -> Self {
        Self(entity)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EdgeEntityMap {
    edges: HashMap<VertexEntity, HashMap<VertexEntity, EdgeEntity>>,
}

impl EdgeEntityMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, from: VertexEntity, to: VertexEntity, entity: EdgeEntity) {
        let previous = self.edges.entry(from).or_default().insert(to, entity);

        if from != to {
            let reverse = self.edges.entry(to).or_default().insert(from, entity);
            debug_assert_eq!(previous, reverse, "EdgeEntityMap should be symmetric");
        }
    }

    pub fn get(&self, from: &VertexEntity, to: &VertexEntity) -> Option<&EdgeEntity> {
        self.edges.get(from).and_then(|inner| inner.get(to))
    }

    pub fn remove(&mut self, from: &VertexEntity, to: &VertexEntity) -> Option<EdgeEntity> {
        let removed = self.edges.get_mut(from).and_then(|inner| inner.remove(to));

        if from != to {
            let reverse = self.edges.get_mut(to).and_then(|inner| inner.remove(from));
            debug_assert_eq!(removed, reverse, "EdgeEntityMap should be symmetric");
        }

        removed
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::World;

    use super::{EdgeEntity, EdgeEntityMap, VertexEntity};

    #[test]
    fn self_loop_coexists_with_an_undirected_edge() {
        let mut world = World::new();
        let source = VertexEntity::new(world.spawn_empty().id());
        let target = VertexEntity::new(world.spawn_empty().id());
        let self_loop = EdgeEntity::new(world.spawn_empty().id());
        let edge = EdgeEntity::new(world.spawn_empty().id());

        let mut edges = EdgeEntityMap::new();
        edges.insert(source, source, self_loop);
        edges.insert(source, target, edge);

        assert_eq!(edges.get(&source, &source), Some(&self_loop));
        assert_eq!(edges.get(&source, &target), Some(&edge));
        assert_eq!(edges.get(&target, &source), Some(&edge));

        assert_eq!(edges.remove(&source, &source), Some(self_loop));
        assert_eq!(edges.get(&source, &source), None);
        assert_eq!(edges.remove(&target, &source), Some(edge));
        assert_eq!(edges.get(&source, &target), None);
        assert_eq!(edges.get(&target, &source), None);
    }
}
