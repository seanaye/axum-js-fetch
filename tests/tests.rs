use axum::{body::Body, routing::get, Router};
use axum_js_fetch::App;
use futures_lite::stream;
use std::convert::Infallible;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

impl Default for MyApp {
    fn default() -> Self {
        let app = Router::new().route("/", get(handler));
        Self(App::new(app))
    }
}
#[wasm_bindgen]
pub struct MyApp(App);

#[wasm_bindgen]
impl MyApp {
    #[wasm_bindgen]
    pub fn new() -> Self {
        Self::default()
    }

    #[wasm_bindgen]
    pub async fn oneshot(&self, req: web_sys::Request) -> web_sys::Response {
        self.0.oneshot(req).await
    }
}

async fn handler() -> Body {
    let chunks: Vec<Result<&'static str, Infallible>> = vec![Ok("Hello"), Ok(" "), Ok("world.")];
    let stream = stream::iter(chunks);
    Body::from_stream(stream)
}

#[wasm_bindgen_test]
async fn test() {
    assert_eq!(
        JsFuture::from(
            MyApp::new()
                .oneshot(web_sys::Request::new_with_str("/").unwrap())
                .await
                .text()
                .unwrap()
        )
        .await
        .unwrap()
        .as_string()
        .unwrap(),
        "Hello world.".to_string()
    )
}
