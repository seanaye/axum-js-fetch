use axum::{body::HttpBody, response::IntoResponse};
use bytes::{buf::Buf, Bytes};
use futures_lite::stream::{Stream, StreamExt};
use http::Request;
use http_body::Frame;
use js_sys::Uint8Array;
use std::{convert::Infallible, error::Error, pin::Pin, sync::Arc, task::Poll};
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

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        Pin::into_inner(self)
            .stream
            .poll_next(cx)
            .map(|d| d.map(Frame::data).map(Ok))
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

        Pin::new(&mut s.stream).poll_frame(cx).map(|f| match f {
            Some(Ok(frame)) => {
                match frame.into_data() {
                    Ok(b) => {
                        let slice = b.chunk();
                        let a = Uint8Array::new(&JsValue::from(slice.len()));
                        a.copy_from(slice);
                        let out: JsValue = a.into();
                        Some(Ok(out))
                    }
                    // Not 100% sure how to best handle this error besides informing the user.
                    // This gives a frame that isn't a data frame and I don't know how that could be useful here?
                    Err(_) => Some(Err(JsValue::from_str("Not a data frame"))),
                }
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

fn to_http_body(req: gloo_net::http::Request) -> axum::body::Body {
    let (sender, rx) = async_channel::unbounded::<Result<bytes::Bytes, Infallible>>();
    let arc_sender = Arc::new(sender);

    if let Some(b) = req.body() {
        spawn_local(async move {
            ReadableStream::from_raw(b.unchecked_into())
                .into_stream()
                .try_for_each(|buf_js| -> Result<(), Err> {
                    let buffer = js_sys::Uint8Array::new(&buf_js.map_err(|_| Err::ConvertError)?);
                    let bytes: Bytes = buffer.to_vec().into();
                    let sender_clone = arc_sender.clone();

                    // dont block on sending bytes into stream
                    spawn_local(async move {
                        let _ = sender_clone.send(Ok(bytes)).await;
                    });
                    Ok(())
                })
                .await
                .unwrap();
        });
    }

    axum::body::Body::from_stream(rx)
}

fn from_fetch_request(req: web_sys::Request) -> Request<axum::body::Body> {
    let gloo_req = gloo_net::http::Request::from(req);
    let headers = gloo_req.headers();

    let mut builder = Request::builder()
        .uri(gloo_req.url())
        .method(gloo_req.method().as_str());

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

fn to_fetch_response(res: impl IntoResponse) -> web_sys::Response {
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
    pub service: BoxCloneService<web_sys::Request, web_sys::Response, BoxError>,
}

impl App {
    pub fn new<S>(service: S) -> Self
    where
        S: ServiceExt<Request<axum::body::Body>> + Clone + Send + Sized + 'static,
        S::Future: Send + 'static,
        S::Response: IntoResponse,
        S::Error: Into<Box<dyn Error + Send + Sync>>,
    {
        let svc = ServiceBuilder::new()
            .map_request(from_fetch_request)
            .map_response(to_fetch_response)
            .service(service)
            .map_err(|e| e.into());

        let service = svc.boxed_clone();
        Self { service }
    }

    pub async fn oneshot(&self, req: web_sys::Request) -> web_sys::Response {
        self.service.clone().oneshot(req).await.unwrap()
    }

    // TODO
    /*
    pub async fn serve(&self, ?) -> ? {
        Listen for a stream of web_sys::Requests and return a stream of web_sys::Responses
        recreating the server functionality in a wasm environment.
    }
    */
}
