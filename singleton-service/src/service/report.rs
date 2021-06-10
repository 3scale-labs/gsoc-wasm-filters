use crate::service::deltas::AppDelta;
use std::collections::HashMap;
use std::vec;
use threescalers::{
    api_call::{ApiCall, Kind},
    application::*,
    credentials::*,
    extensions::{self},
    http::Request,
    service::*,
    transaction::Transaction,
    usage::Usage,
};

#[derive(Debug)]
pub struct Report {
    service_id: String,
    service_token: String,
    usages: HashMap<String, Vec<(String, String)>>,
}

impl Report {
    pub fn service_id(&self) -> &str {
        self.service_id.as_str()
    }

    pub fn service_token(&self) -> &str {
        self.service_token.as_str()
    }

    pub fn usages(&self) -> &HashMap<String, Vec<(String, String)>> {
        &self.usages
    }
}

/// Report method will be used by the cache flush implementation (both cache container limit and periodical)
/// to create a report, which is the proxy level representation. Then it will be used to build the report request
/// which is of threescalers Report request type.
pub fn report<'a>(
    key: &'a str,
    apps: &'a HashMap<String, AppDelta>,
) -> Result<Report, anyhow::Error> {
    let keys = key.split('_').collect::<Vec<_>>();
    let mut usages_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for e in apps {
        let (app_id, app_deltas): (&String, &AppDelta) = e;
        let usage = app_deltas
            .usages
            .iter()
            .map(|(m, v)| (m.to_string(), v.to_string()))
            .collect::<Vec<(String, String)>>();
        usages_map.insert(app_id.to_string(), usage);
    }
    //usages_map.insert("46de54605a1321aa3838480c5fa91bcc".to_string(), metrics);
    Ok(Report {
        service_id: keys[0].to_string(),
        service_token: keys[1].to_string(),
        usages: usages_map,
    })
}

// build_report_request creates a request which is of type threescalers Report.
pub fn build_report_request(report: &Report) -> Result<Request, anyhow::Error> {
    let creds = Credentials::ServiceToken(ServiceToken::from(report.service_token()));
    let svc = Service::new(report.service_id(), creds);
    let mut app_usage = vec![];
    for (user_key, usage) in report.usages().iter() {
        let application = Application::from(UserKey::from(user_key.as_str()));
        let usage = Usage::new(usage);
        app_usage.push((application, usage))
    }
    let txns = app_usage
        .iter()
        .map(|au| (Transaction::new(&au.0, None, Some(&au.1), None)))
        .collect::<Vec<_>>();
    let extensions = extensions::List::new();
    let mut api_call = ApiCall::builder(&svc);
    let api_call = api_call
        .transactions(&txns)
        .extensions(&extensions)
        .kind(Kind::Report)
        .build()?;
    Ok(Request::from(&api_call))
}
