use anyhow::{anyhow, Error};
use log::info;

use gloo_net::http::Response;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::Request;

use crate::credentials::AwsCreds;
use crate::util::global_scope;

const AWS_REQUEST_VERSION: &'static str = "aws4_request";
// TODO: variable region and service
const AWS_REGION: &'static str = "eu-west-1";
const AWS_SERVICE: &'static str = "lambda";

pub async fn sign_and_send_data(body: Vec<u8>, creds: AwsCreds) -> Result<Vec<u8>, Error> {
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

    let response = Response::from_raw(response.unchecked_into());
    let resp_body = response.binary().await?;

    info!(
        "RESP: {:?} -> {:?}",
        response,
        String::from_utf8_lossy(&resp_body)
    );

    Ok(resp_body)
}

pub fn new_signed_request(body: &[u8], creds: &AwsCreds) -> Result<Request, Error> {
    let function_url = "https://".to_owned() + &creds.lambda_host;

    //YYYYMMDD'T'HHMMSS'Z'.
    let time = js_sys::Date::new_0();

    let time_str: String = format!(
        "{}{:02}{:02}T{:02}{:02}{:02}Z",
        time.get_utc_full_year(),
        time.get_utc_month() + 1, // Js counts months from zero
        time.get_utc_date(),      // get_day returns day of week
        time.get_utc_hours(),
        time.get_utc_minutes(),
        time.get_utc_seconds() //39u32,
                               //00u32,
    );
    let headers = gloo_net::http::Headers::new();

    headers.set("Host", &creds.lambda_host);
    headers.set("Content-type", "application/json");
    headers.set("X-amz-date", &time_str);

    let mut canonical_request = "POST".to_owned() + "\n" +  // method
        //"https://" + FUNCTION_HOST + "\n" +
        "/" + "\n" + // Canonical URI
        "" + "\n"; // CanonicalQueryString
    let mut canonical_headers = Vec::new();
    let mut signed_headers = Vec::new();
    for (k, v) in headers.entries() {
        let parts = v.split_whitespace().collect::<Vec<_>>();
        let value = parts.join(" ");

        let header = k.to_lowercase().to_owned();
        canonical_headers.push(header.clone() + ":" + &value);
        signed_headers.push(header);
    }
    canonical_headers.sort();
    signed_headers.sort();
    let canonical_headers = canonical_headers.as_slice().join("\n");
    let signed_headers = signed_headers.as_slice().join(";");
    canonical_request = canonical_request + &canonical_headers + "\n\n";
    canonical_request = canonical_request + &signed_headers + "\n";

    let body_hash = sha256(&body);

    canonical_request = canonical_request + &body_hash;
    let canonical_hash = sha256(canonical_request.as_bytes());

    let date_stamp = format!(
        "{}{:02}{:02}",
        time.get_utc_full_year(),
        time.get_utc_month() + 1, // months are counted from zero in Js
        time.get_utc_date()       // get_day returns day of week in Js
    );

    let scope = format!(
        "{}/{}/{}/{}",
        date_stamp, AWS_REGION, AWS_SERVICE, AWS_REQUEST_VERSION
    );
    let string_to_sign = "AWS4-HMAC-SHA256".to_owned() + "\n" + // Algorithm
        &time_str + "\n" + // RequestDateTime
        &scope + "\n" + // Scope
        &canonical_hash;

    let access_key = "AWS4".to_owned() + &creds.access_key;
    let k_date = hmac(access_key.as_bytes(), date_stamp.as_bytes())?;
    let k_region = hmac(&k_date, AWS_REGION.as_bytes())?;
    let k_service = hmac(&k_region, AWS_SERVICE.as_bytes())?;
    let k_signing = hmac(&k_service, AWS_REQUEST_VERSION.as_bytes())?;
    let mut ks = String::new();
    for byte in &k_signing {
        ks += &format!("{:02x}", byte);
    }

    //info!("kSigning: {:?}", ks);

    let signature = hmac(&k_signing, string_to_sign.as_bytes())?;
    let mut hex_signed = String::new();
    for byte in signature {
        hex_signed += &format!("{:02x}", byte);
    }
    //info!("REQ: {:?}", canonical_request);
    //info!("to sign: {:?}", string_to_sign);
    //info!("signed: {:?}", hex_signed);

    let auth_header = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        &creds.key_id, &scope, &signed_headers, &hex_signed
    );

    headers.set("Authorization", &auth_header);
    let js_body = js_sys::Uint8Array::new_with_length(body.len() as u32);
    js_body.copy_from(&body);

    let mut options = web_sys::RequestInit::new();
    options.headers(&headers.into_raw());
    options.method("POST");
    options.body(Some(&js_body));
    // TODO CORS
    let request =
        web_sys::Request::new_with_str_and_init(&function_url, &options).expect("creating request");

    Ok(request)
}

fn sha256(input: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input);
    let mut result = String::new();
    let res = hasher.finalize();
    for byte in res.as_slice() {
        result += &format!("{:02x}", byte);
    }
    result
}

fn hmac(key: &[u8], input: &[u8]) -> Result<Vec<u8>, Error> {
    use hmac::{Hmac, Mac as _};
    use sha2::Sha256;

    let mut mac = Hmac::<Sha256>::new_from_slice(key)?;
    mac.update(input);
    let result = mac.finalize();
    Ok(result.into_bytes().to_vec())
}
