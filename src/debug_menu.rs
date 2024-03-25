use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::McCamera;

fn show_gui(mut contexts: EguiContexts, camera_query: Query<&Transform, With<McCamera>>) {
    let camera_transform = camera_query.single();
    let (yaw, pitch, _) = camera_transform.rotation.to_euler(EulerRot::YXZ);

    egui::Window::new("Debug Info").show(contexts.ctx_mut(), |ui| {
        egui::Grid::new("debug_info_grid")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Camera Position");
                ui.label(format!("{:?}", camera_transform.translation));
                ui.end_row();

                ui.label("Camera Pitch");
                ui.label(format!("{:?}", pitch.to_degrees()));
                ui.end_row();

                ui.label("Camera Yaw");
                ui.label(format!("{:?}", yaw.to_degrees()));
                ui.end_row();
            });
    });
}

pub struct McDebugMenuPlugin;

impl Plugin for McDebugMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin).add_systems(Update, show_gui);
    }
}
