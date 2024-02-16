use axum::{
    body::Body,
    extract::Path,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use axum_js_fetch::App;
use futures_lite::stream;
use std::convert::Infallible;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

impl Default for MyApp {
    fn default() -> Self {
        let app = Router::new()
            .route("/", get(handler))
            .route("/count/:i", post(count));
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

    #[wasm_bindgen]
    pub async fn serve(&self) -> () {
        todo!()
    }
}

async fn handler() -> Body {
    let chunks: Vec<Result<&'static str, Infallible>> = vec![Ok("Hello"), Ok(" "), Ok("world.")];
    let stream = stream::iter(chunks);
    Body::from_stream(stream)
}

async fn count(Path(i): Path<usize>) -> impl IntoResponse {
    let i = i + 1;
    i.to_string()
}
#[wasm_bindgen_test]
async fn oneshot() {
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
