mod terrain;
mod zombie_state;

use crate::zombie_state::{Status, ZombieState};
use bevy::color::palettes::css::*;
use bevy::prelude::*;
use bevy_life::CellularAutomatonPlugin;
use bevy_life::{LifeSystemSet, MooreCell2d, SimulationBatch};

pub type ZombiePlugin = CellularAutomatonPlugin<MooreCell2d, ZombieState>;

const SCALE: i32 = 100;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Zombie Test".to_string(),
                resolution: (1900.0, 1100.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ZombiePlugin {
            tick_time_step: Some(0.1),
            ..default()
        })
        .insert_resource(SimulationBatch)
        .add_systems(Startup, (setup_camera, setup_map))
        .add_systems(PostStartup, (setup_assets, setup_views).chain())
        .add_systems(
            Update,
            (update_cell_views, state_debug).after(LifeSystemSet::CellUpdate),
        )
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

const CELL_SIZE: f32 = 12.0;
const CELL_HALF_SIZE: f32 = CELL_SIZE / 2.0;

fn setup_map(mut commands: Commands) {
    let (size_x, size_y) = (150, 75);
    let terrain = terrain::TerrainGenerator::new(42).generate(size_x, size_y, 5, 100.0);

    commands
        .spawn((Transform::from_xyz(
            -(size_x as f32 * CELL_SIZE) / 2.,
            -(size_y as f32 * CELL_SIZE) / 2.,
            0.,
        ),))
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
                    let state = zombie_state::ZombieState::from(gen_at_location);

                    builder.spawn((
                        Transform::from_xyz(CELL_SIZE * x as f32, CELL_SIZE * y as f32, 0.),
                        MooreCell2d::new(IVec2::new(x as i32, y as i32)),
                        state,
                    ));
                }
            }
        });
    println!("Map spawned with size: {}x{}", size_x, size_y);
}

#[derive(Resource)]
struct RectMesh(Handle<Mesh>);

#[derive(Resource)]
struct TerrainMaterial(Handle<ColorMaterial>);

#[derive(Resource)]
struct ZombieMaterial(Handle<ColorMaterial>);

#[derive(Resource)]
struct HumanMaterial(Handle<ColorMaterial>);

fn setup_assets(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    let rect = Rectangle::from_size(Vec2::splat(1.0));
    let rect_mesh_handle = meshes.add(rect);

    let terrain_material_handle = materials.add(Color::from(SANDY_BROWN));
    let zombie_material_handle = materials.add(Color::from(GREEN));
    let human_material_handle = materials.add(Color::from(ROYAL_BLUE));

    commands.insert_resource(RectMesh(rect_mesh_handle));

    commands.insert_resource(TerrainMaterial(terrain_material_handle));
    commands.insert_resource(ZombieMaterial(zombie_material_handle));
    commands.insert_resource(HumanMaterial(human_material_handle));
}

#[derive(Component, Clone, Copy)]
struct Humans;

#[derive(Component, Clone, Copy)]
struct Zombies;

fn setup_views(
    cells_q: Query<Entity, With<ZombieState>>,
    mut commands: Commands,
    rect_mesh: Res<RectMesh>,
    terrain_material: Res<TerrainMaterial>,
    zombie_material: Res<ZombieMaterial>,
    human_material: Res<HumanMaterial>,
) {
    let terrain = (
        Mesh2d(rect_mesh.0.clone()),
        MeshMaterial2d(terrain_material.0.clone()),
        Transform {
            translation: Vec3::new(0.0, 0.0, 1.0),
            scale: Vec3::new(CELL_SIZE, CELL_SIZE, 1.0),
            ..default()
        },
    );

    let humans = (
        Mesh2d(rect_mesh.0.clone()),
        MeshMaterial2d(human_material.0.clone()),
        Transform {
            translation: Vec3::new(0.0, 0.0, 3.0), // atop terrain
            scale: Vec3::new(0.0, 0.0, 1.0),
            ..default()
        },
        Humans,
    );

    let zombies = (
        Mesh2d(rect_mesh.0.clone()),
        MeshMaterial2d(zombie_material.0.clone()),
        Transform {
            translation: Vec3::new(0.0, 0.0, 2.0), // atop humans
            scale: Vec3::new(0.0, 0.0, 1.0),
            ..default()
        },
        Zombies,
    );

    for cell in cells_q.iter() {
        commands
            .entity(cell)
            .with_child(terrain.clone())
            .with_child(humans.clone())
            .with_child(zombies.clone());
    }
}

const CELL_MAX_POPULATION: i32 = 1000;
const CELL_MAX_HALF_POPULATION: i32 = CELL_MAX_POPULATION / 2;

fn update_cell_views(
    cells_q: Query<(&ZombieState, &Children)>,
    mut humans_tfs_q: Query<&mut Transform, (With<Humans>, Without<Zombies>)>,
    mut zombies_tfs_q: Query<&mut Transform, (With<Zombies>, Without<Humans>)>,
) {
    for (state, children) in cells_q.iter() {
        let ch = children.to_vec();
        let humans_e = ch[1];
        let zombies_e = ch[2];

        let mut humans_tf = humans_tfs_q.get_mut(humans_e).unwrap();
        let mut zombies_tf = zombies_tfs_q.get_mut(zombies_e).unwrap();

        let population_scale =
            (state.population as f32 / CELL_MAX_POPULATION as f32).min(1.0) * CELL_HALF_SIZE / 2.0;

        let scale = Vec3::new(population_scale, population_scale, 1.0);

        match state.status {
            Status::Empty => {
                humans_tf.scale = Vec3::ZERO;
                zombies_tf.scale = Vec3::ZERO;
            }
            Status::Zombie => {
                humans_tf.scale = Vec3::ZERO;
                zombies_tf.scale = (scale * Vec3::new(25.0, 25.0, 1.0)).min(Vec3::new(
                    CELL_HALF_SIZE,
                    CELL_HALF_SIZE,
                    1.0,
                ));
            }
            Status::Human => {
                humans_tf.scale = scale;
                zombies_tf.scale = Vec3::ZERO;
            }
        }
    }
}

fn state_debug(
    cells_q: Query<(&ZombieState, &Children)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut commands: Commands,
) {
    for (state, children) in cells_q.iter() {
        let terrain_e = children.get(0).unwrap();

        commands
            .entity(*terrain_e)
            .insert(MeshMaterial2d(materials.add(Color::srgba(
                1.0,
                0.0,
                0.0,
                state.smell_zombie as f32 / 1000.0,
            ))));
    }
}
