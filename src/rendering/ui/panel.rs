use bevy::prelude::*;

/// Loaded UI sprite assets shared across all panels.
#[derive(Resource)]
pub struct UiAssets {
    pub panel_image: Handle<Image>,
    pub panel_slicer: TextureSlicer,
}

pub fn setup_ui_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let image = asset_server.load(
        "sprites/Sprout Lands - UI Pack - Premium pack/UI Sprites/Dialouge UI/dialog box.png",
    );

    let slicer = TextureSlicer {
        border: BorderRect::all(6.0),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Stretch,
        max_corner_scale: 1.0,
    };

    commands.insert_resource(UiAssets {
        panel_image: image,
        panel_slicer: slicer,
    });
}

/// Spawn a 9-slice panel node with the given layout. Returns the entity.
pub fn spawn_panel(commands: &mut Commands, assets: &UiAssets, node: Node) -> Entity {
    commands
        .spawn((
            ImageNode {
                image: assets.panel_image.clone(),
                image_mode: NodeImageMode::Sliced(assets.panel_slicer.clone()),
                ..default()
            },
            node,
        ))
        .id()
}
