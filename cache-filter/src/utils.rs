use crate::filter::http::CacheFilter;
use log::info;
use proxy_wasm::types::Action;
use threescale::structs::{Period, ThreescaleData};
use threescalers::{credentials::*, response::Period as ResponsePeriod, service::Service};

// Helper function to handle failure when request headers are recieved
pub fn in_request_failure<C: proxy_wasm::traits::HttpContext>(
    ctx: &C,
    filter: &CacheFilter,
) -> Action {
    if filter.config.failure_mode_deny {
        ctx.send_http_response(403, vec![], Some(b"Access forbidden.\n"));
        return Action::Pause;
    }
    Action::Continue
}

// Helper function to handle failure during processing
pub fn request_process_failure<C: proxy_wasm::traits::HttpContext>(ctx: &C, filter: &CacheFilter) {
    if filter.config.failure_mode_deny {
        ctx.send_http_response(403, vec![], Some(b"Access forbidden.\n"));
    }
    ctx.resume_http_request();
}

pub fn do_auth_call<C: proxy_wasm::traits::HttpContext>(
    ctx: &C,
    filter: &CacheFilter,
    request_data: &ThreescaleData,
) -> Action {
    let cred = Credentials::ServiceToken(ServiceToken::from(request_data.service_token.as_ref()));
    let service = Service::new(request_data.service_id.as_ref(), cred);

    // TODO: Change this consider every app identifiers
    let app = threescalers::application::Application::from_user_key(
        request_data.app_id.as_string().as_ref(),
    );

    let mut metrics = Vec::new();
    for (metric, hits) in request_data.metrics.borrow().iter() {
        metrics.push((metric.clone(), hits.to_string().clone()));
    }

    let usage = threescalers::usage::Usage::new(metrics.as_slice());
    let txn = vec![threescalers::transaction::Transaction::new(
        &app,
        None,
        Some(&usage),
        None,
    )];

    let extensions =
        threescalers::extensions::List::new().push(threescalers::extensions::Extension::Hierarchy);

    let mut apicall = threescalers::api_call::ApiCall::builder(&service);
    let apicall = match apicall
        .transactions(&txn)
        .extensions(&extensions)
        .kind(threescalers::api_call::Kind::Authorize)
        .build()
    {
        Ok(result) => result,
        Err(e) => {
            info!(
                "ctxt {}: Couldn't contact 3scale due to {}",
                filter.context_id, e
            );
            return in_request_failure(ctx, filter);
        }
    };

    let request = threescalers::http::request::Request::from(&apicall);
    let (uri, body) = request.uri_and_body();
    let headers = request
        .headers
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();

    match filter.config.upstream.call(
        ctx,
        uri.as_ref(),
        request.method.as_str(),
        headers,
        body.map(str::as_bytes),
        None,
        None,
    ) {
        Ok(token) => info!(
            "ctxt {}: Dispatched successful: token: {}",
            filter.context_id, token
        ),
        Err(e) => {
            info!(
                "ctxt {}: Couldn't contact 3scale due to {:?}",
                filter.context_id, e
            );
            return in_request_failure(ctx, filter);
        }
    };
    // pause the current request to wait for the response from 3scale
    Action::Pause
}

pub fn period_from_response(res_period: &ResponsePeriod) -> Period {
    match res_period {
        ResponsePeriod::Minute => Period::Minute,
        ResponsePeriod::Hour => Period::Hour,
        ResponsePeriod::Day => Period::Day,
        ResponsePeriod::Week => Period::Week,
        ResponsePeriod::Month => Period::Month,
        ResponsePeriod::Year => Period::Year,
        ResponsePeriod::Eternity => Period::Eternity,
    }
}
