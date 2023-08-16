use axum::{body::HttpBody, response::IntoResponse};
use bytes::{buf::Buf, Bytes};
use futures_lite::stream::{Stream, StreamExt};
use http::Request;
use http_body::combinators::BoxBody;
use js_sys::Uint8Array;
use std::{error::Error, pin::Pin, task::Poll};
use tower::{util::BoxCloneService, BoxError, ServiceBuilder, ServiceExt};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use wasm_streams::ReadableStream;

struct StreamingBody<T> {
    stream: T,
}

impl<T> HttpBody for StreamingBody<T>
where
    T: Stream<Item = Bytes> + Unpin,
{
    type Data = Bytes;

    type Error = BoxError;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        Pin::into_inner(self)
            .stream
            .poll_next(cx)
            .map(|d| d.map(Ok))
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        // todo!('add support for trailers')
        std::task::Poll::Ready(Ok(None))
    }
}

impl<T> Stream for StreamingBody<T>
where
    T: HttpBody + Unpin,
    T::Data: Buf,
    T::Error: Into<Box<dyn Error + Send + Sync>>,
{
    type Item = Result<JsValue, JsValue>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let s = Pin::into_inner(self);

        Pin::new(&mut s.stream).poll_data(cx).map(|f| match f {
            Some(Ok(b)) => {
                let slice = b.chunk();
                let a = Uint8Array::new(&JsValue::from(slice.len()));
                a.copy_from(slice);
                let out: JsValue = a.into();
                Some(Ok(out))
            }
            Some(Err(e)) => Some(Err(JsValue::from_str(&e.into().to_string()))),
            None => None,
        })
    }
}

#[derive(Debug)]
enum Err {
    ConvertError,
}

fn to_http_body(req: gloo_net::http::Request) -> OssiBody {
    let (tx, rx) = async_channel::unbounded::<Bytes>();

    match req.body() {
        Some(b) => {
            spawn_local(async move {
                let _ = ReadableStream::from_raw(b.unchecked_into())
                    .into_stream()
                    .try_for_each(|buf_js| -> Result<(), Err> {
                        let buffer =
                            js_sys::Uint8Array::new(&buf_js.map_err(|_| Err::ConvertError)?);
                        let bytes: Bytes = buffer.to_vec().into();
                        let tx = tx.clone();

                        // dont block on sending bytes into stream
                        spawn_local(async move {
                            let _ = tx.send(bytes).await;
                        });
                        Ok(())
                    })
                    .await;
            });
        }
        None => {
            tx.close();
        }
    }

    let streaming_body = StreamingBody { stream: rx };
    BoxBody::new(streaming_body)
}

type OssiBody = BoxBody<Bytes, BoxError>;

type OssiRequest = Request<OssiBody>;

fn to_ossi_request(req: web_sys::Request) -> OssiRequest {
    let gloo_req = gloo_net::http::Request::from(req);
    let headers = gloo_req.headers();

    let mut builder = Request::builder()
        .uri(gloo_req.url())
        .method(gloo_req.method());

    for (key, value) in headers.entries() {
        builder = builder.header(key, value)
    }

    let body = to_http_body(gloo_req);
    builder.body(body).unwrap()
}

fn create_default_error(e: impl Error) -> gloo_net::http::Response {
    gloo_net::http::Response::builder()
        .status(500)
        .body(Some(format!("{e}").as_str()))
        .unwrap()
}

fn to_ossi_response(res: impl IntoResponse) -> web_sys::Response {
    let (parts, body) = res.into_response().into_parts();
    let headers = gloo_net::http::Headers::new();
    for (key, value) in parts.headers.iter() {
        headers.append(key.as_str(), value.to_str().unwrap());
    }
    let stream_body: web_sys::ReadableStream =
        ReadableStream::from_stream(StreamingBody { stream: body })
            .into_raw()
            .unchecked_into();

    gloo_net::http::Response::builder()
        .status(parts.status.as_u16())
        .headers(headers)
        .body(Some(&stream_body))
        .unwrap_or_else(create_default_error)
        .into()
}

pub struct App {
    service: BoxCloneService<web_sys::Request, web_sys::Response, BoxError>,
}

// impl Default for App {
//     fn default() -> Self {
//         console_error_panic_hook::set_once();

//         let svc = ServiceBuilder::new()
//             .map_request(to_ossi_request)
//             .service(app)
//             .map_response(to_ossi_response);
//     }
// }

impl App {
    pub fn new<S>(service: S) -> Self
    where
        S: ServiceExt<OssiRequest> + Clone + Send + Sized + 'static,
        S::Future: Send + 'static,
        S::Response: IntoResponse,
        S::Error: Into<Box<dyn Error + Send + Sync>>,
    {
        let svc = ServiceBuilder::new()
            .map_request(to_ossi_request)
            .map_response(to_ossi_response)
            .service(service)
            .map_err(|e| e.into());

        let service = svc.boxed_clone();
        Self { service }
    }

    pub async fn serve(&self, req: web_sys::Request) -> web_sys::Response {
        self.service.clone().oneshot(req).await.unwrap()
    }
}

#[cfg(test)]
pub mod tests {
    use crate::App;
    use axum::{
        body::StreamBody,
        extract::{Json, Query},
        routing::get,
        Router,
    };
    use futures_lite::{stream, Stream};
    use serde::Deserialize;
    use std::{collections::HashMap, convert::Infallible};

    struct MyApp(App);

    impl Default for MyApp {
        fn default() -> Self {
            let app = Router::new()
                .route("/", get(handler).post(handler2))
                .route("/stream", get(handler3));
            Self(App::new(app))
        }
    }

    async fn handler(Query(params): Query<HashMap<String, String>>) -> String {
        format!("received: {:?}", params)
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
}
