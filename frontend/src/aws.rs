use std::ops::DerefMut;

use anyhow::{anyhow, Error};
use futures::StreamExt;
use log::info;

use log::warn;
use web_sys::HtmlInputElement;

use dioxus::fermi::hooks::*;
use dioxus::fermi::Writable;
use dioxus::prelude::*;
use gloo_worker::{Spawnable, WorkerBridge};

use model::events::{Event, *};
use worker::AwsCredsEncrypted;
use worker::{AwsCreds, BackendWorker, WorkerRequest, WorkerResponse, WorkerStatus};

// These are here temporarily to simplify debugging (the values will already be in the form)
// TODO: remove values
static FUNCTION_HOST: &'static str = include_str!("../ids/lambda_url");
static AWS_KEY_ID: &'static str = include_str!("../ids/aws_key_id");

pub static LOGIN_SETTINGS: Atom<LoginSettings> = |_| LoginSettings::default();
pub static WORKER_STATUS: AtomRef<WorkerStatus> = |_| WorkerStatus::default();

pub static WORKER_BRIDGE: AtomRef<WorkerBridge<BackendWorker>> = |_| {
    BackendWorker::spawner()
        .callback(move |m| {
            warn!("hi from worker bridge");
        })
        .spawn("/worker.js")
};

#[derive(Clone)]
pub struct LoginSettings {
    pub creds: AwsCreds,
    pub password: String,
    pub show_settings: bool,
    pub show_password: bool,
}

impl Default for LoginSettings {
    fn default() -> Self {
        Self {
            creds: AwsCreds {
                lambda_host: String::from(FUNCTION_HOST),
                key_id: String::from(AWS_KEY_ID),
                access_key: String::new(),
            },
            password: String::new(),
            show_settings: true,
            show_password: true,
        }
    }
}

/// The entries processor coroutine, handles all the signals related to feed entries processing
#[derive(Clone)]
pub struct WorkerStatusHandle {
    actor: CoroutineHandle<WorkerStatus>,
    bridge: WorkerBridge<BackendWorker>,
}

pub fn init_worker_state_actor(cx: &Scope) {
    let worker_state = use_atom_ref(&cx, WORKER_STATUS).clone();
    let actor = use_coroutine(&cx, |mut rx: UnboundedReceiver<WorkerStatus>| async move {
        while let Some(status) = rx.next().await {
            if let WorkerStatus::CredsEncrypted(creds) = &status {
                AwsCredsEncrypted::save_local_storage_blob(&creds);

                *DerefMut::deref_mut(&mut worker_state.write()) = WorkerStatus::Ready;
            } else {
                *DerefMut::deref_mut(&mut worker_state.write()) = status;
            }
        }
    })
    .to_owned();

    let bridge = use_read(cx, WORKER_BRIDGE);

    let b_actor = actor.clone();
    let bridge = bridge.borrow().fork(Some(move |response| {
        warn!("worker bridge got response {:?}", response);
        if let WorkerResponse::Status(status) = response {
            b_actor.send(status)
        } else {
            warn!("unexpected response from worker: {:?}", response);
        }
    }));

    let handle = WorkerStatusHandle {
        actor: actor.to_owned(),
        bridge,
    };
    use_context_provider(cx, move || handle);
}

#[allow(non_snake_case)]
pub fn AwsSettingsForm(cx: Scope) -> Element {
    let worker_handle = use_context::<WorkerStatusHandle>(&cx).unwrap().to_owned();
    let login_settings = use_atom_state(&cx, LOGIN_SETTINGS);

    let prev = login_settings.current();
    let hidden_style = if prev.show_settings {
        "display: block"
    } else {
        "display: none"
    };

    cx.render(rsx! {
        div { style: "{ hidden_style }",
        input {
            placeholder: "Lambda hostname",
            value: "{prev.creds.lambda_host}",
            oninput: move |ev|  {
                login_settings.with_mut(|prev| {
                    prev.creds.lambda_host = ev.value.clone();
                });
            }
        }
        br {},
        input {
            placeholder: "AWS key ID",
            value: "{prev.creds.key_id}",
            oninput: move |ev|  {
                login_settings.with_mut(|prev| {
                    prev.creds.key_id = ev.value.clone();
                });
            }
        }
        br {},
        input {
            placeholder: "AWS secret",
            value: "{prev.creds.access_key}",
            oninput: move |ev|  {
                login_settings.with_mut(|prev| {
                    prev.creds.access_key = ev.value.clone();
                });
            }
        }
        br {},
        button {
            onclick: move |ev|  {
                //login_settings.with_mut(|prev| {
                    let password  = login_settings.password.clone();
                    let creds  = login_settings.creds.clone();
                    worker_handle.read().bridge.send(WorkerRequest::SetCredsPlaintext(password, creds.lambda_host, creds.key_id, creds.access_key));
                    //prev.creds.access_key = ev.value.clone();
                //});
            },
            "OK"
        }
        br {},
        input {
            placeholder: "Password",
            value: "{prev.password}",
            oninput: move |ev|  {
                login_settings.with_mut(|prev| {
                    prev.password = ev.value.clone();
                });
            }
        }
        br {},
        button {
            onclick: move |ev|  {
                login_settings.with_mut(|prev| {
                    let enc_creds = AwsCredsEncrypted::load_local_storage_blob();
                    if let Some(creds) = enc_creds {
                       worker_handle.read().bridge.send(WorkerRequest::SetCredsEncrypted(prev.password.clone(), creds));
                    } else {
                       let password  = prev.password.clone();
                       let creds  = prev.creds.clone();
                       worker_handle.read().bridge.send(
                           WorkerRequest::SetCredsPlaintext(
                               password,
                               creds.lambda_host,
                               creds.key_id,
                               creds.access_key
                               )
                           );
                    }
                });
            },
            "OK"
        }
        }
    })
}
