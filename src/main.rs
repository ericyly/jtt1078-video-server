use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension,
    },
    http::header::{self, HeaderMap},
    response::IntoResponse,
    routing::get,
    Error, Router,
};
use bytes::{Buf, Bytes};
use chrono::prelude::*;
use std::{collections::HashMap, net::SocketAddr, path::Path, sync::Arc, time::Duration};
use tokio::{fs::File, io::AsyncReadExt, time};

static FILE_ITEMS: [&str; 8] = [
    "web_av_app.html",
    "common.js",
    "jts_player.js",
    "decode_worker.js",
    "stream_decoder.js",
    "stream_decoder.wasm",
    "favicon.ico",
    "jiupin-352-288-time-15-audio.rtp",
];

async fn load_file(path: &Path) -> Bytes {
    let mut contents = vec![];
    File::open(path)
        .await
        .expect("open file error")
        .read_to_end(&mut contents)
        .await
        .expect("buffer read_to_end error");
    Bytes::from(contents)
}

async fn init() -> HashMap<String, Bytes> {
    let mut cur_path = std::env::current_dir().unwrap();
    cur_path.push("dist");
    let mut ret = HashMap::new();
    for file in FILE_ITEMS {
        let mut path = cur_path.clone();
        path.push(file);
        let bytes = load_file(path.as_path()).await;
        ret.insert(file.to_string(), bytes);
    }
    ret
}

#[tokio::main]
async fn main() {
    let share_state = Arc::new(init().await);
    let app = Router::new()
        .route("/", get(get_content_handler))
        .route("/web_av_app.html", get(get_content_handler))
        .route("/common.js", get(get_content_handler))
        .route("/jts_player.js", get(get_content_handler))
        .route("/decode_worker.js", get(get_content_handler))
        .route("/stream_decoder.js", get(get_content_handler))
        .route("/stream_decoder.wasm", get(get_content_handler))
        .route("/favicon.ico", get(get_content_handler))
        .route("/tm", get(ws_handler))
        .layer(Extension(share_state));
    let addr = SocketAddr::from(([0, 0, 0, 0], 8088));
    println!("jts-svr listening on: {} ...", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn get_content_handler(
    uri: axum::http::Uri,
    Extension(state): Extension<Arc<HashMap<String, Bytes>>>,
) -> impl IntoResponse {
    let p = uri.path();
    let mut headers = HeaderMap::new();
    let mut content = state.get("web_av_app.html").unwrap().slice(..);
    if p.ends_with("js") {
        headers
            .entry(header::CONTENT_TYPE)
            .or_insert("text/javascript; charset=UTF-8".parse().unwrap());
    } else if p.ends_with("wasm") {
        headers
            .entry(header::CONTENT_TYPE)
            .or_insert("application/wasm".parse().unwrap());
    } else if p.ends_with("html") {
        headers
            .entry(header::CONTENT_TYPE)
            .or_insert("text/html; charset=UTF-8".parse().unwrap());
    } else if p.ends_with("ico") {
        headers
            .entry(header::CONTENT_TYPE)
            .or_insert("image/x-icon".parse().unwrap());
    }
    if let Some(bytes) = state.get(&p[1..].to_string()) {
        content = bytes.slice(..);
    }
    headers
        .entry(header::CONTENT_TYPE)
        .or_insert("text/html; charset=UTF-8".parse().unwrap());
    (headers, content)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<HashMap<String, Bytes>>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<HashMap<String, Bytes>>) {
    let _ = send_rtp_pkt(socket, state).await;
}

async fn send_rtp_pkt(
    mut socket: WebSocket,
    state: Arc<HashMap<String, Bytes>>,
) -> Result<(), Error> {
    let skip_key_frames = (rand::random::<f32>() * 97.0) as i32;
    let mut skipped = 0;
    let bytes = state.get(FILE_ITEMS[7]).unwrap();
    let mut rtp_stream = bytes.clone();
    let mut out_video_pkt_time = 0i64;
    let video_frame_interval = (1000.0 / 15.0 + 0.5) as i64;
    while rtp_stream.len() > 0 {
        let data_type = rtp_stream.slice(15..16).get_u8();
        let mut video_added = 0;
        if data_type != 0x30 {
            video_added = 4;
            if data_type == 0 && skipped < skip_key_frames {
                skipped += 1;
            }
        }
        let start = 24 + video_added;
        let len = rtp_stream.slice(start..start + 2).get_u16();
        let total_len = 26 + video_added + len as usize;
        let rtp_pkt = rtp_stream.slice(..total_len).to_vec();
        rtp_stream.advance(total_len);
        if skipped < skip_key_frames {
            continue;
        }
        if video_added == 4 {
            if out_video_pkt_time != 0 {
                let now = Local::now().timestamp_millis();
                let time_wait = video_frame_interval - (now - out_video_pkt_time);
                if time_wait > 0 {
                    time::sleep(Duration::from_millis(time_wait as u64)).await;
                }
                out_video_pkt_time = Local::now().timestamp_millis();
                socket.send(Message::Binary(rtp_pkt)).await?;
            } else {
                out_video_pkt_time = Local::now().timestamp_millis();
                socket.send(Message::Binary(rtp_pkt)).await?;
            }
        } else {
            socket.send(Message::Binary(rtp_pkt)).await?;
        }
    }
    Ok(())
}
