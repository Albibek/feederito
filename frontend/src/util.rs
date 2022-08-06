use log::warn;
use sycamore::prelude::*;

use model::events::*;

use crate::aws::*;

//pub fn fix_database(cx: Scope) {
//let login_data = use_context::<LoginDataOld>(cx);
//sycamore::futures::spawn_local_scoped(cx, async {
//let event = Event::FixDatabase(());
//let response = sign_and_send(event, login_data.clone()).await.unwrap();
//warn!("response for FixDatabase: {:?}", response);
//});
//}
