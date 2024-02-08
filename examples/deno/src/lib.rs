use axum::{
    body::StreamBody,
    extract::{Json, Query},
    routing::get,
    Router,
};
use axum_js_fetch::App;
use futures_lite::{stream, Stream};
use serde::Deserialize;
use std::{collections::HashMap, convert::Infallible};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct MyApp(App);

impl Default for MyApp {
    fn default() -> Self {
        let app = Router::new()
            .route("/", get(handler).post(handler2))
            .route("/stream", get(handler3));
        Self(App::new(app))
    }
}

#[wasm_bindgen]
impl MyApp {
    #[wasm_bindgen]
    pub fn new() -> Self {
        Self::default()
    }

    #[wasm_bindgen]
    pub async fn serve(&self, req: web_sys::Request) -> web_sys::Response {
        self.0.oneshot(req).await
    }
}

async fn handler(Query(params): Query<HashMap<String, String>>) -> String {
    format!("received query string: {:?}", params)
}

#[derive(Debug, Deserialize)]
struct TestStruct {
    hello: String,
}

async fn handler2(
    Json(payload): Json<TestStruct>,
) -> StreamBody<impl Stream<Item = Result<String, Infallible>>> {
    let stream = stream::repeat(Ok(payload.hello));
    StreamBody::new(stream)
}

async fn handler3() -> StreamBody<impl Stream<Item = std::io::Result<&'static str>>> {
    let chunks = vec![Ok("Hello,"), Ok(" "), Ok("world!")];
    let stream = stream::iter(chunks);
    StreamBody::new(stream)
}
