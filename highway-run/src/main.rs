// TODO: use AABB instead of Rect for centered box, so collision checking doesn't have to offset by half size

use engine_simple as engine;
use engine_simple::wgpu;
use engine_simple::{geom::*, Camera, Engine, SheetRegion, Transform, Zeroable};
use rand::{Rng, distributions::Uniform};
const W: f32 = 768.0;
const H: f32 = 1280.0;
const GUY_SPEED: f32 = 4.0;
const SPRITE_MAX: usize = 16;
const CATCH_DISTANCE: f32 = 8.0;
const COLLISION_STEPS: usize = 3;
struct Guy {
    pos: Vec2,
    is_jumping: bool,
    jump_velocity: f32,
}

struct Car {
    pos: Vec2,
    vel: Vec2,
}

struct Coin {
    pos: Vec2,
    vel: Vec2,
}

struct Game {
    camera: engine::Camera,
    walls: Vec<AABB>,
    guy: Guy,
    cars: Vec<Car>,
    car_timer: u32,
    coins: Vec<Coin>, 
    coin_timer: u32,
    score: u32,
    font: engine_simple::BitFont,
    curr_frame: usize,
    frame_counter: usize,
    frame_direction: isize,
    game_over: bool,
    floor_y_position: f32,
}

impl engine::Game for Game {
    fn new(engine: &mut Engine) -> Self {
        let camera = Camera {
            screen_pos: [0.0, 0.0],
            screen_size: [W, H],
        };
        #[cfg(target_arch = "wasm32")]
        let sprite_img = {
            let img_bytes = include_bytes!("content/demo.png");
            image::load_from_memory_with_format(&img_bytes, image::ImageFormat::Png)
                .map_err(|e| e.to_string())
                .unwrap()
                .into_rgba8()
        };
        #[cfg(not(target_arch = "wasm32"))]
        let sprite_img = image::open("../content/spritesheet.png").unwrap().into_rgba8();
        let sprite_tex = engine.renderer.gpu.create_texture(
            &sprite_img,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            sprite_img.dimensions(),
            Some("spr-demo.png"),
        );
        engine.renderer.sprites.add_sprite_group(
            &engine.renderer.gpu,
            &sprite_tex,
            vec![Transform::zeroed(); SPRITE_MAX], //bg, three walls, guy, a few cars
            vec![SheetRegion::zeroed(); SPRITE_MAX],
            camera,
        );
        let guy = Guy {
            pos: Vec2 {
                x: 378.66,
                y: 24.0,
            },
            is_jumping: false,
            jump_velocity: 0.0,
        };
        let floor_y_position = 16.0;
        let floor = AABB {
            center: Vec2 { x: W / 2.0, y: 8.0 },
            size: Vec2 { x: W, y: 16.0 },
        };
        let left_wall = AABB {
            center: Vec2 { x: 8.0, y: H / 2.0 },
            size: Vec2 { x: 288.0, y: H },
        };
        let right_wall = AABB {
            center: Vec2 {
                x: W-8.0,
                y: H / 2.0,
            },
            size: Vec2 { x: 288.0, y: H },
        };

        let font = engine::BitFont::with_sheet_region(
            '0'..='9',
            SheetRegion::new(0, 0, 512, 0, 80, 8),
            10,
        );
        Game {
            camera,
            guy,
            walls: vec![left_wall, right_wall, floor],
            cars: Vec::with_capacity(16),
            car_timer: 0,
            coins: Vec::with_capacity(16),
            coin_timer: 0,
            score: 0,
            font,
            curr_frame: 0,
            frame_counter: 0,
            frame_direction: 1,
            game_over: false,
            floor_y_position,
        }
    }
    fn is_game_over(&self) -> bool {
        self.game_over
    }
    fn update(&mut self, engine: &mut Engine) {
        let mut now = std::time::Instant::now();
        // set the speed of animation for guy. Adjust number after modulo.
        self.frame_counter = (self.frame_counter + 1) % 5;
        if self.frame_counter == 0 {
            // Update the current frame based on the direction
            self.curr_frame = (self.curr_frame as isize + self.frame_direction) as usize;

            // Change the direction if reaching the boundaries
            if self.curr_frame == 2 || self.curr_frame == 0 {
                self.frame_direction *= -1;
            }
        }

        let dir = engine.input.key_axis(engine::Key::Left, engine::Key::Right);
        self.guy.pos.x += dir * GUY_SPEED;
        let mut contacts = Vec::with_capacity(self.walls.len());

        // for jumping
        if engine.input.is_key_pressed(engine::Key::Up) && !self.guy.is_jumping {
            self.guy.is_jumping = true;
            self.guy.jump_velocity = 8.0; // You can adjust the initial jump velocity
        }
        
        // update Guy Position for Jumping
        if self.guy.is_jumping {
            self.guy.pos.y += 100.0; // this number can be changed
            self.guy.jump_velocity -= 0.2; // Adjust the gravity value as needed

            // Check if the guy has landed
            if self.guy.pos.y >= self.floor_y_position {
                // self.guy.pos.y = self.floor_y_position;
                self.guy.is_jumping = false;
            }
        }

        // TODO: for multiple guys this might be better as flags on the guy for what side he's currently colliding with stuff on
        for _iter in 0..COLLISION_STEPS {
            let guy_aabb = AABB {
                center: self.guy.pos,
                size: Vec2 { x: 38.4, y: 85.33 },
            };
            contacts.clear();
            // TODO: to generalize to multiple guys, need to iterate over guys first and have guy_index, rect_index, displacement in a contact tuple
            contacts.extend(
                self.walls
                    .iter()
                    .enumerate()
                    .filter_map(|(ri, w)| w.displacement(guy_aabb).map(|d| (ri, d))),
            );
            if contacts.is_empty() {
                break;
            }
            // This part stays mostly the same for multiple guys, except the shape of contacts is different
            contacts.sort_by(|(_r1i, d1), (_r2i, d2)| {
                d2.length_squared()
                    .partial_cmp(&d1.length_squared())
                    .unwrap()
            });
            for (wall_idx, _disp) in contacts.iter() {
                // TODO: for multiple guys should access self.guys[guy_idx].
                let guy_aabb = AABB {
                    center: self.guy.pos,
                    size: Vec2 { x: 38.4, y: 85.33 },
                };
                let wall = self.walls[*wall_idx];
                let mut disp = wall.displacement(guy_aabb).unwrap_or(Vec2::ZERO);
                // We got to a basically zero collision amount
                if disp.x.abs() < std::f32::EPSILON || disp.y.abs() < std::f32::EPSILON {
                    break;
                }
                // Guy is left of wall, push left
                if self.guy.pos.x < wall.center.x {
                    disp.x *= -1.0;
                }
                // Guy is below wall, push down
                if self.guy.pos.y < wall.center.y {
                    disp.y *= -1.0;
                }
                if disp.x.abs() <= disp.y.abs() {
                    self.guy.pos.x += disp.x;
                    // so far it seems resolved; for multiple guys this should probably set a flag on the guy
                } else if disp.y.abs() <= disp.x.abs() {
                    self.guy.pos.y += disp.y;
                    // so far it seems resolved; for multiple guys this should probably set a flag on the guy
                }
            }
        }
        let mut rng = rand::thread_rng();
        let possible_values = [261.33, 378.66, 496.0];

        // create columns for cars
        let uniform = Uniform::new(0, possible_values.len());
        let random_index = rng.sample(uniform);
        let random_value = possible_values[random_index];

        if self.car_timer > 0 {
            self.car_timer -= 1;
        } else if self.cars.len() < 8 {
            self.cars.push(Car {
                pos: Vec2 {
                    x: random_value,
                    y: H + 8.0,
                },
                vel: Vec2 {
                    x: 0.0,
                    y: rng.gen_range((-4.0)..(-1.0)),
                },
            });
            self.car_timer = rng.gen_range(30..90);
        }
        for car in self.cars.iter_mut() {
            car.pos += car.vel;
        }
        if let Some(idx) = self
            .cars
            .iter()
            .position(|car| car.pos.distance(self.guy.pos) <= CATCH_DISTANCE)
        {
            self.game_over = true;
            // self.cars.swap_remove(idx);
            // self.score += 1;
        } 
        self.cars.retain(|car| car.pos.y > -8.0);
        
        // Spawn new coins
        if self.coin_timer > 0 {
            self.coin_timer -= 1;
        } else if self.coins.len() < 8 {
            self.coins.push(Coin {
                pos: Vec2 {
                    x: random_value,
                    y: H + 8.0,
                },
                vel: Vec2 {
                    x: 0.0,
                    y: rng.gen_range((-4.0)..(-1.0)),
                },
            });
            self.coin_timer = rng.gen_range(30..90);
        }
        // Update coins
        for coin in self.coins.iter_mut() {
            coin.pos += coin.vel;
        }
        self.coins.retain(|coin| coin.pos.y > -8.0);
    }
    fn render(&mut self, engine: &mut Engine) {
        // set bg image
        let (trfs, uvs) = engine.renderer.sprites.get_sprites_mut(0);
        trfs[0] = AABB {
            center: Vec2 {
                x: W / 2.0,
                y: H / 2.0,
            },
            size: Vec2 { x: W, y: H },
        }
        .into();
        uvs[0] = SheetRegion::new(0, 0, 0, 16, 640, 480);
        // set walls
        const WALL_START: usize = 1;
        let guy_idx = WALL_START + self.walls.len();
        for (wall, (trf, uv)) in self.walls.iter().zip(
            trfs[WALL_START..guy_idx]
                .iter_mut()
                .zip(uvs[WALL_START..guy_idx].iter_mut()),
        ) {
            *trf = (*wall).into();
            *uv = SheetRegion::new(0, 0, 480, 12, 8, 8);
        }
        // set guy
        trfs[guy_idx] = AABB {
            center: self.guy.pos,
            size: Vec2 { x: 38.4, y: 85.33 },
        }
        .into();
        // TODO animation frame

        // animate the guy character
        let ones_place = self.curr_frame % 10;
        match ones_place {
            0  => {
                uvs[guy_idx] = SheetRegion::new(0, 100, 498, 8, 14, 18);
            }
            1 => {
                uvs[guy_idx] = SheetRegion::new(0, 114, 480, 8, 14, 18);
            }
            2 => {
                uvs[guy_idx] = SheetRegion::new(0, 114, 498, 8, 14, 18);
            }
            _ => {
                // Handle other cases if needed
            }
        }

        // uvs[guy_idx] = SheetRegion::new(0, 100, 480, 8, 14, 18);
        // set car
        let car_start = guy_idx + 1;
        for (car, (trf, uv)) in self.cars.iter().zip(
            trfs[car_start..]
                .iter_mut()
                .zip(uvs[car_start..].iter_mut()),
        ) {
            *trf = AABB {
                center: car.pos,
                size: Vec2 { x: 38.4, y: 85.33 },
            }
            .into();
            *uv = SheetRegion::new(0, 27, 525, 4, 27, 32);
        }
        // set coin
        let coin_start = car_start + self.cars.len();
        for (coin, (trf, uv)) in self.coins.iter().zip(
            trfs[coin_start..]
                .iter_mut()
                .zip(uvs[coin_start..].iter_mut()),
        ) {
            *trf = AABB {
                center: coin.pos,
                size: Vec2 { x: 38.4, y: 85.33 },
            }
            .into();
            *uv = SheetRegion::new(0, 0, 525, 4, 27, 32);
        }

        let sprite_count = coin_start + self.coins.len();
        // let sprite_count = car_start + self.cars.len();

        let score_str = self.score.to_string();
        let text_len = score_str.len();
        engine.renderer.sprites.resize_sprite_group(
            &engine.renderer.gpu,
            0,
            sprite_count + text_len,
        );
        self.font.draw_text(
            &mut engine.renderer.sprites,
            0,
            sprite_count,
            &score_str,
            Vec2 {
                x: 16.0,
                y: H - 16.0,
            }
            .into(),
            16.0,
        );
        let text_start = coin_start + self.coins.len();
        engine.renderer.sprites.resize_sprite_group(
            &engine.renderer.gpu,
            0,
            text_start + text_len,
        );
        engine
            .renderer
            .sprites
            .upload_sprites(&engine.renderer.gpu, 0, 0..sprite_count + text_len);
        engine
            .renderer
            .sprites
            .set_camera_all(&engine.renderer.gpu, self.camera);
    }
}
fn main() {
    Engine::new(winit::window::WindowBuilder::new()).run::<Game>();
}