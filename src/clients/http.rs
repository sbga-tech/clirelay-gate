pub fn client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder().user_agent("cpa-portal")
}
