use std::rc::Rc;

use anyhow::{anyhow, Error};
use base64ct::Base64;
use base64ct::Encoding;
use log::{info, warn};
use serde::{Deserialize, Serialize};

use gloo_worker::{HandlerId, Worker, WorkerScope};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::credentials::AwsCreds;
use crate::credentials::AwsCredsEncrypted;
use crate::crypto::KeyData;
use crate::util::global_scope;

#[derive(Debug, Serialize, Deserialize)]
pub enum WorkerRequest {
    BackendRequest(Vec<u8>),
    StatusRequest,
    SetCredsEncrypted(String, Vec<u8>),
    SetCredsPlaintext(String, String, String, String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WorkerResponse {
    BackendResponse(Option<Vec<u8>>),
    Status(WorkerStatus),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WorkerStatus {
    NotReady,
    CredsEncrypted(Vec<u8>),
    Ready,
}

impl Default for WorkerStatus {
    fn default() -> Self {
        WorkerStatus::NotReady
    }
}

enum WorkerState {
    Uninitialized,
    Ready(KeyData, AwsCreds),
}

pub struct BackendWorker {
    state: WorkerState,
    next_slot: u64,
    queue: im::Vector<(u64, Option<Rc<WorkerResponse>>)>,
}

impl Worker for BackendWorker {
    type Input = WorkerRequest;
    type Message = (HandlerId, u64, Option<Vec<u8>>);
    type Output = WorkerResponse;

    fn create(scope: &WorkerScope<Self>) -> Self {
        /*
        scope.send_future(async move {
            if let Ok(maybe_encrypted) = crate::credentials::load_indexed_db_blob()
                .await
                .map_err(|e| warn!("error loading credentials: {}", e))
            {
                if let Some(encrypted) = maybe_encrypted {
                    let decoded = Base64::decode_vec(&encrypted).expect("decoding base64");
                }
            }
        });
        */

        Self {
            // worker always starts initialized and will require the password
            // loading any data is useless at this stage
            state: WorkerState::Uninitialized,
            next_slot: 0,
            queue: im::Vector::new(),
        }
    }

    fn update(&mut self, scope: &WorkerScope<Self>, msg: Self::Message) {
        // We want to keep the order of incoming requests, but since they could
        // only be performed in background futures (i.e. in parallel)
        // we put them back to the queue into their corresponding slot
        // and delay until the foremost request is ready
        let (who, slot, response) = msg;
        let pos = self
            .queue
            .binary_search_by_key(&slot, |elem| elem.0)
            .expect("internal error while queueing responses");
        self.queue[pos] = (
            slot,
            Some(Rc::new(WorkerResponse::BackendResponse(response))),
        );
        loop {
            let mut pop = false;
            if let Some(front_element) = self.queue.front() {
                if front_element.1.is_some() {
                    pop = true;
                }
            }
            if pop {
                let response = self.queue.pop_front().unwrap().1.unwrap();
                let response = Rc::try_unwrap(response).unwrap();
                scope.respond(who, response);
            } else {
                break;
            }
        }
    }

    fn received(&mut self, scope: &WorkerScope<Self>, msg: Self::Input, who: HandlerId) {
        warn!("worker got message from {:?}: {:?}", who, msg);
        let response = match msg {
            WorkerRequest::BackendRequest(event) => {
                if let WorkerState::Ready(_, creds) = &self.state {
                    let slot = self.next_slot;
                    self.next_slot += 1;
                    self.queue.push_back((slot, None));

                    let creds = creds.clone();
                    let future = async move {
                        let response = sign_and_send_data(event, creds)
                            .await
                            .map_err(|e| warn!("worker error on sending backend request: {}", e))
                            .ok();
                        (who, slot, response)
                    };
                    scope.send_future(future);
                    None
                } else {
                    Some(self.get_status())
                }
            }
            WorkerRequest::StatusRequest => Some(self.get_status()),
            WorkerRequest::SetCredsEncrypted(p, creds) => Some(self.set_password(p, creds)),
            WorkerRequest::SetCredsPlaintext(password, host, id, access) => {
                if let Some(data) = self.set_plaintext(password, host, id, access) {
                    let b64data = Base64::encode_string(&data);
                    let inscope = scope.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let _ = crate::credentials::save_indexed_db_blob(b64data)
                            .await
                            .map_err(|e| {
                                warn!("error saving to indexed db: {}", e);
                            });
                        inscope.respond(
                            who,
                            WorkerResponse::Status(WorkerStatus::CredsEncrypted(data)),
                        );
                    });
                    None
                } else {
                    Some(WorkerResponse::Status(WorkerStatus::NotReady))
                }
            }
        };

        warn!("worker sending response {:?}", response);
        if let Some(response) = response {
            scope.respond(who, response)
        }

        warn!("worker response sent");
    }
}

impl BackendWorker {
    pub(crate) fn get_status(&self) -> WorkerResponse {
        match &self.state {
            WorkerState::Uninitialized => WorkerResponse::Status(WorkerStatus::Ready),
            WorkerState::Ready(_, _) => WorkerResponse::Status(WorkerStatus::NotReady),
        }
    }

    pub(crate) fn set_password(&mut self, password: String, enc_creds: Vec<u8>) -> WorkerResponse {
        let encrypted: AwsCredsEncrypted =
            serde_json::from_slice(&enc_creds).expect("deserializing encrypted creds");
        let mut new_kd = KeyData::new_with_salt(&encrypted.salt).expect("creating kd from enc");
        new_kd.fill_from_password(&password);

        match encrypted.try_decrypt(&new_kd) {
            Ok(plaintext_creds) => {
                self.state = WorkerState::Ready(new_kd, plaintext_creds);
                WorkerResponse::Status(WorkerStatus::Ready)
            }
            Err(e) => {
                warn!("error decrypting creds : {:?}", e);
                WorkerResponse::Status(WorkerStatus::NotReady)
            }
        }
    }

    pub(crate) fn set_plaintext(
        &mut self,
        password: String,
        lambda_host: String,
        key_id: String,
        access_key: String,
    ) -> Option<Vec<u8>> {
        let mut kd = KeyData::new();
        kd.fill_from_password(&password);
        match AwsCreds::create_and_save(&kd, lambda_host, key_id, access_key) {
            Ok(creds) => {
                let encrypted = creds.try_encrypt(&kd).expect("encrypting creds");
                let data = serde_json::to_vec(&encrypted).expect("serializing creds");

                self.state = WorkerState::Ready(kd, creds);
                Some(data)
            }
            Err(e) => {
                warn!("error encrypting aws creds: {}", e);
                None
            }
        }
    }

    //pub(crate) fn set_aws(
    //&mut self,
    //lambda_host: String,
    //key_id: String,
    //access_key: String,
    //) -> WorkerResponse {
    //let (new_key_data, aws_creds) = match &mut self.state {
    //WorkerState::Uninitialized => return WorkerStatus::NoPassword,
    //WorkerState::HasKey(key_data) | WorkerState::Ready(key_data, _) => {
    //let aws_creds =
    //match AwsCreds::create_and_save(key_data, lambda_host, key_id, access_key) {
    //Ok(creds) => creds,
    //Err(e) => {
    //warn!("error saving/encrypting aws creds: {}", e);
    //return WorkerStatus::NoCreds;
    //}
    //};
    //(KeyData::new_from(key_data), aws_creds)
    //}
    //};

    //self.state = WorkerState::Ready(new_key_data, aws_creds);
    //WorkerStatus::Ready
    //}
}

async fn sign_and_send_data(body: Vec<u8>, creds: AwsCreds) -> Result<Vec<u8>, Error> {
    //let login_data = crate::aws::LoginSettings {
    //// we don't want to clone an encryption key
    //key_id: creds.key_id,
    //access_key: creds.access_key,
    //show_settings: false,
    //};

    //let response = request.send().await?;
    let request = crate::aws::new_signed_request(&body, &creds)?;
    info!("sending request: {:?}", request);
    info!("body: {:?}", String::from_utf8_lossy(&body));
    let promise = global_scope().fetch_with_request(&request);
    let response = JsFuture::from(promise)
        .await
        .map_err(|e| anyhow!("making request: {:?}", e))?;
    //match response.dyn_into::<web_sys::Response>() {
    //Ok(response) => Ok(Response {
    //response: response.unchecked_into(),
    //}),
    //Err(e) => panic!("fetch returned {:?}, not `Response` - this is a bug", e),
    //}

    let response = gloo_net::http::Response::from_raw(response.unchecked_into());
    let resp_body = response.binary().await?;

    info!(
        "RESP: {:?} -> {:?}",
        response,
        String::from_utf8_lossy(&resp_body)
    );

    Ok(resp_body)
}
