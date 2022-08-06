#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

mod aws;
mod entries;
mod feeds;
//mod util;

use std::ops::Deref;

//use sycamore::prelude::*;
use dioxus::fermi::*;
use dioxus::prelude::*;
use worker::WorkerStatus;

use crate::aws::*;
use crate::entries::*;

//use crate::util::*;

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).unwrap();

    dioxus::web::launch(app);
}

fn app(cx: Scope) -> Element {
    init_worker_state_actor(&cx);
    init_entries_actor(&cx);

    let entries_handle = use_context::<EntriesHandle>(&cx).unwrap().to_owned();

    let login_settings = use_atom_state(&cx, LOGIN_SETTINGS);
    let worker_status = use_read(&cx, WORKER_STATUS);
    let button_style = format_args!("f6 link dim br3 ba ph3 pv2 mb2 dib dark-blue bg-white");

    if let &WorkerStatus::Ready = worker_status.borrow().deref() {
        cx.render(rsx!(
            div {
                // main container, includes login form
                class: "fl w-100 pa2",

                AwsSettingsForm {},
                div {
                    // a row with buttons
                    class: "fl w-100",
                    //button(on:click=move |_| fetch_feeds(ctx)) {"Feeds"}
                    button {
                        class: button_style,
                        onclick: move |_| entries_handle.read().handle_fetch_entries(),  "Entries" }
                    button {
                        class: button_style,
                        onclick: move |_| entries_handle.read().handle_refresh(),  "Refresh" }

                    //button {
                        //class: "f6 link dim br3 ba ph3 pv2 mb2 dib dark-blue",
                        //onclick: move |_| entries_handle.read().handle_undo(&cx),  "Undo" }
                    button {
                        class: button_style,
                        onclick: move |_| {
                            login_settings.with_mut(|prev| {
                               prev.show_settings = !prev.show_settings;
                           });
                    },
                        "Settings"
                    },
                }
                div {
                    // a row with main panel: feeds and entries
                    class: "fl h-90 w-100",
                    //style: "height: 90vh",
                    div {
                        class: "fl w-20",
                        //style: "height: 80vh",
                        nav {
                            "TODO feeds"
                        //Feeds {}
                        }
                    }
                    div {
                        class: "fl w-60",
                        style: "overflow: scroll; height: 80vh",

                        main { Entries {} }
                    }
                    div {
                        class: "fl w-20",
                        style: "overflow: scroll; height: 80vh",

                        main { ReadEntries {} }
                    }
                }
            }
        ))
    } else {
        cx.render(rsx!(AwsSettingsForm {},))
    }
}
