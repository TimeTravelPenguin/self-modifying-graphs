//! Header, heatmap legend, and selected vertex details.

use bevy::prelude::*;
use simulation::{
    graph::VertexId,
    rendering::{UI_COLORS, heatmap_color},
    simulation::SimulationSnapshot,
};
use slotmap::Key;

use super::layout::ViewerLayout;

/// UI nodes whose dimensions or visibility are managed by the viewer.
#[derive(Component)]
pub(super) enum UiRegion {
    Header,
    Legend,
    Inspector,
    InspectorPrompt,
    InspectorDetails,
}

/// Text nodes updated from the displayed snapshot and selection.
#[derive(Component, Debug, PartialEq, Eq)]
pub(super) enum UiText {
    RunStatus,
    InspectorPrompt,
    VertexId,
    CellPosition,
    StoredResource,
    CellResource,
    LegendMinimum,
    LegendMaximum,
}

/// Creates the header, legend, and initially unselected inspector.
pub(super) fn spawn(commands: &mut Commands, snapshot: &SimulationSnapshot, bounds: (f64, f64)) {
    spawn_header(commands, snapshot);
    spawn_legend(commands, bounds);
    spawn_inspector(commands, snapshot);
}

/// Places each UI panel in its reserved logical-pixel rectangle.
pub(super) fn apply_layout(layout: &ViewerLayout, regions: &mut Query<(&UiRegion, &mut Node)>) {
    for (region, mut node) in regions {
        let rect = match region {
            UiRegion::Header => layout.header,
            UiRegion::Legend => layout.legend,
            UiRegion::Inspector => layout.inspector,
            UiRegion::InspectorPrompt => continue,
            UiRegion::InspectorDetails => {
                let columns = if layout.compact { 2 } else { 1 };

                if node.grid_template_columns.len() != columns {
                    node.grid_template_columns = vec![GridTrack::flex(1.0); columns];
                }

                continue;
            }
        };

        let left = px(rect.min.x);
        let top = px(rect.min.y);
        let width = px(rect.width());
        let height = px(rect.height());

        if (node.left, node.top, node.width, node.height) != (left, top, width, height) {
            node.left = left;
            node.top = top;
            node.width = width;
            node.height = height;
        }
    }
}

/// Updates the visible details and colorscale labels from the displayed snapshot.
pub(super) fn refresh(
    snapshot: &SimulationSnapshot,
    selected: Option<VertexId>,
    (min, max): (f64, f64),
    texts: &mut Query<(&UiText, &mut Text)>,
    regions: &mut Query<(&UiRegion, &mut Node)>,
) {
    let vertex = selected.and_then(|id| snapshot.vertices.iter().find(|vertex| vertex.id == id));
    let cell_resource = vertex.and_then(|vertex| snapshot.field.get_resource_at(vertex.position));

    for (region, mut node) in regions {
        let display = match region {
            UiRegion::InspectorPrompt if vertex.is_none() => Display::Flex,
            UiRegion::InspectorDetails if vertex.is_some() => Display::Grid,
            UiRegion::InspectorPrompt | UiRegion::InspectorDetails => Display::None,
            _ => continue,
        };

        if node.display != display {
            node.display = display;
        }
    }

    for (label, mut text) in texts {
        let value = match label {
            UiText::RunStatus => status_text(snapshot),
            UiText::InspectorPrompt => inspector_prompt(snapshot).to_owned(),
            UiText::VertexId => vertex
                .map(|vertex| format!("{:?}", vertex.id.data()))
                .unwrap_or_default(),
            UiText::CellPosition => vertex
                .map(|vertex| format!("({}, {})", vertex.position.x, vertex.position.y))
                .unwrap_or_default(),
            UiText::StoredResource => vertex
                .map(|vertex| format!("{:.2}", vertex.stored_resource))
                .unwrap_or_default(),
            UiText::CellResource => cell_resource
                .map(|amount| format!("{amount:.2}"))
                .unwrap_or_default(),
            UiText::LegendMinimum => legend_minimum(min),
            UiText::LegendMaximum => format!("{max:.1}"),
        };

        text.set_if_neq(Text::new(value));
    }
}

fn spawn_header(commands: &mut Commands, snapshot: &SimulationSnapshot) {
    commands
        .spawn((
            UiRegion::Header,
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::horizontal(px(24.0)),
                align_items: AlignItems::Center,
                align_content: AlignContent::Center,
                justify_content: JustifyContent::SpaceBetween,
                flex_wrap: FlexWrap::Wrap,
                row_gap: px(4.0),
                column_gap: px(16.0),
                ..default()
            },
            BackgroundColor(UI_COLORS.surface0.into()),
        ))
        .with_children(|header| {
            header.spawn((
                Text::new("Graph simulation"),
                TextFont::from_font_size(21.0),
                TextColor(UI_COLORS.text.into()),
            ));
            header.spawn((
                UiText::RunStatus,
                Text::new(status_text(snapshot)),
                TextFont::from_font_size(15.0),
                TextColor(UI_COLORS.subtext1.into()),
            ));
        });
}

fn status_text(snapshot: &SimulationSnapshot) -> String {
    format!(
        "Seed {}  |  Tick {}  |  Time {:.1}",
        snapshot.seed, snapshot.tick, snapshot.simulation_time
    )
}

fn spawn_legend(commands: &mut Commands, (min, max): (f64, f64)) {
    commands
        .spawn((
            UiRegion::Legend,
            Node {
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: px(10.0),
                ..default()
            },
        ))
        .with_children(|legend| {
            legend.spawn((
                UiText::LegendMinimum,
                Text::new(legend_minimum(min)),
                TextFont::from_font_size(14.0),
                TextColor(UI_COLORS.subtext1.into()),
            ));
            legend.spawn(Node::default()).with_children(|scale| {
                for index in 0..12 {
                    scale.spawn((
                        Node {
                            width: px(12.0),
                            height: px(8.0),
                            ..default()
                        },
                        BackgroundColor(heatmap_color(index as f32 / 11.0)),
                    ));
                }
            });

            legend.spawn((
                UiText::LegendMaximum,
                Text::new(format!("{max:.1}")),
                TextFont::from_font_size(14.0),
                TextColor(UI_COLORS.subtext1.into()),
            ));
        });
}

fn legend_minimum(min: f64) -> String {
    format!("Cell resource    {min:.1}")
}

fn spawn_inspector(commands: &mut Commands, snapshot: &SimulationSnapshot) {
    commands
        .spawn((
            UiRegion::Inspector,
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(px(18.0), px(12.0)),
                row_gap: px(8.0),
                ..default()
            },
            BackgroundColor(UI_COLORS.mantle.into()),
        ))
        .with_children(|inspector| {
            inspector.spawn((
                Text::new("Vertex inspector"),
                TextFont::from_font_size(18.0),
                TextColor(UI_COLORS.text.into()),
            ));
            inspector.spawn((
                UiRegion::InspectorPrompt,
                UiText::InspectorPrompt,
                Text::new(inspector_prompt(snapshot)),
                TextFont::from_font_size(15.0),
                TextColor(UI_COLORS.subtext1.into()),
            ));
            inspector
                .spawn((
                    UiRegion::InspectorDetails,
                    Node {
                        display: Display::None,
                        grid_template_columns: vec![GridTrack::flex(1.0)],
                        column_gap: px(12.0),
                        row_gap: px(8.0),
                        ..default()
                    },
                ))
                .with_children(|details| {
                    for (heading, label) in [
                        ("Vertex ID", UiText::VertexId),
                        ("Cell coordinates (x, y)", UiText::CellPosition),
                        ("Stored resource", UiText::StoredResource),
                        ("Cell resource", UiText::CellResource),
                    ] {
                        details
                            .spawn(Node {
                                flex_direction: FlexDirection::Column,
                                ..default()
                            })
                            .with_children(|row| {
                                row.spawn((
                                    Text::new(heading),
                                    TextFont::from_font_size(12.0),
                                    TextColor(UI_COLORS.subtext1.into()),
                                ));
                                row.spawn((
                                    label,
                                    Text::default(),
                                    TextFont::from_font_size(16.0),
                                    TextColor(UI_COLORS.text.into()),
                                ));
                            });
                    }
                });
        });
}

fn inspector_prompt(snapshot: &SimulationSnapshot) -> &'static str {
    if snapshot.vertices.is_empty() {
        "No live vertices"
    } else {
        "Select a vertex"
    }
}
