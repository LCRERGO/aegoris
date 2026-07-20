use aegoris_core::domain::Profile;
use aegoris_core::parse::parse_linkedin_html;
use anyhow::{bail, Context, Result};

const USER_AGENT: &str = concat!(
    "aegoris/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/anomalyco/opencode)"
);

/// Whether a profile argument should be fetched over HTTP.
pub fn is_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

/// Fetch a public LinkedIn profile page and normalize it.
///
/// Scraping is best-effort and opt-in: LinkedIn actively rate-limits and
/// redirects automated requests to a login wall. When the page does not expose
/// structured data, this fails with a clear error and the official data export
/// remains the reliable path.
pub async fn scrape_profile(url: &str) -> Result<Profile> {
    let html = fetch(url).await?;
    parse_linkedin_html(&html).with_context(|| format!("parsing scraped profile {url}"))
}

async fn fetch(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("building HTTP client")?;

    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
        .header(reqwest::header::ACCEPT_LANGUAGE, "en")
        .send()
        .await
        .with_context(|| format!("fetching {url}"))?;

    let status = response.status();
    match status.as_u16() {
        999 => bail!(
            "LinkedIn refused the request (HTTP 999 anti-bot); use the official data export instead"
        ),
        404 => bail!("LinkedIn profile not found: {url}"),
        _ if !status.is_success() => bail!("fetching {url} failed with status {status}"),
        _ => {}
    }

    response
        .text()
        .await
        .with_context(|| format!("reading response from {url}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_http_urls() {
        assert!(is_url("https://www.linkedin.com/in/ada"));
        assert!(is_url("http://example.com"));
        assert!(!is_url("/tmp/profile.json"));
        assert!(!is_url("profile.zip"));
        assert!(!is_url("-"));
    }
}
