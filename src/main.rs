use assets_manager::{asset::Png, AssetCache};
use frenderer::{
    input::{Input, Key},
    sprites::{Camera2D, SheetRegion, Transform},
    wgpu, Immediate,
};
use rand::Rng;
mod geom;
mod grid;
use geom::*;

#[derive(Clone, Debug, PartialEq, Eq)]
enum EntityType {
    Player,
    Enemy,
    // which level, grid x in dest level, grid y in dest level
    #[allow(dead_code)]
    Door(String, u16, u16),
}

#[derive(Clone, Copy, Debug)]
struct TileData {
    solid: bool,
    sheet_region: SheetRegion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Dir {
    N,
    E,
    S,
    W,
}
const PLAYER: [SheetRegion; 4] = [
    //n, e, s, w
    SheetRegion::rect(461 + 16 * 2, 39, 16, 16),
    SheetRegion::rect(461, 39, 16, 16),
    SheetRegion::rect(461 + 16 * 3, 39, 16, 16),
    SheetRegion::rect(461 + 16, 39, 16, 16),
];
const PLAYER_ATK: [SheetRegion; 4] = [
    //n, e, s, w
    SheetRegion::rect(428, 0, 16, 8), // offset by 8px in direction
    SheetRegion::rect(349, 22, 8, 16),
    SheetRegion::rect(162, 13, 16, 8),
    SheetRegion::rect(549, 17, 8, 16),
];
const ENEMY: [SheetRegion; 4] = [
    SheetRegion::rect(533 + 16 * 2, 39, 16, 16),
    SheetRegion::rect(533 + 16, 39, 16, 16),
    SheetRegion::rect(533, 39, 16, 16),
    SheetRegion::rect(533 + 16 * 3, 39, 16, 16),
];

const HEART: SheetRegion = SheetRegion::rect(525, 35, 8, 8);

impl Dir {
    fn to_vec2(self) -> Vec2 {
        match self {
            Dir::N => Vec2 { x: 0.0, y: 1.0 },
            Dir::E => Vec2 { x: 1.0, y: 0.0 },
            Dir::S => Vec2 { x: 0.0, y: -1.0 },
            Dir::W => Vec2 { x: -1.0, y: 0.0 },
        }
    }
}
#[derive(Clone, Debug)]
struct Entity {
    pos: Vec2,
    dir: Dir,
    etype: EntityType,
}
impl Entity {
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.pos.x - TILE_SZ as f32 / 2.0 + 2.0,
            y: self.pos.y - TILE_SZ as f32 / 2.0 + 2.0,
            w: TILE_SZ as u16 - 4,
            h: TILE_SZ as u16 - 4,
        }
    }
    pub fn transform(&self) -> Transform {
        Transform {
            x: self.pos.x,
            y: self.pos.y,
            w: TILE_SZ as u16,
            h: TILE_SZ as u16,
            rot: 0.0,
        }
    }
    pub fn uv(&self) -> SheetRegion {
        match self.etype {
            EntityType::Player => PLAYER[self.dir as usize],
            EntityType::Enemy => ENEMY[self.dir as usize],
            _ => panic!("can't draw doors"),
        }
        .with_depth(1)
    }
}
mod level;
use level::Level;
struct Game {
    assets: AssetCache,
    current_level: usize,
    levels: Vec<Level>,
    entities: Vec<Entity>,
    attack_area: Rect,
    attack_timer: f32,
    knockback_timer: f32,
    health: u8,
}

// Feel free to change this if you use a different tilesheet
const TILE_SZ: usize = 16;
const W: usize = 320;
const H: usize = 240;

// pixels per second
const PLAYER_SPEED: f32 = 64.0;
const ENEMY_SPEED: f32 = 32.0;
const KNOCKBACK_SPEED: f32 = 128.0;

const ATTACK_MAX_TIME: f32 = 0.3;
const ATTACK_COOLDOWN_TIME: f32 = 0.1;
const KNOCKBACK_TIME: f32 = 0.25;

const DT: f32 = 1.0 / 60.0;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    let source =
        assets_manager::source::FileSystem::new("content").expect("Couldn't load resources");
    #[cfg(target_arch = "wasm32")]
    let source = assets_manager::source::Embedded::from(assets_manager::source::embed!("content"));
    let cache = assets_manager::AssetCache::with_source(source);

    let drv = frenderer::Driver::new(
        winit::window::WindowBuilder::new()
            .with_title("test")
            .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 768.0)),
        Some((W as u32, H as u32)),
    );

    let mut input = Input::default();

    let mut now = frenderer::clock::Instant::now();
    let mut acc = 0.0;
    drv.run_event_loop::<(), _>(
        move |window, frend| {
            let mut frend = Immediate::new(frend);
            let game = Game::new(&mut frend, cache);
            (window, game, frend)
        },
        move |event, target, (window, ref mut game, ref mut frend)| {
            use winit::event::{Event, WindowEvent};
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    target.exit();
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    if !frend.gpu().is_web() {
                        frend.resize_surface(size.width, size.height);
                    }
                    window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    let elapsed = now.elapsed().as_secs_f32();
                    // You can add the time snapping/death spiral prevention stuff here if you want.
                    // I'm not using it here to keep the starter code small.
                    acc += elapsed;
                    now = std::time::Instant::now();
                    // While we have time to spend
                    while acc >= DT {
                        // simulate a frame
                        acc -= DT;
                        game.simulate(&input, DT);
                        input.next_frame();
                    }
                    game.render(frend);
                    frend.render();
                    window.request_redraw();
                }
                event => {
                    input.process_input_event(&event);
                }
            }
        },
    )
    .expect("event loop error");
}

impl Game {
    fn new(renderer: &mut Immediate, cache: AssetCache) -> Self {
        let tile_handle = cache
            .load::<Png>("texture")
            .expect("Couldn't load tilesheet img");
        let tile_img = tile_handle.read().0.to_rgba8();
        let tile_tex = renderer.create_array_texture(
            &[&tile_img],
            wgpu::TextureFormat::Rgba8UnormSrgb,
            tile_img.dimensions(),
            Some("tiles-sprites"),
        );
        let levels = vec![Level::from_str(
            &cache
                .load::<String>("level1")
                .expect("Couldn't access level1.txt")
                .read(),
        )];
        let current_level = 0;
        let camera = Camera2D {
            screen_pos: [0.0, 0.0],
            screen_size: [W as f32, H as f32],
        };
        let sprite_estimate =
            levels[current_level].sprite_count() + levels[current_level].starts().len();
        renderer.sprite_group_add(
            &tile_tex,
            vec![Transform::ZERO; sprite_estimate],
            vec![SheetRegion::ZERO; sprite_estimate],
            camera,
        );
        let player_start = *levels[current_level]
            .starts()
            .iter()
            .find(|(t, _)| *t == EntityType::Player)
            .map(|(_, ploc)| ploc)
            .expect("Start level doesn't put the player anywhere");
        let mut game = Game {
            assets: cache,
            current_level,
            attack_area: Rect {
                x: 0.0,
                y: 0.0,
                w: 0,
                h: 0,
            },
            knockback_timer: 0.0,
            attack_timer: 0.0,
            levels,
            health: 3,
            entities: vec![Entity {
                etype: EntityType::Player,
                pos: player_start,
                dir: Dir::S,
            }],
        };
        game.enter_level(player_start);
        game
    }
    fn level(&self) -> &Level {
        &self.levels[self.current_level]
    }
    fn enter_level(&mut self, player_pos: Vec2) {
        self.entities.truncate(1);
        self.entities[0].pos = player_pos;
        for (etype, pos) in self.levels[self.current_level].starts().iter() {
            match etype {
                EntityType::Player => {}
                EntityType::Door(_rm, _x, _y) => todo!("doors not supported"),
                EntityType::Enemy => self.entities.push(Entity {
                    pos: *pos,
                    dir: Dir::S,
                    etype: etype.clone(),
                }),
            }
        }
    }
    fn render(&mut self, frend: &mut Immediate) {
        self.level().render_immediate(frend);
        for entity in self.entities.iter() {
            frend.draw_sprite(0, entity.transform(), entity.uv());
        }
        if !self.attack_area.is_empty() {
            let (w, h) = match self.entities[0].dir {
                Dir::N | Dir::S => (16, 8),
                _ => (8, 16),
            };
            let delta = self.entities[0].dir.to_vec2() * 7.0;
            let pos = self.entities[0].pos + delta;
            frend.draw_sprite(
                0,
                Transform {
                    w,
                    h,
                    x: pos.x,
                    y: pos.y,
                    rot: 0.0,
                },
                PLAYER_ATK[self.entities[0].dir as usize].with_depth(0),
            );
        }
        // TODO POINT: draw hearts
    }
    fn simulate(&mut self, input: &Input, dt: f32) {
        if self.attack_timer > 0.0 {
            self.attack_timer -= dt;
        }
        if self.knockback_timer > 0.0 {
            self.knockback_timer -= dt;
        }
        let mut dx = input.key_axis(Key::ArrowLeft, Key::ArrowRight) * PLAYER_SPEED * DT;
        // now down means -y and up means +y!  beware!
        let mut dy = input.key_axis(Key::ArrowDown, Key::ArrowUp) * PLAYER_SPEED * DT;
        let attacking = !self.attack_area.is_empty();
        let knockback = self.knockback_timer > 0.0;
        if attacking {
            dx = 0.0;
            dy = 0.0;
        } else if knockback {
            let delta = self.entities[0].dir.to_vec2();
            dx = -delta.x * KNOCKBACK_SPEED * dt;
            dy = -delta.y * KNOCKBACK_SPEED * dt;
        } else {
            if dx > 0.0 {
                self.entities[0].dir = Dir::E;
            }
            if dx < 0.0 {
                self.entities[0].dir = Dir::W;
            }
            if dy > 0.0 {
                self.entities[0].dir = Dir::N;
            }
            if dy < 0.0 {
                self.entities[0].dir = Dir::S;
            }
        }
        if self.attack_timer <= 0.0 && input.is_key_pressed(Key::Space) {
            // TODO POINT: compute the attack area's center based on the player's position and facing and some offset
            // For the spritesheet provided, the attack is placed 8px "forwards" from the player.
            self.attack_area = todo!();
            self.attack_timer = ATTACK_MAX_TIME;
        } else if self.attack_timer <= ATTACK_COOLDOWN_TIME {
            self.attack_area = Rect {
                x: 0.0,
                y: 0.0,
                w: 0,
                h: 0,
            };
        }
        let dest = self.entities[0].pos + Vec2 { x: dx, y: dy };
        self.entities[0].pos = dest;
        let mut rng = rand::thread_rng();
        for enemy in self.entities[1..].iter_mut() {
            if rng.gen_bool(0.05) {
                enemy.dir = match rng.gen_range(0..4) {
                    0 => Dir::N,
                    1 => Dir::E,
                    2 => Dir::S,
                    3 => Dir::W,
                    _ => panic!(),
                };
            }
            enemy.pos += enemy.dir.to_vec2() * ENEMY_SPEED * DT;
        }
        // ----
        // TODO POINT: implement collision detection here.
        // first, get a rectangle for each entity.  You can use iter() and map() for this, along with Entity::rect.
        // next, make either one Contact struct (with a_index, a_rectangle, b_index, b_rectangle, and overlap fields) or two structs (LevelContact with a_index, b_rectangle, overlap, and EntContact with a_index, b_index, overlap).
        // then, write a function to get contacts between a slice of rectangles and a level.  for collision of each rect with the tilemap, you can use Level::tiles_within to find the tiles touching that rectangle, and filter out the ones that are not solid. print out how many contacts you have and make sure it seems right.
        // then, write a function to get contacts between two slices of entities, or to check all entity-entity contacts.  print out the results and make sure they make sense.
        // next, write your collision response versus the level code (per the pseudocode on miro); it's probably a good idea to write a get_displacement(rect_a, rect_b) function to find the right offset, and add that directly to the entity involved in the collision.
        // finally, implement your collision event handling for entity-entity collisions (harming the player, defeating enemies, etc).
        // TODO POINT: damage and knock back the player (you can use knockback_timer & health fields of game; you want the player to be invulnerable temporarily after hitting an enemy, so just decreasing health on its own won't work!)
        // TODO POINT: damage/destroy the enemy

        // You could have helper functions like gather_contacts(&[rect], &[rect], &mut contacts_vec) or do_collision_response(&contacts, &mut group_a, &group_b) or compute_displacement(rect, rect) -> Vec2.
        // A Contact struct is a good idea (with displacement, a_index, a_rect, b_index, and b_rect fields).
        // Then, you can check for contacts between the player & their attack rectangle on one side, and the enemies on the other side (you can reuse gather_contacts for this).  These don't need to participate in collision response, but you can use them to determine whether the player or enemy should be damaged.

        // For deleting enemies, it's best to add the enemy to a "to_remove" vec, and then remove those enemies after this loop is all done.
        // Alternatively, you could "disable" an enemy by giving it an `alive` flag or similar and setting that to false, not drawing or updating dead enemies.
    }
}
