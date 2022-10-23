use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use glam::Vec2;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

async fn user_connected(ws: WebSocket, users: Users, states: States) {
    use futures_util::StreamExt;
    let (ws_sender, mut ws_receiver) = ws.split();
    let send_channel = create_send_channel(ws_sender);
    let my_id = send_welcome(&send_channel).await;
    log::debug!("new user connected: {}", my_id);

    users.write().await.insert(my_id, send_channel);

    while let Some(result) = ws_receiver.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                log::warn!("websocket err (id={}): '{}'", my_id, e);
                break;
            }
        };

        log::debug!(" user sent message: {:?}", msg);

        if let Some(msg) = parse_message(msg) {
            user_message(my_id, msg, &states).await;
        }
    }
    log::debug!(" user disconnected: {:}", my_id);
    users.write().await.remove(&my_id);
    states.write().await.remove(&my_id);
    broadcast(ServerMessage::GoodBye(my_id), &users).await;
}

fn parse_message(msg: Message) -> Option<ClientMessage> {
    if msg.is_binary() {
        let msg = msg.into_bytes();
        serde_json::from_slice::<ClientMessage>(msg.as_slice()).ok()
    } else {
        None
    }
}

async fn user_message(my_id: usize, msg: ClientMessage, states: &States) {
    match msg {
        ClientMessage::State(state) => {
            let msg = RemoteState {
                id: my_id,
                position: state.pos,
                rotation: state.r,
            };
            states.write().await.insert(msg.id, msg);
        }
    }
}

async fn broadcast(msg: ServerMessage, users: &Users) {
    let users = users.read().await;
    for (_, tx) in users.iter() {
        send_msg(tx, &msg).await;
    }
}

type OutBoundChannel = mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>;

fn create_send_channel(
    ws_sender: futures_util::stream::SplitSink<WebSocket, Message>,
) -> OutBoundChannel {
    use futures_util::FutureExt;
    use futures_util::StreamExt;

    use tokio_stream::wrappers::UnboundedReceiverStream;

    let (sender, receiver) = mpsc::unbounded_channel();
    let rx = UnboundedReceiverStream::new(receiver);

    tokio::task::spawn(rx.forward(ws_sender).map(|result| {
        if let Err(e) = result {
            log::error!("websocket send error: {}", e);
        }
    }));

    sender
}

static NEXT_YSER_ID: AtomicUsize = AtomicUsize::new(1);

async fn send_welcome(out: &OutBoundChannel) -> usize {
    let id = NEXT_YSER_ID.fetch_add(1, Ordering::Relaxed);
    let states = ServerMessage::Welcome(id);
    send_msg(out, &states).await;
    id
}

type Users = Arc<RwLock<HashMap<usize, OutBoundChannel>>>;
type States = Arc<RwLock<HashMap<usize, RemoteState>>>;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let users = Users::default();
    let states = States::default();

    let users = warp::any().map(move || users.clone());
    let states = warp::any().map(move || states.clone());

    let game = warp::path("game")
        .and(warp::ws())
        .and(users)
        .and(states)
        .map(|ws: warp::ws::Ws, users, states| {
            ws.on_upgrade(|socket| user_connected(socket, users, states))
        });

    let status = warp::path!("status").map(move || warp::reply::html("hello"));
    let routes = status.or(game);
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

async fn send_msg(tx: &OutBoundChannel, msg: &ServerMessage) {
    let buffer = serde_json::to_vec(&msg).expect("Not possible to convert to binary");
    let msg = Message::binary(buffer);
    tx.send(Ok(msg)).expect("sending message failed");
}

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
