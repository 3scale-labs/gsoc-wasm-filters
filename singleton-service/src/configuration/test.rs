use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct TestConfiguration {
    pub service_id_1: String,
    pub service_token_1: String,
    pub application_1: String,
    pub service_id_2: String,
    pub service_token_2: String,
    pub application_2: String,
}
