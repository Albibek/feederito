use js_sys::Reflect;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::WorkerGlobalScope;

pub(crate) fn global_scope() -> WorkerGlobalScope {
    let global = js_sys::global();
    let maybe_worker = Reflect::get(&global, &JsValue::from_str("WorkerGlobalScope")).unwrap();
    if !maybe_worker.is_undefined() {
        let worker = global.dyn_into::<web_sys::WorkerGlobalScope>().unwrap();
        worker
    } else {
        panic!("worker scope is undefined");
    }
}
