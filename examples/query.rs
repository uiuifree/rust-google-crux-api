//! Prints the current Core Web Vitals and the LCP history of a URL or origin.
//!
//! ```bash
//! CRUX_API_KEY=... cargo run --example query -- https://example.com        # origin
//! CRUX_API_KEY=... cargo run --example query -- https://example.com/page   # page
//! ```

use google_crux_api::{CruxApi, QueryHistoryRequest, QueryRecordRequest, Target, metric};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .ok_or("usage: query <origin or url>")?;
    let key = std::env::var("CRUX_API_KEY").map_err(|_| "set CRUX_API_KEY")?;
    let api = CruxApi::with_api_key(key);

    // "https://example.com" is an origin, anything with a path is a page
    let is_origin = input.splitn(4, '/').nth(3).is_none_or(str::is_empty) && !input.ends_with('/');
    let target = if is_origin {
        Target::Origin(input)
    } else {
        Target::Url(input)
    };

    match api
        .query_record(&QueryRecordRequest::new(target.clone()))
        .await
    {
        Ok(response) => {
            let vitals = response.record.core_web_vitals();
            println!("{}", serde_json::to_string_pretty(&vitals)?);
            println!("passes Core Web Vitals: {:?}", vitals.passes());
        }
        Err(e) if e.is_not_found() => {
            println!("no CrUX data");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    }

    let history = api
        .query_history_record(
            &QueryHistoryRequest::new(target).metric(metric::LARGEST_CONTENTFUL_PAINT),
        )
        .await?;
    for (period, p75) in history.record.p75_series(metric::LARGEST_CONTENTFUL_PAINT) {
        println!("{}  LCP p75 {:?}", period.last_date, p75);
    }
    Ok(())
}
