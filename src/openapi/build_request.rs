use cookie::Cookie;
use openapiv3::OpenAPI;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::input::{parameter::ParameterKind, OpenApiRequest};

/// Build a request to a path from the API using the input values.
pub fn build_request_from_input(
    client: &reqwest::blocking::Client,
    cookie_store: &std::sync::Arc<reqwest_cookie_store::CookieStoreMutex>,
    api: &OpenAPI,
    input: &OpenApiRequest,
) -> Option<reqwest::blocking::RequestBuilder> {
    let server = &api
        .servers.first()
        .expect("API specification contains no usable servers. If you did specify any, consult logs for attempts to connect to them.");
    let mut path = server.url.to_owned() + &input.path;
    let mut header_params = HeaderMap::new();
    header_params.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static("application/json"),
    );
    let mut query_params = Vec::new();
    let mut cookie_params = Vec::new();
    for ((name, kind), value) in input // voor elke parameter in openapirequest
        .parameters
        .iter()
    {
        match kind {
            ParameterKind::Query => query_params.push((name, value.to_url_encoding())),
            ParameterKind::Header => {
                if let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) {
                    header_params.insert(header_name, value.to_header_value());
                }
            }
            ParameterKind::Path => {
                let search_term = format!("{{{name}}}");
                if let Some(offset) = path.find(&search_term) {
                    path.replace_range(
                        offset..(offset + search_term.len()),
                        &value.to_url_encoding(),
                    )
                }
            }
            ParameterKind::Cookie => cookie_params.push(Cookie::new(name, value.to_cookie_value())),
            ParameterKind::Body => unimplemented!("Body parameters are not supposed to occur here"),
        }
    }

    // Deserialize the path into a Url
    let path_with_query_params =
        reqwest::Url::parse_with_params(&path, query_params).expect("Invalid URL");

    // Add any collected cookie parameters to the cookie store
    {
        let mut cookie_store = cookie_store.lock().unwrap();
        let bare_url = reqwest::Url::parse(&path).expect("Invalid URL");
        for cookie in cookie_params {
            let _ = cookie_store.insert_raw(&cookie, &bare_url);
        }
    } // Release the cookie_store lock

    let mut builder = client
        .request(input.method.into(), path_with_query_params)
        .headers(header_params);
    if let Some(contents) = input.reqwest_body() {
        builder = builder
            .body(contents)
            .header(reqwest::header::CONTENT_TYPE, input.body_content_type());
    }
    Some(builder)
}
