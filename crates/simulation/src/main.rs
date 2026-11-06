use bevy::{platform::collections::HashMap, prelude::*};

use simulation::{demo, grid::GridCoord, rendering};

#[derive(Component, Clone, Copy, Debug)]
pub struct Node {
    pub energy: f32,
}

#[derive(Resource, Default)]
pub struct NodeManager {
    pub nodes: HashMap<GridCoord, Entity>,
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, rendering::RenderingPlugin))
        .add_systems(Startup, demo::setup.after(rendering::setup))
        .run();
}
