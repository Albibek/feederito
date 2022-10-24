mod aws;
mod credentials;
mod crypto;
mod util;
mod worker;

use log::warn;

use crate::worker::BackendWorker;
use gloo_worker::Registrable;

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).unwrap();

    BackendWorker::registrar().register();
}
