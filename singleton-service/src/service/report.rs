use log::debug;
use std::collections::HashMap;
use std::vec;
use threescale::structs::AppIdentifier;
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

/// Proxy level representation of the report data for a single service.
#[derive(Debug)]
pub struct Report {
    service_id: String,
    service_token: String,
    usages: HashMap<AppIdentifier, Vec<(String, String)>>,
}

impl Report {
    pub fn service_id(&self) -> &str {
        self.service_id.as_str()
    }

    pub fn service_token(&self) -> &str {
        self.service_token.as_str()
    }

    pub fn usages(&self) -> &HashMap<AppIdentifier, Vec<(String, String)>> {
        &self.usages
    }
}

/// This method will be used by the cache flush implementation (both cache container limit and period based)
/// to create a report, which is the proxy level representation. Then it will be used to build the report request
/// which is of threescalers Report request type.
pub fn report<'a>(
    key: &'a str,
    apps: &'a HashMap<AppIdentifier, HashMap<String, u64>>,
) -> Result<Report, anyhow::Error> {
    let keys = key.split('_').collect::<Vec<_>>();
    let mut usages_map: HashMap<AppIdentifier, Vec<(String, String)>> = HashMap::new();
    for app in apps {
        let (app_id, app_deltas): (&AppIdentifier, &HashMap<String, u64>) = app;
        let usage = app_deltas
            .iter()
            .map(|(m, v)| (m.to_string(), v.to_string()))
            .collect::<Vec<(String, String)>>();
        usages_map.insert(app_id.clone(), usage);
    }
    Ok(Report {
        service_id: keys[0].to_string(),
        service_token: keys[1].to_string(),
        usages: usages_map,
    })
}

/// This method creates a request which is of type threescalers Report.
pub fn build_report_request(report: &Report) -> Result<Request, anyhow::Error> {
    let creds = Credentials::ServiceToken(ServiceToken::from(report.service_token()));
    let svc = Service::new(report.service_id(), creds);
    let mut app_usage = vec![];
    for (app_identifier, usage) in report.usages().iter() {
        let app;
        match app_identifier {
            AppIdentifier::UserKey(user_key) => {
                debug!("AppIdentifier UserKey: {:?}", user_key);
                app = Application::from_user_key(user_key.as_ref())
            }
            AppIdentifier::AppId(app_id, None) => {
                debug!("AppIdentifier AppId : {:?}", app_id);
                app = Application::from_app_id(app_id.as_ref())
            }
            AppIdentifier::AppId(app_id, Some(app_key)) => {
                debug!("AppIdentifier AppId+AppKey : {:?}_{:?}", app_id, app_key);
                app = Application::from_app_id_and_key(app_id.as_ref(), app_key.as_ref())
            }
        }
        let usage = Usage::new(usage);
        app_usage.push((app, usage))
    }
    let txns = app_usage
        .iter()
        .map(|au| (Transaction::new(&au.0, None, Some(&au.1), None)))
        .collect::<Vec<_>>();
    // TODO : Add FlatUsage extension
    let extensions = extensions::List::new().push(extensions::Extension::FlatUsage("1".into()));
    let mut api_call = ApiCall::builder(&svc);
    let api_call = api_call
        .transactions(&txns)
        .extensions(&extensions)
        .kind(Kind::Report)
        .build()?;
    Ok(Request::from(&api_call))
}
