use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct State {
    pub pos: Vec2,
    pub r: f32,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct RemoteState {
    pub id: usize,
    pub position: Vec2,
    pub rotation: f32,
}

impl RemoteState {
    pub fn new(id: usize, x: f32, y: f32, rotation: f32) -> Self {
        Self {
            id,
            position: Vec2::new(x, y),
            rotation,
        }
    }
    pub fn id(&mut self, id: usize) {
        self.id = id;
    }
    pub fn update_position(&mut self, x: f32, y: f32, speed: f32) {
        self.position += Vec2::new(x, y) * speed;
    }
    pub fn rotation(&mut self, rotation: f32) {
        self.rotation = rotation;
    }
}

#[derive(Deserialize, Serialize)]
pub enum ServerMessage {
    Welcome(usize),
    GoodBye(usize),
    Update(Vec<RemoteState>),
}

#[derive(Deserialize, Serialize)]
pub enum ClientMessage {
    State(State),
}
