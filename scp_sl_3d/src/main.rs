use bevy::input::mouse::MouseMotion;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Константы
// ---------------------------------------------------------------------------
const TS: f32 = 2.0; // размер тайла в метрах
const MW: i32 = 40;
const MH: i32 = 28;
const WALL_H: f32 = 3.2;
const PLAYER_R: f32 = 0.42;
const PLAY_SPEED: f32 = 3.4;
const SPRINT_MULT: f32 = 1.55;
const STATUE_SPEED: f32 = 3.9;

const COL_BG: Color = Color::srgb(0.02, 0.025, 0.035);
const COL_TEXT: Color = Color::srgb(0.85, 0.9, 0.94);
const COL_MUTED: Color = Color::srgb(0.5, 0.56, 0.62);
const COL_RED: Color = Color::srgb(0.9, 0.25, 0.2);

fn glow(col: Color, k: f32) -> LinearRgba {
    let c = LinearRgba::from(col);
    LinearRgba::rgb(c.red * k, c.green * k, c.blue * k)
}

// ---------------------------------------------------------------------------
// Состояние
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Menu,
    Playing,
    Paused,
    Dead,
    Won,
}

#[derive(Resource)]
struct Run {
    phase: Phase,
    yaw: f32,
    pitch: f32,
    stamina: f32,
    battery: f32,
    flash_on: bool,
    cards: [bool; 4],
    blink_t: f32,
    blink_len: f32,
    blinking: f32,
    run_time: f32,
    toast: String,
    toast_t: f32,
    death_t: f32,
    win_t: f32,
    threat: f32,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            phase: Phase::Menu,
            yaw: 0.0,
            pitch: 0.0,
            stamina: 100.0,
            battery: 100.0,
            flash_on: true,
            cards: [false; 4],
            blink_t: 6.5,
            blink_len: 6.5,
            blinking: 0.0,
            run_time: 0.0,
            toast: String::new(),
            toast_t: 0.0,
            death_t: 0.0,
            win_t: 0.0,
            threat: 0.0,
        }
    }
}

#[derive(Clone)]
struct Room {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    name: &'static str,
}

#[derive(Resource)]
struct MapData {
    wall: Vec<bool>,
    rooms: Vec<Room>,
    spawn: Vec3,
    statue_spawn: Vec3,
    card_spots: Vec<(f32, f32, u8)>,
    exit_center: Vec3,
    exit_half: Vec2,
}

#[derive(Clone)]
struct DoorInfo {
    tx: i32,
    ty: i32,
    need: u8,
    open: bool,
    anim: f32,
    orient: u8,
    entity: Entity,
    msg_cd: f32,
}

#[derive(Resource)]
struct DoorSet(Vec<DoorInfo>);

// ---------------------------------------------------------------------------
// Компоненты
// ---------------------------------------------------------------------------
#[derive(Component)]
struct Player;

#[derive(Component)]
struct PlayerCam;

#[derive(Component)]
struct Flashlight;

#[derive(Component)]
struct Statue;

#[derive(Component)]
struct StatueAI {
    path: Vec<(i32, i32)>,
    timer: f32,
    active: bool,
    frozen: bool,
}

#[derive(Component)]
struct Card {
    level: u8,
}

#[derive(Component)]
struct ExitPad;

#[derive(Component)]
struct Flicker {
    base: f32,
    phase: f32,
}

#[derive(Component, Clone, Copy, PartialEq)]
enum HudText {
    Objective,
    Timer,
    Sector,
    Toast,
    Warning,
    Card(u8),
}

#[derive(Component, Clone, Copy, PartialEq)]
enum UiNode {
    Hud,
    Menu,
    Pause,
    Dead,
    Won,
    Blink,
    Danger,
    Stamina,
    Battery,
    BlinkBar,
}

// ---------------------------------------------------------------------------
// Карта
// ---------------------------------------------------------------------------
fn tile_idx(tx: i32, ty: i32) -> usize {
    (ty * MW + tx) as usize
}

fn carve(wall: &mut [bool], x0: i32, y0: i32, x1: i32, y1: i32) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            if x >= 0 && y >= 0 && x < MW && y < MH {
                wall[tile_idx(x, y)] = false;
            }
        }
    }
}

fn world(tx: f32, ty: f32) -> Vec3 {
    Vec3::new(tx * TS, 0.0, ty * TS)
}

fn make_map() -> MapData {
    let mut wall = vec![true; (MW * MH) as usize];
    let rooms = vec![
        Room { x: 2, y: 2, w: 9, h: 7, name: "ЗОНА СОДЕРЖАНИЯ" },
        Room { x: 14, y: 2, w: 10, h: 6, name: "ОФИСЫ" },
        Room { x: 28, y: 2, w: 9, h: 8, name: "ЛАБОРАТОРИЯ" },
        Room { x: 2, y: 13, w: 10, h: 7, name: "КПП / ГАРАЖ" },
        Room { x: 15, y: 11, w: 8, h: 7, name: "СТОЛОВАЯ" },
        Room { x: 28, y: 14, w: 9, h: 9, name: "РЕАКТОР" },
        Room { x: 15, y: 21, w: 8, h: 5, name: "ШЛЮЗ" },
    ];
    for r in &rooms {
        carve(&mut wall, r.x, r.y, r.x + r.w - 1, r.y + r.h - 1);
    }
    // коридоры
    carve(&mut wall, 10, 5, 14, 5);
    carve(&mut wall, 23, 5, 28, 5);
    carve(&mut wall, 6, 8, 6, 13);
    carve(&mut wall, 11, 16, 15, 16);
    carve(&mut wall, 18, 7, 18, 11);
    carve(&mut wall, 32, 9, 32, 14);
    carve(&mut wall, 22, 14, 28, 14);
    carve(&mut wall, 18, 17, 18, 21);

    MapData {
        wall,
        rooms,
        spawn: world(6.5, 16.5),
        statue_spawn: world(6.5, 4.3),
        card_spots: vec![
            (9.4 * TS, 3.4 * TS, 1),
            (21.5 * TS, 3.4 * TS, 2),
            (34.4 * TS, 7.3 * TS, 3),
        ],
        exit_center: world(19.0, 23.0),
        exit_half: Vec2::new(4.0 * TS * 0.5, 2.0 * TS * 0.5),
    }
}

fn tile_of(p: Vec3) -> (i32, i32) {
    ((p.x / TS).floor() as i32, (p.z / TS).floor() as i32)
}

fn in_bounds(tx: i32, ty: i32) -> bool {
    tx >= 0 && ty >= 0 && tx < MW && ty < MH
}

fn walkable(map: &MapData, doors: &DoorSet, tx: i32, ty: i32) -> bool {
    if !in_bounds(tx, ty) || map.wall[tile_idx(tx, ty)] {
        return false;
    }
    for d in &doors.0 {
        if d.tx == tx && d.ty == ty && !d.open {
            return false;
        }
    }
    true
}

fn solid(map: &MapData, doors: &DoorSet, tx: i32, ty: i32) -> bool {
    !walkable(map, doors, tx, ty)
}

fn circle_solid(map: &MapData, doors: &DoorSet, x: f32, z: f32, r: f32) -> bool {
    let x0 = ((x - r) / TS).floor() as i32;
    let x1 = ((x + r) / TS).floor() as i32;
    let y0 = ((z - r) / TS).floor() as i32;
    let y1 = ((z + r) / TS).floor() as i32;
    for ty in y0..=y1 {
        for tx in x0..=x1 {
            if !solid(map, doors, tx, ty) {
                continue;
            }
            let minx = tx as f32 * TS;
            let minz = ty as f32 * TS;
            let nx = x.clamp(minx, minx + TS);
            let nz = z.clamp(minz, minz + TS);
            if (x - nx) * (x - nx) + (z - nz) * (z - nz) < r * r {
                return true;
            }
        }
    }
    false
}

fn bfs(map: &MapData, doors: &DoorSet, start: (i32, i32), goal: (i32, i32)) -> Vec<(i32, i32)> {
    if start == goal {
        return vec![goal];
    }
    if !walkable(map, doors, goal.0, goal.1) || !in_bounds(start.0, start.1) {
        return vec![];
    }
    let mut prev: Vec<Option<(i32, i32)>> = vec![None; (MW * MH) as usize];
    let mut seen = vec![false; (MW * MH) as usize];
    let mut q = VecDeque::new();
    q.push_back(start);
    seen[tile_idx(start.0, start.1)] = true;
    while let Some(cur) = q.pop_front() {
        if cur == goal {
            let mut path = vec![];
            let mut c = Some(cur);
            while let Some(p) = c {
                path.push(p);
                if p == start {
                    break;
                }
                c = prev[tile_idx(p.0, p.1)];
            }
            path.reverse();
            return path;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (cur.0 + dx, cur.1 + dy);
            if !in_bounds(n.0, n.1) || seen[tile_idx(n.0, n.1)] {
                continue;
            }
            if !walkable(map, doors, n.0, n.1) {
                continue;
            }
            seen[tile_idx(n.0, n.1)] = true;
            prev[tile_idx(n.0, n.1)] = Some(cur);
            q.push_back(n);
        }
    }
    vec![]
}

fn los(map: &MapData, doors: &DoorSet, a: Vec3, b: Vec3) -> bool {
    let d = Vec2::new(b.x - a.x, b.z - a.z);
    let len = d.length();
    if len < 0.01 {
        return true;
    }
    let steps = (len / 0.25).ceil() as i32;
    for i in 1..steps {
        let t = i as f32 / steps as f32;
        let x = a.x + d.x * t;
        let z = a.z + d.y * t;
        let (tx, ty) = tile_of(Vec3::new(x, 0.0, z));
        if solid(map, doors, tx, ty) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Стартовая сцена
// ---------------------------------------------------------------------------
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let map = make_map();
    let spawn = map.spawn;
    let statue_spawn = map.statue_spawn;
    let card_spots = map.card_spots.clone();
    let exit_center = map.exit_center;
    let exit_half = map.exit_half;
    let rooms = map.rooms.clone();
    let wall_grid = map.wall.clone();

    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.55, 0.62, 0.75),
        brightness: 30.0,
        ..default()
    });

    // ------------------ игрок и камера ------------------
    commands
        .spawn((Player, Transform::from_translation(spawn), Visibility::default()))
        .with_children(|p| {
            p.spawn((
                PlayerCam,
                Camera3d::default(),
                Projection::Perspective(PerspectiveProjection {
                    fov: 78f32.to_radians(),
                    ..default()
                }),
                Transform::from_xyz(0.0, 1.65, 0.0),
                DistanceFog {
                    color: Color::srgba(0.02, 0.03, 0.045, 1.0),
                    falloff: FogFalloff::Linear { start: 9.0, end: 60.0 },
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn((
                    Flashlight,
                    Transform::from_xyz(0.18, -0.18, 0.0),
                    SpotLight {
                        color: Color::srgb(1.0, 0.93, 0.8),
                        intensity: 3_000_000.0,
                        range: 45.0,
                        outer_angle: 0.50,
                        inner_angle: 0.25,
                        shadows_enabled: true,
                        ..default()
                    },
                ));
            });
        });

    // ------------------ материалы ------------------
    let mat_wall = materials.add(StandardMaterial {
        base_color: Color::srgb(0.19, 0.22, 0.26),
        perceptual_roughness: 0.92,
        ..default()
    });
    let mat_floor = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.13, 0.16),
        perceptual_roughness: 0.85,
        ..default()
    });
    let mat_ceiling = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.06, 0.08),
        ..default()
    });
    let mat_concrete = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.61, 0.56),
        perceptual_roughness: 0.95,
        ..default()
    });
    let mat_eye = materials.add(StandardMaterial {
        base_color: COL_RED,
        emissive: LinearRgba::rgb(6.0, 0.35, 0.2),
        ..default()
    });
    let mat_exit = materials.add(StandardMaterial {
        base_color: Color::srgba(0.25, 1.0, 0.35, 0.65),
        emissive: LinearRgba::rgb(0.1, 1.2, 0.25),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    // ------------------ пол и потолок ------------------
    let floor_mesh = meshes.add(Cuboid::new(MW as f32 * TS, 0.2, MH as f32 * TS));
    commands.spawn((
        Mesh3d(floor_mesh.clone()),
        MeshMaterial3d(mat_floor.clone()),
        Transform::from_xyz(MW as f32 * TS * 0.5, -0.1, MH as f32 * TS * 0.5),
    ));
    commands.spawn((
        Mesh3d(floor_mesh.clone()),
        MeshMaterial3d(mat_ceiling.clone()),
        Transform::from_xyz(MW as f32 * TS * 0.5, WALL_H + 0.1, MH as f32 * TS * 0.5),
    ));

    // ------------------ стены ------------------
    let wall_mesh = meshes.add(Cuboid::new(TS, WALL_H, TS));
    for ty in 0..MH {
        for tx in 0..MW {
            if !wall_grid[tile_idx(tx, ty)] {
                continue;
            }
            let mut near_floor = false;
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let nx = tx + dx;
                let ny = ty + dy;
                if in_bounds(nx, ny) && !wall_grid[tile_idx(nx, ny)] {
                    near_floor = true;
                }
            }
            if !near_floor {
                continue;
            }
            commands.spawn((
                Mesh3d(wall_mesh.clone()),
                MeshMaterial3d(mat_wall.clone()),
                Transform::from_xyz(
                    tx as f32 * TS + TS * 0.5,
                    WALL_H * 0.5,
                    ty as f32 * TS + TS * 0.5,
                ),
            ));
        }
    }

    // ------------------ свет в комнатах ------------------
    for (i, r) in rooms.iter().enumerate() {
        let cx = (r.x as f32 + r.w as f32 * 0.5) * TS;
        let cz = (r.y as f32 + r.h as f32 * 0.5) * TS;
        let base = 1_200_000.0;
        let xs: [f32; 2] = if r.w >= 8 { [-0.25, 0.25] } else { [0.0, 0.0] };
        let n = if r.w >= 8 { 2 } else { 1 };
        for k in 0..n {
            commands.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.88, 0.72),
                    intensity: base,
                    range: 22.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(cx + xs[k] * r.w as f32 * TS, 2.75, cz),
                Flicker {
                    base,
                    phase: i as f32 * 1.7 + k as f32 * 0.9,
                },
            ));
        }
        // лампа на потолке
        let lamp = meshes.add(Cuboid::new(1.2, 0.06, 0.35));
        let lamp_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.95, 0.85),
            emissive: LinearRgba::rgb(4.0, 3.6, 2.8),
            ..default()
        });
        commands.spawn((
            Mesh3d(lamp),
            MeshMaterial3d(lamp_mat),
            Transform::from_xyz(cx, 3.06, cz),
        ));
    }

    // красные аварийные лампы в коридорах
    for (cx, cz, ph) in [(13.0, 21.0, 0.0), (24.0, 11.0, 1.0), (51.0, 11.0, 2.0), (64.0, 24.0, 3.0), (36.0, 38.0, 4.0)] {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.12, 0.08),
                intensity: 45_000.0,
                range: 9.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(cx, 2.9, cz),
            Flicker { base: 45_000.0, phase: ph },
        ));
    }

    // ------------------ двери ------------------
    let door_defs = [(12, 5, 1u8, 0u8), (25, 5, 2, 0), (32, 11, 2, 1), (18, 19, 3, 1)];
    let mut door_infos = Vec::new();
    for (tx, ty, need, orient) in door_defs {
        let col = match need {
            1 => Color::srgb(0.35, 0.7, 1.0),
            2 => Color::srgb(1.0, 0.78, 0.3),
            _ => Color::srgb(1.0, 0.28, 0.24),
        };
        let mat = materials.add(StandardMaterial {
            base_color: col,
            emissive: glow(col, 0.6),
            perceptual_roughness: 0.6,
            metallic: 0.4,
            ..default()
        });
        let mesh = if orient == 0 {
            meshes.add(Cuboid::new(0.18, WALL_H, TS * 0.98))
        } else {
            meshes.add(Cuboid::new(TS * 0.98, WALL_H, 0.18))
        };
        let e = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                Transform::from_xyz(
                    tx as f32 * TS + TS * 0.5,
                    WALL_H * 0.5,
                    ty as f32 * TS + TS * 0.5,
                ),
            ))
            .id();
        door_infos.push(DoorInfo {
            tx,
            ty,
            need,
            open: false,
            anim: 0.0,
            orient,
            entity: e,
            msg_cd: 0.0,
        });
    }
    commands.insert_resource(DoorSet(door_infos));

    // ------------------ карты доступа ------------------
    let card_mesh = meshes.add(Cuboid::new(0.36, 0.5, 0.05));
    for (x, z, lvl) in card_spots {
        let col = match lvl {
            1 => Color::srgb(0.35, 0.7, 1.0),
            2 => Color::srgb(1.0, 0.78, 0.3),
            _ => Color::srgb(1.0, 0.28, 0.24),
        };
        let mat = materials.add(StandardMaterial {
            base_color: col,
            emissive: glow(col, 2.5),
            ..default()
        });
        commands.spawn((
            Card { level: lvl },
            Mesh3d(card_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_xyz(x, 1.15, z),
        ));
        commands.spawn((
            PointLight {
                color: col,
                intensity: 6_000.0,
                range: 3.5,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, 1.3, z),
        ));
    }

    // ------------------ выход ------------------
    let exit_mesh = meshes.add(Cuboid::new(exit_half.x * 2.0, 0.06, exit_half.y * 2.0));
    commands.spawn((
        ExitPad,
        Mesh3d(exit_mesh),
        MeshMaterial3d(mat_exit),
        Transform::from_xyz(exit_center.x, 0.03, exit_center.z),
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(0.3, 1.0, 0.4),
            intensity: 30_000.0,
            range: 10.0,
            ..default()
        },
        Transform::from_xyz(exit_center.x, 2.4, exit_center.z),
    ));

    // ------------------ SCP-173 ------------------
    let capsule = meshes.add(Capsule3d::new(0.42, 1.15));
    let head = meshes.add(Sphere::new(0.31));
    let arm = meshes.add(Capsule3d::new(0.1, 0.95));
    let eye = meshes.add(Sphere::new(0.055));
    commands
        .spawn((
            Statue,
            StatueAI {
                path: vec![],
                timer: 0.0,
                active: false,
                frozen: true,
            },
            Transform::from_translation(statue_spawn),
            Visibility::default(),
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(capsule),
                MeshMaterial3d(mat_concrete.clone()),
                Transform::from_xyz(0.0, 1.3, 0.0),
            ));
            p.spawn((
                Mesh3d(head),
                MeshMaterial3d(mat_concrete.clone()),
                Transform::from_xyz(0.0, 2.25, 0.06),
            ));
            for side in [-1.0f32, 1.0] {
                p.spawn((
                    Mesh3d(arm.clone()),
                    MeshMaterial3d(mat_concrete.clone()),
                    Transform::from_xyz(side * 0.5, 1.25, 0.0)
                        .with_rotation(Quat::from_rotation_z(side * 0.12)),
                ));
                p.spawn((
                    Mesh3d(eye.clone()),
                    MeshMaterial3d(mat_eye.clone()),
                    Transform::from_xyz(side * 0.11, 2.28, 0.30),
                ));
            }
        });

    // ------------------ UI ------------------
    let font = asset_server.load("fonts/ui.ttf");
    spawn_ui(&mut commands, &font);

    commands.insert_resource(map);
    let mut run = Run::default();
    if std::env::var("SCP_AUTO").is_ok() {
        run.phase = Phase::Playing;
    }
    commands.insert_resource(run);
}

fn text_style(font: &Handle<Font>, size: f32, color: Color) -> (TextFont, TextColor) {
    (
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

fn spawn_ui(commands: &mut Commands, font: &Handle<Font>) {
    // ------------------ HUD ------------------
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(10),
            UiNode::Hud,
        ))
        .with_children(|root| {
            // задача
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0),
                    top: Val::Px(16.0),
                    width: Val::Px(430.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.05, 0.07, 0.82)),
                BorderRadius::all(Val::Px(4.0)),
            ))
            .with_children(|p| {
                let (f, c) = text_style(font, 11.0, COL_MUTED);
                p.spawn((Text::new("ЗАДАЧА"), f, c));
                let (f, c) = text_style(font, 17.0, COL_TEXT);
                p.spawn((Text::new("Найти карту уровня 1"), f, c, HudText::Objective));
            });

            // таймер
            let (f, c) = text_style(font, 26.0, COL_TEXT);
            root.spawn((
                Text::new("00:00"),
                f,
                c,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(24.0),
                    top: Val::Px(16.0),
                    ..default()
                },
                HudText::Timer,
            ));

            // сектор
            let (f, c) = text_style(font, 14.0, COL_TEXT);
            root.spawn((
                Text::new("КПП / ГАРАЖ"),
                f,
                c,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(22.0),
                    bottom: Val::Px(78.0),
                    ..default()
                },
                HudText::Sector,
            ));

            // карты
            for lvl in 1..=3u8 {
                let (f, c) = text_style(font, 20.0, Color::srgb(0.25, 0.3, 0.36));
                root.spawn((
                    Text::new(format!("[{}]", lvl)),
                    f,
                    c,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(22.0 + lvl as f32 * 44.0),
                        bottom: Val::Px(36.0),
                        ..default()
                    },
                    HudText::Card(lvl),
                ));
            }
            let (f, c) = text_style(font, 11.0, COL_MUTED);
            root.spawn((
                Text::new("УРОВЕНЬ ДОСТУПА"),
                f,
                c,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(22.0),
                    bottom: Val::Px(40.0),
                    ..default()
                },
            ));

            // полосы
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(24.0),
                    bottom: Val::Px(20.0),
                    width: Val::Px(240.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
            ))
            .with_children(|p| {
                for (label, marker) in [("ВЫНОСЛИВОСТЬ", 0), ("ФОНАРЬ", 1), ("МОРГАНИЕ", 2)] {
                    let (f, c) = text_style(font, 10.0, COL_MUTED);
                    p.spawn((Text::new(label.to_string()), f, c));
                    p.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(9.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.06, 0.08, 0.1)),
                        BorderRadius::all(Val::Px(2.0)),
                    ))
                    .with_children(|bar| {
                        let fill = Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        };
                        let col = match marker {
                            0 => Color::srgb(0.55, 0.75, 0.2),
                            1 => Color::srgb(0.9, 0.72, 0.25),
                            _ => Color::srgb(0.35, 0.65, 0.9),
                        };
                        if marker == 0 {
                            bar.spawn((fill, BackgroundColor(col), UiNode::Stamina));
                        } else if marker == 1 {
                            bar.spawn((fill, BackgroundColor(col), UiNode::Battery));
                        } else {
                            bar.spawn((fill, BackgroundColor(col), UiNode::BlinkBar));
                        }
                    });
                }
            });

            // подсказка
            let (f, c) = text_style(font, 11.0, Color::srgb(0.32, 0.38, 0.44));
            root.spawn((
                Text::new("WASD — движение · Shift — бег · F — фонарь · Esc — пауза"),
                f,
                c,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(22.0),
                    bottom: Val::Px(14.0),
                    ..default()
                },
            ));

            // тост
            let (f, c) = text_style(font, 15.0, COL_TEXT);
            root.spawn((
                Text::new(""),
                f,
                c,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    bottom: Val::Px(140.0),
                    width: Val::Percent(100.0),
                    ..default()
                },
                TextLayout::new_with_justify(JustifyText::Center),
                HudText::Toast,
            ));

            // предупреждение
            let (f, c) = text_style(font, 16.0, COL_RED);
            root.spawn((
                Text::new(""),
                f,
                c,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(70.0),
                    width: Val::Percent(100.0),
                    ..default()
                },
                TextLayout::new_with_justify(JustifyText::Center),
                HudText::Warning,
            ));
        });

    // красная пелена опасности
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.7, 0.02, 0.02, 0.10)),
        GlobalZIndex(11),
        UiNode::Danger,
    ));

    // чёрная вспышка моргания
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.985)),
        GlobalZIndex(15),
        UiNode::Blink,
    ));

    // ------------------ меню ------------------
    spawn_overlay(commands, font, UiNode::Menu, "SCP: SECRET LABORATORY",
                  Color::srgb(0.92, 0.95, 0.98), true);
    spawn_overlay(commands, font, UiNode::Pause, "ПАУЗА", COL_TEXT, false);
    spawn_overlay(commands, font, UiNode::Dead, "ШЕЯ СЛОМАНА", COL_RED, false);
    spawn_overlay(commands, font, UiNode::Won, "ПОБЕГ УСПЕШЕН",
                  Color::srgb(0.55, 0.95, 0.5), false);
}

fn spawn_overlay(
    commands: &mut Commands,
    font: &Handle<Font>,
    marker: impl Component,
    title: &str,
    color: Color,
    menu: bool,
) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                display: Display::None,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.045, 0.76)),
            GlobalZIndex(20),
            marker,
        ))
        .with_children(|p| {
            let (f, c) = text_style(font, 50.0, color);
            p.spawn((Text::new(title.to_string()), f, c));
            if menu {
                let (f, c) = text_style(font, 18.0, COL_MUTED);
                p.spawn((Text::new("3D-прототип · Rust + Bevy"), f, c));
                let (f, c) = text_style(font, 15.0, Color::srgb(0.68, 0.74, 0.8));
                for line in [
                    "Вы — сотрудник класса D. Комплекс потерял контроль.",
                    "Соберите карты доступа 1–3 уровня и доберитесь до шлюза.",
                    "SCP-173 замирает, пока вы на него смотрите. Моргнёте — он ударит.",
                    "",
                    "WASD — движение     Shift — бег     F — фонарь",
                    "Мышь — взгляд       Esc — пауза     R — заново",
                ] {
                    p.spawn((Text::new(line.to_string()), f.clone(), c.clone()));
                }
                let (f, c) = text_style(font, 24.0, COL_TEXT);
                p.spawn((
                    Text::new("ENTER — НАЧАТЬ"),
                    f,
                    c,
                    Node { margin: UiRect::top(Val::Px(18.0)), ..default() },
                ));
                let (f, c) = text_style(font, 11.0, Color::srgb(0.3, 0.34, 0.38));
                p.spawn((Text::new("Фанатский прототип. Не связан с Northwood Studios."), f, c));
            } else {
                let (f, c) = text_style(font, 16.0, COL_MUTED);
                let hint = match title {
                    "ПАУЗА" => "Esc — продолжить · Backspace — в меню",
                    "ШЕЯ СЛОМАНА" => "R — попробовать снова · Esc — меню",
                    _ => "R — ещё раз · Esc — меню",
                };
                p.spawn((Text::new(hint.to_string()), f, c));
            }
        });
}

// ---------------------------------------------------------------------------
// Клавиши, сброс, курсор
// ---------------------------------------------------------------------------
fn reset_world(
    run: &mut Run,
    doors: &mut DoorSet,
    map: &MapData,
    player: &mut Transform,
    cam: &mut Transform,
    statue: &mut Transform,
    ai: &mut StatueAI,
    cards: &mut Query<(&Card, &mut Visibility)>,
) {
    player.translation = map.spawn;
    player.rotation = Quat::IDENTITY;
    cam.translation = Vec3::new(0.0, 1.65, 0.0);
    cam.rotation = Quat::IDENTITY;
    statue.translation = map.statue_spawn;
    statue.rotation = Quat::from_rotation_y(std::f32::consts::PI);
    ai.path.clear();
    ai.timer = 0.0;
    ai.active = false;
    ai.frozen = true;
    for d in doors.0.iter_mut() {
        d.open = false;
        d.anim = 0.0;
        d.msg_cd = 0.0;
    }
    for (_, mut vis) in cards.iter_mut() {
        *vis = Visibility::Visible;
    }
    run.phase = Phase::Playing;
    run.yaw = 0.0;
    run.pitch = 0.0;
    run.stamina = 100.0;
    run.battery = 100.0;
    run.flash_on = true;
    run.cards = [false; 4];
    run.blink_t = 6.5;
    run.blink_len = 6.5;
    run.blinking = 0.0;
    run.run_time = 0.0;
    run.toast.clear();
    run.toast_t = 0.0;
    run.death_t = 0.0;
    run.win_t = 0.0;
    run.threat = 0.0;
}

fn global_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut run: ResMut<Run>,
    mut doors: ResMut<DoorSet>,
    map: Res<MapData>,
    mut player: Query<&mut Transform, With<Player>>,
    mut cam: Query<&mut Transform, (With<PlayerCam>, Without<Player>)>,
    mut statue: Query<(&mut Transform, &mut StatueAI), (With<Statue>, Without<Player>, Without<PlayerCam>)>,
    mut cards: Query<(&Card, &mut Visibility)>,
) {
    if keys.just_pressed(KeyCode::Enter) && run.phase == Phase::Menu {
        if let (Ok(mut p), Ok(mut c), Ok((mut s, mut ai))) = (
            player.get_single_mut(),
            cam.get_single_mut(),
            statue.get_single_mut(),
        ) {
            reset_world(&mut run, &mut doors, &map, &mut p, &mut c, &mut s, &mut ai, &mut cards);
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        run.phase = match run.phase {
            Phase::Playing => Phase::Paused,
            Phase::Paused => Phase::Playing,
            Phase::Dead | Phase::Won => Phase::Menu,
            Phase::Menu => Phase::Menu,
        };
    }
    if keys.just_pressed(KeyCode::Backspace) && run.phase == Phase::Paused {
        run.phase = Phase::Menu;
    }
    if keys.just_pressed(KeyCode::KeyR) && matches!(run.phase, Phase::Dead | Phase::Won) {
        if let (Ok(mut p), Ok(mut c), Ok((mut s, mut ai))) = (
            player.get_single_mut(),
            cam.get_single_mut(),
            statue.get_single_mut(),
        ) {
            reset_world(&mut run, &mut doors, &map, &mut p, &mut c, &mut s, &mut ai, &mut cards);
        }
    }
}

fn cursor_system(
    run: Res<Run>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if let Ok(mut w) = windows.get_single_mut() {
        let want = run.phase == Phase::Playing && std::env::var("SCP_NOLOCK").is_err();
        let mode = if want { CursorGrabMode::Locked } else { CursorGrabMode::None };
        if w.cursor_options.grab_mode != mode {
            w.cursor_options.grab_mode = mode;
            w.cursor_options.visible = !want;
        }
    }
}

fn look_system(
    mut ev: EventReader<MouseMotion>,
    mut run: ResMut<Run>,
    mut player: Query<&mut Transform, With<Player>>,
    mut cam: Query<&mut Transform, (With<PlayerCam>, Without<Player>)>,
) {
    if run.phase != Phase::Playing || std::env::var("SCP_NOLOOK").is_ok() {
        ev.clear();
        return;
    }
    let mut delta = Vec2::ZERO;
    for m in ev.read() {
        delta += m.delta;
    }
    run.yaw -= delta.x * 0.0022;
    run.pitch = (run.pitch - delta.y * 0.0022).clamp(-1.45, 1.45);
    if let Ok(mut t) = player.get_single_mut() {
        t.rotation = Quat::from_rotation_y(run.yaw);
    }
    if let Ok(mut t) = cam.get_single_mut() {
        t.rotation = Quat::from_rotation_x(run.pitch);
    }
}

// ---------------------------------------------------------------------------
// Движение, двери, карты, моргание
// ---------------------------------------------------------------------------
fn game_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut run: ResMut<Run>,
    mut doors: ResMut<DoorSet>,
    map: Res<MapData>,
    mut player: Query<&mut Transform, (With<Player>, Without<Statue>)>,
    mut ai_q: Query<&mut StatueAI>,
    mut cards: Query<(&Card, &Transform, &mut Visibility), (Without<Player>, Without<Statue>)>,
    mut flash: Query<&mut SpotLight, (With<Flashlight>, Without<Card>)>,
) {
    if run.phase != Phase::Playing {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    run.run_time += dt;
    run.toast_t = (run.toast_t - dt).max(0.0);
    for d in doors.0.iter_mut() {
        d.msg_cd = (d.msg_cd - dt).max(0.0);
    }

    let Ok(mut pt) = player.get_single_mut() else {
        return;
    };

    // фонарь
    if keys.just_pressed(KeyCode::KeyF) {
        if run.battery > 0.0 {
            run.flash_on = !run.flash_on;
            run.toast = if run.flash_on { "ФОНАРЬ ВКЛ".into() } else { "ФОНАРЬ ВЫКЛ".into() };
            run.toast_t = 1.6;
        } else {
            run.toast = "БАТАРЕЯ ПУСТА".into();
            run.toast_t = 1.6;
        }
    }
    if let Ok(mut light) = flash.get_single_mut() {
        light.intensity = if run.flash_on && run.battery > 0.0 {
            3_000_000.0
        } else {
            0.0
        };
    }
    if run.flash_on && run.battery > 0.0 {
        run.battery = (run.battery - 2.0 * dt).max(0.0);
    } else if !run.flash_on {
        run.battery = (run.battery + 0.7 * dt).min(100.0);
    }

    // движение
    let mut mv = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) { mv.y += 1.0; }
    if keys.pressed(KeyCode::KeyS) { mv.y -= 1.0; }
    if keys.pressed(KeyCode::KeyD) { mv.x += 1.0; }
    if keys.pressed(KeyCode::KeyA) { mv.x -= 1.0; }
    let moving = mv.length_squared() > 0.0;
    let sprint = keys.pressed(KeyCode::ShiftLeft) && run.stamina > 1.0 && moving;
    let speed = PLAY_SPEED * if sprint { SPRINT_MULT } else { 1.0 };
    if sprint {
        run.stamina = (run.stamina - 24.0 * dt).max(0.0);
    } else {
        run.stamina = (run.stamina + 14.0 * dt).min(100.0);
    }
    if moving {
        mv = mv.normalize();
        let (sy, cy) = run.yaw.sin_cos();
        let forward = Vec2::new(-sy, -cy);
        let right = Vec2::new(cy, -sy);
        let dir = (forward * mv.y + right * mv.x) * speed * dt;
        let pos = pt.translation;
        let nx = pos.x + dir.x;
        if !circle_solid(&map, &doors, nx, pos.z, PLAYER_R) {
            pt.translation.x = nx;
        }
        let nz = pos.z + dir.y;
        if !circle_solid(&map, &doors, pt.translation.x, nz, PLAYER_R) {
            pt.translation.z = nz;
        }
    }

    // моргание
    run.blink_t -= dt;
    if run.blink_t <= 0.0 {
        run.blinking = 0.22;
        run.blink_len = 5.5 + rand_float() * 2.5;
        run.blink_t = run.blink_len;
    }
    run.blinking = (run.blinking - dt).max(0.0);

    // двери
    let pos = pt.translation;
    for i in 0..doors.0.len() {
        let (tx, ty, need, open, msg_cd, orient) = {
            let d = &doors.0[i];
            (d.tx, d.ty, d.need, d.open, d.msg_cd, d.orient)
        };
        if open {
            continue;
        }
        let cx = tx as f32 * TS + TS * 0.5;
        let cz = ty as f32 * TS + TS * 0.5;
        let _ = orient;
        if Vec2::new(cx - pos.x, cz - pos.z).length() < 2.0 {
            let best = (1..=3u8).rev().find(|l| run.cards[*l as usize]).unwrap_or(0);
            if best >= need {
                doors.0[i].open = true;
                run.toast = format!("ДВЕРЬ ОТКРЫТА (уровень {})", need);
                run.toast_t = 2.2;
            } else if msg_cd <= 0.0 {
                doors.0[i].msg_cd = 1.8;
                run.toast = format!("ТРЕБУЕТСЯ КАРТА УРОВНЯ {}", need);
                run.toast_t = 2.0;
            }
        }
    }

    // карты
    for (card, ct, mut vis) in cards.iter_mut() {
        if *vis == Visibility::Hidden {
            continue;
        }
        if ct.translation.distance(Vec3::new(pos.x, 1.15, pos.z)) < 1.35 {
            *vis = Visibility::Hidden;
            run.cards[card.level as usize] = true;
            run.toast = format!("КАРТА УРОВНЯ {} ПОЛУЧЕНА", card.level);
            run.toast_t = 2.4;
            if card.level == 1 {
                if let Ok(mut ai) = ai_q.get_single_mut() {
                    ai.active = true;
                    ai.timer = 0.0;
                }
                run.toast = "ВНИМАНИЕ: SCP-173 ПОКИНУЛ КАМЕРУ".into();
                run.toast_t = 2.8;
            }
        }
    }

    // выход
    let in_exit = (pos.x - map.exit_center.x).abs() <= map.exit_half.x
        && (pos.z - map.exit_center.z).abs() <= map.exit_half.y;
    if in_exit {
        let gate_open = doors.0.iter().any(|d| d.tx == 18 && d.ty == 19 && d.open);
        if gate_open {
            run.phase = Phase::Won;
            run.win_t = 0.0;
        }
    }
}

fn rand_float() -> f32 {
    // простой псевдослучайный генератор без внешних крейтов
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u32> = const { Cell::new(0x9E3779B9) };
    }
    SEED.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        s.set(x);
        (x & 0xffffff) as f32 / 0xffffff as f32
    })
}

// ---------------------------------------------------------------------------
// SCP-173
// ---------------------------------------------------------------------------
fn statue_system(
    time: Res<Time>,
    mut run: ResMut<Run>,
    map: Res<MapData>,
    doors: Res<DoorSet>,
    mut q: Query<(&mut Transform, &mut StatueAI), With<Statue>>,
    player_q: Query<&Transform, (With<Player>, Without<Statue>)>,
    cam_q: Query<&GlobalTransform, With<PlayerCam>>,
) {
    if run.phase != Phase::Playing {
        return;
    }
    let Ok((mut st, mut ai)) = q.get_single_mut() else {
        return;
    };
    let Ok(pt) = player_q.get_single() else {
        return;
    };
    let Ok(cam) = cam_q.get_single() else {
        return;
    };
    let ppos = pt.translation;
    let spos = st.translation;
    let dist = ppos.distance(spos);
    if !ai.active {
        ai.frozen = true;
        return;
    }

    let to = Vec2::new(spos.x - ppos.x, spos.z - ppos.z);
    let mut visible = false;
    if run.blinking <= 0.0 && dist < 32.0 {
        let fwd3 = cam.forward();
        let fwd = Vec2::new(fwd3.x, fwd3.z).normalize_or_zero();
        let dir = to.normalize_or_zero();
        let half: f32 = if run.flash_on && run.battery > 0.0 { 0.45 } else { 0.95 };
        let dots = fwd.dot(dir).clamp(-1.0, 1.0);
        if dots > half.cos() && los(&map, &doors, ppos, spos) {
            visible = true;
        }
    }
    let close_los = run.blinking <= 0.0 && dist < 3.2 && los(&map, &doors, ppos, spos);
    ai.frozen = visible || close_los;

    if dist < 1.45 && !ai.frozen {
        run.phase = Phase::Dead;
        run.death_t = 0.0;
        run.threat = 0.0;
        return;
    }

    // поворот к игроку (модель смотрит в +Z)
    st.rotation = Quat::from_rotation_y((to.x).atan2(to.y));

    if ai.frozen {
        return;
    }

    let dt = time.delta_secs().min(0.05);
    ai.timer -= dt;
    if ai.timer <= 0.0 {
        ai.timer = 0.4;
        let (sx, sy) = tile_of(spos);
        let (gx, gy) = tile_of(ppos);
        ai.path = bfs(&map, &doors, (sx, sy), (gx, gy));
    }
    while !ai.path.is_empty() {
        let (tx, ty) = ai.path[0];
        let target = Vec3::new(tx as f32 * TS + TS * 0.5, 0.0, ty as f32 * TS + TS * 0.5);
        if target.distance(Vec3::new(spos.x, 0.0, spos.z)) < 0.35 {
            ai.path.remove(0);
        } else {
            break;
        }
    }
    let target = if let Some((tx, ty)) = ai.path.first() {
        Vec3::new(*tx as f32 * TS + TS * 0.5, 0.0, *ty as f32 * TS + TS * 0.5)
    } else {
        Vec3::new(ppos.x, 0.0, ppos.z)
    };
    let mut dir = Vec3::new(target.x - spos.x, 0.0, target.z - spos.z);
    if dir.length_squared() > 0.0001 {
        dir = dir.normalize() * STATUE_SPEED * dt;
        let nx = spos.x + dir.x;
        if !circle_solid(&map, &doors, nx, spos.z, 0.5) {
            st.translation.x = nx;
        }
        let nz = spos.z + dir.z;
        if !circle_solid(&map, &doors, st.translation.x, nz, 0.5) {
            st.translation.z = nz;
        }
    }

    let threat_target = if dist < 18.0 { 1.0 } else { 0.25 };
    run.threat += (threat_target - run.threat) * (dt * 4.0).min(1.0);
}

// ---------------------------------------------------------------------------
// Анимации, UI
// ---------------------------------------------------------------------------
fn anim_system(
    time: Res<Time>,
    mut doors: ResMut<DoorSet>,
    mut q: Query<&mut Transform>,
    mut flick: Query<(&Flicker, &mut PointLight)>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    for d in doors.0.iter_mut() {
        if d.open && d.anim < 1.0 {
            d.anim = (d.anim + dt * 2.2).min(1.0);
        }
        if !d.open && d.anim > 0.0 {
            d.anim = (d.anim - dt * 2.2).max(0.0);
        }
        let _ = d.orient;
        if let Ok(mut tr) = q.get_mut(d.entity) {
            tr.translation.y = WALL_H * 0.5 + d.anim * (WALL_H + 0.2);
        }
    }
    for (fl, mut light) in flick.iter_mut() {
        let n = ((t * 7.0 + fl.phase).sin() * 0.5 + 0.5).powf(6.0);
        light.intensity = fl.base * (0.82 + 0.18 * n);
    }
}

fn card_spin_system(
    time: Res<Time>,
    mut q: Query<(&Card, &mut Transform), (Without<Statue>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    for (_, mut tr) in q.iter_mut() {
        tr.rotate_y(dt * 1.4);
        tr.translation.y = 1.15 + (time.elapsed_secs() * 1.5 + tr.translation.x).sin() * 0.08;
    }
}

fn ui_system(
    run: Res<Run>,
    map: Res<MapData>,
    mut texts: Query<(&HudText, &mut Text, &mut TextColor)>,
    mut nodes: Query<(&UiNode, &mut Node)>,
    player_q: Query<&Transform, With<Player>>,
) {
    let owned = run.cards;
    let obj_text = if !owned[1] {
        "Найти карту уровня 1 в Зоне содержания"
    } else if !owned[2] {
        "Найти карту уровня 2 в Офисах"
    } else if !owned[3] {
        "Найти карту уровня 3 в Лаборатории"
    } else {
        "Добраться до шлюза и покинуть комплекс"
    };
    let secs = run.run_time as u32;
    let time_text = format!("{:02}:{:02}", secs / 60, secs % 60);
    let toast_text = if run.toast_t > 0.0 { run.toast.clone() } else { String::new() };
    let warn_text = if run.phase == Phase::Playing && run.threat > 0.55 {
        "SCP-173 ВНЕ ПОЛЯ ЗРЕНИЯ"
    } else {
        ""
    };
    let mut sector_name = "КОРИДОР";
    if let Ok(pt) = player_q.get_single() {
        let (tx, ty) = tile_of(pt.translation);
        for r in &map.rooms {
            if tx >= r.x && tx < r.x + r.w && ty >= r.y && ty < r.y + r.h {
                sector_name = r.name;
            }
        }
    }

    for (kind, mut text, mut color) in texts.iter_mut() {
        match *kind {
            HudText::Objective => {
                if text.0 != obj_text {
                    text.0 = obj_text.to_string();
                }
            }
            HudText::Timer => {
                if text.0 != time_text {
                    text.0 = time_text.clone();
                }
            }
            HudText::Sector => {
                if text.0 != sector_name {
                    text.0 = sector_name.to_string();
                }
            }
            HudText::Toast => {
                if text.0 != toast_text {
                    text.0 = toast_text.clone();
                }
            }
            HudText::Warning => {
                if text.0 != warn_text {
                    text.0 = warn_text.to_string();
                }
            }
            HudText::Card(lvl) => {
                let col = if owned[lvl as usize] {
                    match lvl {
                        1 => Color::srgb(0.35, 0.7, 1.0),
                        2 => Color::srgb(1.0, 0.78, 0.3),
                        _ => Color::srgb(1.0, 0.28, 0.24),
                    }
                } else {
                    Color::srgb(0.25, 0.3, 0.36)
                };
                color.0 = col;
                let s = if owned[lvl as usize] { format!("[{}]", lvl) } else { "[ ]".to_string() };
                if text.0 != s {
                    text.0 = s;
                }
            }
        }
    }

    let blink_pct = (1.0 - run.blink_t / run.blink_len.max(0.01)) * 100.0;
    for (kind, mut node) in nodes.iter_mut() {
        match *kind {
            UiNode::Stamina => node.width = Val::Percent(run.stamina),
            UiNode::Battery => node.width = Val::Percent(run.battery),
            UiNode::BlinkBar => node.width = Val::Percent(blink_pct.clamp(0.0, 100.0)),
            UiNode::Blink => {
                node.display = if run.blinking > 0.0 { Display::Flex } else { Display::None };
            }
            UiNode::Hud => {
                node.display = if run.phase == Phase::Playing { Display::Flex } else { Display::None };
            }
            UiNode::Menu => {
                node.display = if run.phase == Phase::Menu { Display::Flex } else { Display::None };
            }
            UiNode::Pause => {
                node.display = if run.phase == Phase::Paused { Display::Flex } else { Display::None };
            }
            UiNode::Dead => {
                node.display = if run.phase == Phase::Dead { Display::Flex } else { Display::None };
            }
            UiNode::Won => {
                node.display = if run.phase == Phase::Won { Display::Flex } else { Display::None };
            }
            UiNode::Danger => {
                let visible = run.phase == Phase::Playing && run.threat > 0.2;
                node.display = if visible { Display::Flex } else { Display::None };
            }
        }
    }
}


// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
fn main() {
    // ассеты ищем рядом с exe (портативная сборка) либо в папке проекта (cargo run)
    if std::env::var("BEVY_ASSET_ROOT").is_err() {
        let mut root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
            if dir.join("assets/fonts/ui.ttf").exists() {
                root = dir;
            }
        }
        std::env::set_var("BEVY_ASSET_ROOT", root);
    }
    App::new()
        .insert_resource(ClearColor(COL_BG))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "SCP: Secret Laboratory — 3D (Rust + Bevy)".into(),
                resolution: (1040.0_f32, 585.0_f32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                global_keys,
                cursor_system,
                look_system,
                game_system,
                statue_system,
                anim_system,
                card_spin_system,
                ui_system,
            )
                .chain(),
        )
        .run();
}