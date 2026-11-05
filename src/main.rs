mod demo;
mod grid;
pub mod rendering;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, rendering::RenderingPlugin))
        .add_systems(Startup, demo::setup.after(rendering::setup))
        .run();
}
