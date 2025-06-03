mod terrain;
mod zombie_state;

use bevy::prelude::*;
#[cfg(feature = "2D")]
use bevy_life::CellularAutomatonPlugin;
use bevy_life::{MooreCell2d, SimulationBatch};

#[cfg(feature = "2D")]
use crate::zombie_state::ZombieState;

#[cfg(feature = "2D")]
pub type ZombiePlugin = CellularAutomatonPlugin<MooreCell2d, ZombieState>;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Zombie Test".to_string(),
                resolution: (1200.0, 800.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ZombiePlugin::default())
        .insert_resource(SimulationBatch)
        .add_systems(Startup, (setup_camera, setup_map))
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn setup_map(mut commands: Commands) {
    spawn_map(&mut commands);
}

fn spawn_map(commands: &mut Commands) {
    let (size_x, size_y) = (150, 100);
    let sprite_size = 2.;
    let terrain = terrain::TerrainGenerator::new(42).generate(size_x, size_y, 5, 100.0);
    let color = Color::srgba(0., 0., 0., 0.);

    commands
        .spawn((
            Transform::from_xyz(
                -(size_x as f32 * sprite_size) / 2.,
                -(size_y as f32 * sprite_size) / 2.,
                0.,
            ),
            Visibility::default(),
        ))
        .with_children(|builder| {
            for y in 0..size_y {
                for x in 0..size_x {
                    let mut gen_at_location: Vec<i32> = vec![0; 9];
                    gen_at_location[0] = x as i32; // X coordinate
                    gen_at_location[1] = y as i32; // Y coordinate
                    gen_at_location[2] = terrain[y][x][0] as i32; // Altitude
                    gen_at_location[3] = terrain[y][x][1] as i32; // Temperature

                    // Temporary, randomly assign cells as human, zombie, empty, and with population
                    let random_state = rand::random::<u8>() % 4; // Randomly choose between 0-3
                    gen_at_location[4] = match random_state {
                        0 => 0, // Empty
                        1 => 1, // Zombie
                        2 => 2, // Human
                        _ => 0, // Default to empty
                    };
                    // If human, give a big population. If zombie, a small one.
                    gen_at_location[5] = if gen_at_location[4] == 2 {
                        (rand::random::<u8>() % 100 + 50) as i32 // Humans have a population between 50-150
                    } else if gen_at_location[4] == 1 {
                        (rand::random::<u8>() % 10 + 1) as i32 // Zombies have a population between 1-10
                    } else {
                        0 // Empty cells have no population
                    };
                    let state = zombie_state::ZombieState(gen_at_location);
                    builder.spawn((
                        Sprite {
                            custom_size: Some(Vec2::splat(sprite_size)),
                            color,
                            ..default()
                        },
                        Transform::from_xyz(sprite_size * x as f32, sprite_size * y as f32, 0.),
                        MooreCell2d::new(IVec2::new(x as i32, y as i32)),
                        state,
                    ));
                }
            }
        });
    println!("Map spawned with size: {}x{}", size_x, size_y);
}