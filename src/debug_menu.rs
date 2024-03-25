use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

use crate::McCamera;

fn show_gui(mut contexts: EguiContexts, camera_query: Query<&Transform, With<McCamera>>) {
    let camera_transform = camera_query.single();
    egui::Window::new("Debug Info").show(contexts.ctx_mut(), |ui| {
        ui.label(format!(
            "Camera Position: {:?}",
            camera_transform.translation
        ));
    });
}

pub struct McDebugMenuPlugin;

impl Plugin for McDebugMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin).add_systems(Update, show_gui);
    }
}
