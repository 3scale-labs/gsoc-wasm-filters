use crate::filter::http::CacheFilter;
use crate::info;
use proxy_wasm::{
    hostcalls::{resume_http_request, send_http_response},
    traits::HttpContext,
    types::Action,
};
use threescale::structs::{AppIdentifier, ThreescaleData};
use threescalers::{
    api_call::{ApiCall, Kind},
    application::Application,
    credentials::*,
    extensions::{self},
    http::Request,
    service::Service,
    transaction::Transaction,
    usage::Usage,
};

// Helper function to handle failure when request headers are recieved
pub fn in_request_failure(filter: &CacheFilter) -> Action {
    if filter.config.failure_mode_deny {
        if cfg!(feature = "visible_logs") {
            let (key, val) = crate::log::visible_logs::get_logs_header_pair(filter.context_id);
            send_http_response(
                403,
                vec![(key.as_ref(), val.as_ref())],
                Some(b"Access forbidden.\n"),
            )
            .unwrap(); // Safe for current implementation.
        } else {
            send_http_response(403, vec![], Some(b"Access forbidden.\n")).unwrap();
        }
        return Action::Pause;
    }
    Action::Continue
}

// Helper function to handle failure during processing
pub fn request_process_failure(filter: &CacheFilter) {
    if filter.config.failure_mode_deny {
        if cfg!(feature = "visible_logs") {
            let (key, val) = crate::log::visible_logs::get_logs_header_pair(filter.context_id);
            send_http_response(
                403,
                vec![(key.as_ref(), val.as_ref())],
                Some(b"Access forbidden.\n"),
            )
            .unwrap(); //
        } else {
            send_http_response(403, vec![], Some(b"Access forbidden.\n")).unwrap();
        }
    }
    resume_http_request().unwrap();
}

pub fn do_auth_call<C: HttpContext>(
    ctx: &C,
    filter: &CacheFilter,
    request_data: &ThreescaleData,
) -> Action {
    let cred = Credentials::ServiceToken(ServiceToken::from(request_data.service_token.as_ref()));
    let service = Service::new(request_data.service_id.as_ref(), cred);

    let app = match &request_data.app_id {
        AppIdentifier::UserKey(user_key) => Application::from_user_key(user_key.as_ref()),
        AppIdentifier::AppId(app_id, None) => Application::from_app_id(app_id.as_ref()),
        AppIdentifier::AppId(app_id, Some(app_key)) => {
            Application::from_app_id_and_key(app_id.as_ref(), app_key.as_ref())
        }
    };

    let mut metrics = Vec::new();
    for (metric, hits) in request_data.metrics.borrow().iter() {
        metrics.push((metric.clone(), hits.to_string().clone()));
    }

    let usage = Usage::new(metrics.as_slice());
    let txn = vec![Transaction::new(&app, None, Some(&usage), None)];

    let extensions = extensions::List::new()
        .push(extensions::Extension::Hierarchy)
        .push(extensions::Extension::ListAppKeys("1".into()));

    let mut apicall = ApiCall::builder(&service);
    let apicall = match apicall
        .transactions(&txn)
        .extensions(&extensions)
        .kind(Kind::Authorize)
        .build()
    {
        Ok(result) => result,
        Err(e) => {
            info!(filter.context_id, "couldn't contact 3scale: {}", e);
            return in_request_failure(filter);
        }
    };

    let request = Request::from(&apicall);
    let (uri, body) = request.uri_and_body();
    let headers = request
        .headers
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();

    info!(filter.context_id, "App : {:?}", apicall);
    match request_data.upstream.call(
        ctx,
        uri.as_ref(),
        request.method.as_str(),
        headers,
        body.map(str::as_bytes),
        None,
        None,
    ) {
        Ok(token) => info!(
            filter.context_id,
            "dispatch successful with token: {}", token
        ),
        Err(e) => {
            info!(filter.context_id, "couldn't contact 3scale: {:?}", e);
            return in_request_failure(filter);
        }
    };
    // pause the current request to wait for the response from 3scale
    Action::Pause
}
