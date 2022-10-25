use macroquad::prelude::*;
use shared::{ClientMessage, RemoteState, ServerMessage, State};
use ws::Connection;
mod ws;

const PLANE_WIDTH: f32 = 32.;
const PLANE_HEIGHT: f32 = 32.;
#[macroquad::main("game")]
async fn main() {
    let mut connection = Connection::new();
    connection.connect("ws://localhost:3030/game");
    let mut game = Game::new().await;
    loop {
        let state = ClientMessage::State(State {
            pos: game.player_state.position,
            r: game.player_state.rotation,
        });

        client_send(&state, &mut connection);
        if let Some(msg) = connection.poll() {
            let msg = serde_json::from_slice::<ServerMessage>(msg.as_slice())
                .expect("deserialization failed");

            game.handle_message(msg);
        }

        game.update();
        game.draw();
        if game.quit {
            return;
        }
        next_frame().await
    }
}
pub fn client_send(msg: &ClientMessage, connection: &mut Connection) {
    let bytes = serde_json::to_vec(msg).expect("serialization failed");
    connection.send(bytes);
}

pub struct PlayerState {
    pub id: usize,
    pub position: Vec2,
    pub rotation: f32,
}

pub struct Game {
    pub quit: bool,
    pub texture: Texture2D,
    pub player_state: RemoteState,
    pub remote_states: Vec<RemoteState>,
}
fn draw_box(pos: Vec2, size: Vec2) {
    let dimension = size * 2.;
    let upper_left = pos - size;

    draw_rectangle(upper_left.x, upper_left.y, dimension.x, dimension.y, BLACK)
}

pub fn vec2_from_angle(angle: f32) -> (f32, f32) {
    let angle = angle - std::f32::consts::FRAC_PI_2;
    (angle.cos(), angle.sin())
}
impl Game {
    pub async fn new() -> Self {
        let texture = load_texture("assets/planes.png").await.unwrap();
        Self {
            quit: false,
            remote_states: Vec::new(),
            player_state: RemoteState::new(0, 100f32, 100f32, 0f32),
            texture,
        }
    }

    pub fn update(&mut self) {
        const ROT_SPEED: f32 = 0.015;

        const SPEED: f32 = 0.6;

        let (x, y) = vec2_from_angle(self.player_state.rotation);

        self.player_state.update_position(x, y, SPEED);

        if is_key_down(KeyCode::Escape) {
            self.quit = true;
        }
        if is_key_down(KeyCode::Right) {
            self.player_state.rotation += ROT_SPEED;
        }
        if is_key_down(KeyCode::Left) {
            self.player_state.rotation -= ROT_SPEED;
        }

        if self.player_state.position.x > screen_width() {
            self.player_state.position.x = -PLANE_WIDTH;
        } else if self.player_state.position.x < -PLANE_WIDTH {
            self.player_state.position.x = screen_width();
        }
        if self.player_state.position.y > screen_height() {
            self.player_state.position.y = -PLANE_HEIGHT;
        } else if self.player_state.position.y < -PLANE_HEIGHT {
            self.player_state.position.y = screen_height();
        }
    }

    pub fn draw_plane(&self, state: &RemoteState) {
        let cols = (self.texture.width() / PLANE_WIDTH).floor() as usize;
        let index = state.id % 10;
        let tx_x = index % cols;
        let tx_y = index / cols;

        draw_texture_ex(
            self.texture,
            state.position.x,
            state.position.y,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(
                    tx_x as f32 * PLANE_WIDTH,
                    tx_y as f32 * PLANE_HEIGHT,
                    PLANE_WIDTH,
                    PLANE_HEIGHT,
                )),
                rotation: state.rotation,
                ..Default::default()
            },
        )
    }

    pub fn draw(&self) {
        clear_background(color_u8!(255, 255, 255, 255));

        draw_box(Vec2::new(200f32, 200f32), Vec2::new(10f32, 10f32));

        self.draw_plane(&self.player_state);

        for state in &self.remote_states {
            self.draw_plane(state);
        }
    }

    pub fn handle_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::Welcome(id) => {
                self.player_state.id = id;
            }
            ServerMessage::GoodBye(id) => {
                self.remote_states.retain(|s| s.id != id);
            }
            ServerMessage::Update(remote_states) => {
                self.remote_states = remote_states;
            }
        }
    }
}
