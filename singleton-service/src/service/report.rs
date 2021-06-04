use std::collections::HashMap;
use std::vec;
use threescalers::{
    api_call::{ApiCall, Kind},
    application::*,
    credentials::*,
    extensions::{self, Extension},
    http::Request,
    service::*,
    transaction::Transaction,
    usage::Usage,
};

pub struct Report<'a> {
    service_id: String,
    service_token: String,
    usages: HashMap<String, Vec<(&'a str, &'a str)>>,
}

impl<'a> Report<'a> {
    pub fn service_id(&self) -> &String {
        &self.service_id
    }

    pub fn service_token(&self) -> &String {
        &self.service_token
    }

    pub fn usages(&self) -> &HashMap<String, Vec<(&'a str, &'a str)>> {
        &self.usages
    }
}

/// Report method will be used by the cache flush implementation (both cache container limit and periodical)
/// to create a report, which is the proxy level representation. Then it will be used to build the report request
/// which is of threescalers Report request type.
pub fn report<'a>() -> Result<Report<'a>, anyhow::Error> {
    let metrics = [("hits", "1"), ("hits.79419", "1")].to_vec();
    let mut usages_map: HashMap<String, Vec<(&'a str, &'a str)>> = HashMap::new();
    usages_map.insert("46de54605a1321aa3838480c5fa91bcc".to_string(), metrics);
    Ok(Report {
        service_id: "2555417902188".to_string(),
        service_token: "6705c7d02e9a899d4db405dc1413361611e4250dfd12ec3dcbcea8c3de7cdd29"
            .to_string(),
        usages: usages_map,
    })
}

// build_report_request creates a request which is of type threescalers Report.
pub fn build_report_request(report: &Report) -> Result<Request, anyhow::Error> {
    let creds = Credentials::ServiceToken(ServiceToken::from(report.service_token().as_str()));
    let svc = Service::new(report.service_id().as_str(), creds);
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
    let extensions = extensions::List::new().no_body().push(Extension::Hierarchy);
    let mut api_call = ApiCall::builder(&svc);
    let api_call = api_call
        .transactions(&txns)
        .extensions(&extensions)
        .kind(Kind::Report)
        .build()?;
    Ok(Request::from(&api_call))
}
