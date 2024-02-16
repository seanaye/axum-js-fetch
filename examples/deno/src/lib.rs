use axum::{
    body::Body,
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
) -> Body {
    let stream: futures_lite::stream::Repeat<Result<String, Infallible>> = stream::repeat(Ok(payload.hello));
    Body::from_stream(stream)
}

async fn handler3() -> Body {
    let chunks : Vec<Result<&'static str, Infallible>> = vec![Ok("Hello,"), Ok(" "), Ok("world!")];
    let stream = stream::iter(chunks);
    Body::from_stream(stream)
}

