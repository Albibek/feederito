use std::rc::Rc;

use dioxus::fermi::*;
use dioxus::prelude::*;
use log::{debug, warn};

use model::events::*;
use model::feed::*;

use crate::aws::*;
use crate::entries::*;
use crate::WORKER_BRIDGE;
use worker::AwsCreds;

#[derive(PartialEq, Eq, Clone)]
pub struct ShownFeed {
    pub stored: StoredFeed,
}

pub struct Feeds {
    feeds: Vec<ShownFeed>,
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
