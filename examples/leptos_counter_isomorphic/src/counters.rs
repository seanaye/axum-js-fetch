use leptos::*;
use leptos_meta::*;
use leptos_router::*;
#[cfg(feature = "ssr")]
use tracing::instrument;

pub mod ssr_imports {
    pub use broadcaster::BroadcastChannel;
    pub use once_cell::sync::OnceCell;
    pub use std::sync::atomic::{AtomicI32, Ordering};

    pub static COUNT: AtomicI32 = AtomicI32::new(0);

    lazy_static::lazy_static! {
        pub static ref COUNT_CHANNEL: BroadcastChannel<i32> = BroadcastChannel::new();
    }

    static LOG_INIT: OnceCell<()> = OnceCell::new();

    pub fn init_logging() {
        LOG_INIT.get_or_init(|| {
            simple_logger::SimpleLogger::new().env().init().unwrap();
        });
    }
}

#[server(endpoint="count")]
#[cfg_attr(feature = "ssr", instrument)]
pub async fn get_server_count() -> Result<i32, ServerFnError> {
    use ssr_imports::*;
    Ok(COUNT.load(Ordering::Relaxed))
}

#[server]
#[cfg_attr(feature = "ssr", instrument)]
pub async fn adjust_server_count(
    delta: i32,
    msg: String,
) -> Result<i32, ServerFnError> {
    use ssr_imports::*;

    let new = COUNT.load(Ordering::Relaxed) + delta;
    COUNT.store(new, Ordering::Relaxed);
    _ = COUNT_CHANNEL.send(&new).await;
    println!("message = {:?}", msg);
    Ok(new)
}

#[server]
#[cfg_attr(feature = "ssr", instrument)]
pub async fn clear_server_count() -> Result<i32, ServerFnError> {
    use ssr_imports::*;

    COUNT.store(0, Ordering::Relaxed);
    _ = COUNT_CHANNEL.send(&0).await;
    Ok(0)
}

#[component]
pub fn Counters() -> impl IntoView {
    #[cfg(feature = "ssr")]
    ssr_imports::init_logging();

    provide_meta_context();
    view! {
        <Router>
            <header>
                <h1>"Server-Side Counters"</h1>
                <p>"Each of these counters stores its data in the same variable on the server."</p>
                <p>
                    "The value is shared across connections. Try opening this is another browser tab to see what I mean."
                </p>
            </header>
            <nav>
                <ul>
                    <li>
                        <A href="">"Simple"</A>
                    </li>
                    <li>
                        <A href="form">"Form-Based"</A>
                    </li>
                    <li>
                        <A href="multi">"Multi-User"</A>
                    </li>
                </ul>
            </nav>
            <Link rel="shortcut icon" type_="image/ico" href="/favicon.ico"/>
            <main>
                <Routes>
                    <Route
                        path=""
                        view=|| {
                            view! { <Counter/> }
                        }
                    />
                </Routes>
            </main>
        </Router>
    }
}

// This is an example of "single-user" server functions
// The counter value is loaded from the server, and re-fetches whenever
// it's invalidated by one of the user's own actions
// This is the typical pattern for a CRUD app
#[component]
pub fn Counter() -> impl IntoView {
    let dec = create_action(|_: &()| adjust_server_count(-1, "decing".into()));
    let inc = create_action(|_: &()| adjust_server_count(1, "incing".into()));
    let clear = create_action(|_: &()| clear_server_count());
    let counter = create_resource(
        move || {
            (
                dec.version().get(),
                inc.version().get(),
                clear.version().get(),
            )
        },
        |_| get_server_count(),
    );

    view! {
        <div>
            <h2>"Simple Counter"</h2>
            <p>
                "This counter sets the value on the server and automatically reloads the new value."
            </p>
            <div>
                <button on:click=move |_| clear.dispatch(())>"Clear"</button>
                <button on:click=move |_| dec.dispatch(())>"-1"</button>
                <Suspense fallback=move || view!{ <span>"Value: "</span>}>
                  <span>"Value: " { counter.get().map(|count| count.unwrap_or(0)).unwrap_or(0);} "!"</span>
                </Suspense>
                <button on:click=move |_| inc.dispatch(())>"+1"</button>
            </div>
            <Suspense>
              {move || {
                counter.get().and_then(|res| match res {
                  Ok(_) => None,
                  Err(e) => Some(e),
                }).map(|msg| {
                  view! { <p>"Error: " {msg.to_string()}</p> }
                })
              }}
            </Suspense>
        </div>
    }
}
