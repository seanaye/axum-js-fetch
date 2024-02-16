use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use std::convert::Infallible;



wasm_bindgen_test_configure!(run_in_browser);



#[wasm_bindgen_test]
async fn test() {
    assert_eq!(JsFuture::from(
        MyApp::new()
            .serve(web_sys::Request::new_with_str("api/count").unwrap()).await.text()
    .unwrap()).await
    .unwrap()
    .as_string()
    .unwrap(),"0".to_string())
    

}