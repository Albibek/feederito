use std::rc::Rc;

use dioxus::fermi::*;
use dioxus::prelude::*;
use futures::{SinkExt, StreamExt};
use log::{debug, warn};

use model::events::*;
use model::feed::*;

use crate::aws::*;
use crate::entries::*;

use crate::WORKER_BRIDGE;
use gloo_worker::WorkerBridge;
use worker::{BackendWorker, WorkerRequest, WorkerResponse};

pub static FEEDS: AtomRef<DisplayedFeeds> = |_| DisplayedFeeds::default();
#[derive(PartialEq, Eq, Clone)]
pub struct DisplayedFeed {
    pub stored: StoredFeed,
    pub enabled: bool,
}

pub struct DisplayedFeeds {
    pub feeds: Vec<DisplayedFeed>,
}

impl Default for DisplayedFeeds {
    fn default() -> Self {
        DisplayedFeeds { feeds: Vec::new() }
    }
}

enum FeedsAction {
    Replace(Vec<StoredFeed>),
    FlipEnabled(FeedID),
    BackendResponse(Option<Vec<u8>>),
}

/// The feeds processor coroutine, handles all the signals related to feed entries processing
#[derive(Clone)]
pub struct FeedsHandle {
    actor: CoroutineHandle<FeedsAction>,
    bridge: WorkerBridge<BackendWorker>,
}

impl FeedsHandle {
    pub fn handle_fetch_feeds(&self) {
        let event = Event::AllFeeds(());
        let data = serde_json::to_vec(&event).unwrap();
        self.bridge.send(WorkerRequest::BackendRequest(data));
    }

    pub fn handle_flip_enabled(&self, id: FeedID) {

        //
    }
}

fn handle_action(action: FeedsAction, atom_feeds: UseAtomRef<DisplayedFeeds>) {
    match action {
        FeedsAction::Replace(mut new_feeds) => {
            let mut feeds = atom_feeds.write();
            feeds.feeds.clear();
            while let Some(feed) = new_feeds.pop() {
                feeds.feeds.push(DisplayedFeed {
                    stored: feed,
                    enabled: true,
                });
            }
        }
        FeedsAction::FlipEnabled(id) => {
            let mut feeds = atom_feeds.write();
        }
        FeedsAction::BackendResponse(data) => {
            if let Some(data) = data {
                warn!("worker feeds response: {}", String::from_utf8_lossy(&data));
                let response = serde_json::from_slice(&data).expect("cannot deserialize response");

                warn!("worker feeds response deserialized: {:?}", response);
                match response {
                    Response::AllFeeds(feeds) => {
                        handle_action(FeedsAction::Replace(feeds), atom_feeds.clone())
                    }
                    _ => {
                        warn!("bad response from backend: {:?}", response);
                    }
                }
            } else {
                warn!("empty response from worker");
            }
        }
    }
}

/*
 * This is an idea of a generic handler to avoid copy-pasting it for each component state

trait BackendEvent {
    fn wrap_backend_response(data: Option<Vec<u8>>) -> Self;
}

trait Handle<A, T> {
    fn handle_action(action: A, atom: UseAtomRef<T>);
}

pub fn init_event_actor<E: BackendEvent, H: Handle<E, AtomT>, AtomT>(
    cx: &Scope,
    state: AtomRef<AtomT>,
) {
    let atom = use_atom_ref(&cx, state).clone();
    let actor = use_coroutine(&cx, |mut rx: UnboundedReceiver<E>| async move {
        while let Some(action) = rx.next().await {
            //handle_action(action, atom.clone());
            H::handle_action(action, atom.clone());
        }
    })
    .to_owned();

    let bridge = use_read(cx, WORKER_BRIDGE);

    let b_actor = actor.clone();
    let bridge = bridge.borrow().fork(Some(move |response| {
        // TODO move callback to a separate function
        if let WorkerResponse::BackendResponse(data) = response {
            // TODO: process data.is_none()
            // TODO: process response deserialize error
            // the callback means "when worker responds with the message"
            let converted = E::wrap_backend_response(data);
            //b_actor.send(FeedsAction::BackendResponse(data))
            b_actor.send(converted)
        } else {
            // TODO: send worker status change message
            todo!();
        }
    }));

    let handle = FeedsHandle {
        actor: actor.to_owned(),
        bridge,
    };
    use_context_provider(cx, move || handle);
}
*/

pub fn init_feeds_actor(cx: &Scope) {
    let feeds = use_atom_ref(&cx, FEEDS).clone();
    let actor = use_coroutine(&cx, |mut rx: UnboundedReceiver<FeedsAction>| async move {
        while let Some(action) = rx.next().await {
            handle_action(action, feeds.clone());
        }
    })
    .to_owned();

    let bridge = use_read(cx, WORKER_BRIDGE);

    let b_actor = actor.clone();
    let bridge = bridge.borrow().fork(Some(move |response| {
        // TODO move callback to a separate function
        if let WorkerResponse::BackendResponse(data) = response {
            // TODO: process data.is_none()
            // TODO: process response deserialize error
            // the callback means "when worker responds with the message"
            b_actor.send(FeedsAction::BackendResponse(data))
        } else {
            // TODO: send worker status change message
            todo!();
        }
    }));

    let handle = FeedsHandle {
        actor: actor.to_owned(),
        bridge,
    };
    use_context_provider(cx, move || handle);
}

#[allow(non_snake_case)]
pub fn Feeds(cx: Scope) -> Element {
    let feeds = use_atom_ref(&cx, FEEDS);
    let feeds_handle = use_context::<FeedsHandle>(&cx).unwrap().to_owned();
    let feeds: &DisplayedFeeds = &feeds.read();

    let feed_nodes = feeds.feeds.iter().map(|feed| {
        let title = feed.stored.title.clone().unwrap_or_default();

        let id = feed.stored.feed_id.clone();
        let title = feed.stored.title.clone().unwrap_or("untitled".to_string());
        //let url = feed.stored.url.clone();
        let key = id.0.clone();
        let enabled = if feed.enabled { "x" } else { "v" };

        rsx!(
        p {
            key: "{key}",
            div {
                /*a {
                    target: "_blank",
                    href: "{link}",
                    rel: "noopener noreferrer",
                    "{title}"
                } */
                div {
                    "{title}"
                    button {
                        //disabled: "{read}",
                        onclick: move |_| {
                            let feeds = use_atom_ref(&cx, FEEDS).clone();
                            let mut feeds = feeds.write();
                            if let Some(pos) = feeds.feeds.iter().position(|f| f.stored.feed_id == id) {
                                feeds.feeds.get_mut(pos).unwrap().enabled = !feeds.feeds.get(pos).unwrap().enabled;
                            }
                        },
                        "{enabled}"
                    }
                }
            }
        }
        )
    });
    cx.render(rsx! {
       feed_nodes
    })
}

//pub fn handle_refresh(cx: &Scope) {
//let login_data = use_read(cx, LOGIN_SETTINGS);
//let login_data: LoginSettings = LoginSettings {
//creds: AwsCreds {
//key_id: login_data.key_id.clone(),
//access_key: login_data.access_key.clone(),
//},
//show_settings: login_data.show_settings.clone(),
//};
//let handle = use_context::<EntriesHandle>(cx).unwrap().read().clone();
//cx.spawn(async move {
//let event = Event::RefreshFeeds(RefreshFeeds { force: true });
//let response = sign_and_send(event, &login_data.clone())
//.await
//.map_err(|e| warn!("refresh failed: {}", e))
//.map(|_| ());
//if response.is_ok() {
//handle.fetch_entries(login_data, None).await;
//}
//debug!("{:?}", response);
//});
//}

//pub fn fetch_feeds(cx: Scope) {
//let login_data = use_context::<LoginDataOld>(cx);
//let feeds = use_context::<RcSignal<Vec<ShownFeed>>>(cx);
////Ok(Vec::new());
//sycamore::futures::spawn_local_scoped(cx, async move {
////fetch_feeds().await
//let event = Event::AllFeeds(());
//let response = sign_and_send(event, login_data.clone()).await.unwrap();
//if let Response::AllFeeds(mut res_feeds) = response {
//let mut feeds = feeds.modify();
//feeds.clear();
//while let Some(feed) = res_feeds.pop() {
//feeds.push(ShownFeed { stored: feed })
//}
//} else {
//warn!("bad response for AllEntries: {:?}", response);
//}
//});
//}

//#[component]
//fn SingleFeed<G: Html>(cx: Scope, feed: ShownFeed) -> View<G> {
//let url = feed.stored.url.clone();
//let title = feed.stored.title.clone().unwrap_or(url);
//view! { cx,
//div {
//(title)

//}
//}
//}

//#[component]
//pub fn Feeds<G: Html>(cx: Scope) -> View<G> {
//let feeds = create_rc_signal::<Vec<ShownFeed>>(Vec::new());
//provide_context(cx, feeds);

//let feeds = use_context::<RcSignal<Vec<ShownFeed>>>(cx);
//view! { cx,
//div {
//Indexed {
//iterable: feeds,
//view: |cx, x: ShownFeed| view! { cx,
//p { SingleFeed(x.clone()) }
//}
//}
//}
//}
//}

//#[derive(Clone, Eq, PartialEq)]
//pub struct AddFeedData {
//feed_url: RcSignal<String>,
//pub hidden: RcSignal<bool>,
//}

//impl Default for AddFeedData {
//fn default() -> Self {
//Self {
//feed_url: create_rc_signal(String::new()),
//hidden: create_rc_signal(true),
//}
//}
//}

//#[component]
//pub fn AddFeedForm<G: Html>(cx: Scope) -> View<G> {
//let add_feed_data = use_context::<AddFeedData>(cx);

//let handle_submit = move |_| {
//sycamore::futures::spawn_local_scoped(cx, async move {
//let add_feed_data = use_context::<AddFeedData>(cx);
//let login_data = use_context::<LoginDataOld>(cx);

//let event = Event::AddFeed(add_feed_data.feed_url.get().to_string());
//let response = sign_and_send(event, login_data.clone()).await.unwrap();
//warn!("add feed: {:?}", response);
//fetch_feeds(cx);
//});
//};
//let style_hidden = create_memo(cx, || {
//if *add_feed_data.hidden.get() {
//"display: none"
//} else {
//"display: block"
//}
//});
//view! { cx,
//div(style=style_hidden) {
//input(placeholder="Feed URL", bind:value=add_feed_data.feed_url)
//br
//button(on:click=handle_submit) { "Add" }
//}
//}
//}
