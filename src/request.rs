use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Device class (`formFactor`). When not set, the API aggregates all devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FormFactor {
    /// All devices together. Same as not setting a form factor.
    AllFormFactors,
    /// Phones.
    Phone,
    /// Desktop computers.
    Desktop,
    /// Tablets.
    Tablet,
}

impl FormFactor {
    /// Wire value (`PHONE`, `DESKTOP`, ...).
    pub fn as_str(&self) -> &'static str {
        match self {
            FormFactor::AllFormFactors => "ALL_FORM_FACTORS",
            FormFactor::Phone => "PHONE",
            FormFactor::Desktop => "DESKTOP",
            FormFactor::Tablet => "TABLET",
        }
    }
}

/// Network class (`effectiveConnectionType`). Only accepted by
/// [`query_record`](crate::CruxApi::query_record), not by the history API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectiveConnectionType {
    /// `4G`
    #[serde(rename = "4G")]
    FourG,
    /// `3G`
    #[serde(rename = "3G")]
    ThreeG,
    /// `2G`
    #[serde(rename = "2G")]
    TwoG,
    /// `slow-2G`
    #[serde(rename = "slow-2G")]
    Slow2G,
    /// `offline`
    #[serde(rename = "offline")]
    Offline,
}

impl EffectiveConnectionType {
    /// Wire value (`4G`, `slow-2G`, ...).
    pub fn as_str(&self) -> &'static str {
        match self {
            EffectiveConnectionType::FourG => "4G",
            EffectiveConnectionType::ThreeG => "3G",
            EffectiveConnectionType::TwoG => "2G",
            EffectiveConnectionType::Slow2G => "slow-2G",
            EffectiveConnectionType::Offline => "offline",
        }
    }
}

/// Whether a request targets a whole origin or a single page.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    /// An origin such as `https://example.com` (scheme and host, no path).
    Origin(String),
    /// A page URL. The API normalizes it (drops the fragment, for example);
    /// see [`UrlNormalization`](crate::UrlNormalization).
    Url(String),
}

impl Target {
    fn insert_into(&self, body: &mut Map<String, Value>) {
        match self {
            Target::Origin(origin) => body.insert("origin".into(), Value::String(origin.clone())),
            Target::Url(url) => body.insert("url".into(), Value::String(url.clone())),
        };
    }

    fn value(&self) -> &str {
        match self {
            Target::Origin(v) | Target::Url(v) => v,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        let value = self.value();
        if !(value.starts_with("http://") || value.starts_with("https://")) {
            return Err(format!("must start with http:// or https://: {}", value));
        }
        Ok(())
    }
}

/// Body of `records:queryRecord`: the latest 28-day aggregate.
///
/// # Example
///
/// ```
/// use google_crux_api::{metric, FormFactor, QueryRecordRequest};
///
/// let request = QueryRecordRequest::origin("https://example.com")
///     .form_factor(FormFactor::Phone)
///     .metrics([metric::LARGEST_CONTENTFUL_PAINT, metric::INTERACTION_TO_NEXT_PAINT]);
///
/// assert_eq!(
///     request.to_json(),
///     serde_json::json!({
///         "origin": "https://example.com",
///         "formFactor": "PHONE",
///         "metrics": ["largest_contentful_paint", "interaction_to_next_paint"]
///     })
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRecordRequest {
    target: Target,
    form_factor: Option<FormFactor>,
    effective_connection_type: Option<EffectiveConnectionType>,
    metrics: Vec<String>,
}

impl QueryRecordRequest {
    /// Data for a whole origin, such as `https://example.com`.
    pub fn origin<T: Into<String>>(origin: T) -> Self {
        Self::new(Target::Origin(origin.into()))
    }

    /// Data for a single page URL.
    pub fn url<T: Into<String>>(url: T) -> Self {
        Self::new(Target::Url(url.into()))
    }

    /// Request for the given target.
    pub fn new(target: Target) -> Self {
        QueryRecordRequest {
            target,
            form_factor: None,
            effective_connection_type: None,
            metrics: Vec::new(),
        }
    }

    /// Limits the data to one device class.
    pub fn form_factor(mut self, form_factor: FormFactor) -> Self {
        self.form_factor = Some(form_factor);
        self
    }

    /// Limits the data to one network class.
    pub fn effective_connection_type(mut self, ect: EffectiveConnectionType) -> Self {
        self.effective_connection_type = Some(ect);
        self
    }

    /// Adds one metric. When no metric is set, the API returns all metrics.
    /// See [`metric`](crate::metric) for the names.
    pub fn metric<T: Into<String>>(mut self, name: T) -> Self {
        let name = name.into();
        if !self.metrics.contains(&name) {
            self.metrics.push(name);
        }
        self
    }

    /// Adds several metrics.
    pub fn metrics<I, T>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        for name in names {
            self = self.metric(name);
        }
        self
    }

    /// The target of the request.
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// JSON body sent to the API.
    pub fn to_json(&self) -> Value {
        let mut body = Map::new();
        self.target.insert_into(&mut body);
        if let Some(form_factor) = self.form_factor {
            body.insert("formFactor".into(), form_factor.as_str().into());
        }
        if let Some(ect) = self.effective_connection_type {
            body.insert("effectiveConnectionType".into(), ect.as_str().into());
        }
        if !self.metrics.is_empty() {
            body.insert("metrics".into(), self.metrics.clone().into());
        }
        Value::Object(body)
    }
}

/// Body of `records:queryHistoryRecord`: a weekly series of 28-day aggregates.
///
/// # Example
///
/// ```
/// use google_crux_api::{metric, QueryHistoryRequest};
///
/// let request = QueryHistoryRequest::url("https://example.com/")
///     .metric(metric::CUMULATIVE_LAYOUT_SHIFT)
///     .collection_period_count(10);
///
/// assert_eq!(
///     request.to_json(),
///     serde_json::json!({
///         "url": "https://example.com/",
///         "metrics": ["cumulative_layout_shift"],
///         "collectionPeriodCount": 10
///     })
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryHistoryRequest {
    target: Target,
    form_factor: Option<FormFactor>,
    metrics: Vec<String>,
    collection_period_count: Option<u32>,
}

impl QueryHistoryRequest {
    /// Maximum value of [`collection_period_count`](Self::collection_period_count).
    pub const MAX_COLLECTION_PERIOD_COUNT: u32 = 40;

    /// History for a whole origin.
    pub fn origin<T: Into<String>>(origin: T) -> Self {
        Self::new(Target::Origin(origin.into()))
    }

    /// History for a single page URL.
    pub fn url<T: Into<String>>(url: T) -> Self {
        Self::new(Target::Url(url.into()))
    }

    /// Request for the given target.
    pub fn new(target: Target) -> Self {
        QueryHistoryRequest {
            target,
            form_factor: None,
            metrics: Vec::new(),
            collection_period_count: None,
        }
    }

    /// Limits the data to one device class.
    pub fn form_factor(mut self, form_factor: FormFactor) -> Self {
        self.form_factor = Some(form_factor);
        self
    }

    /// Adds one metric. When no metric is set, the API returns all metrics.
    pub fn metric<T: Into<String>>(mut self, name: T) -> Self {
        let name = name.into();
        if !self.metrics.contains(&name) {
            self.metrics.push(name);
        }
        self
    }

    /// Adds several metrics.
    pub fn metrics<I, T>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        for name in names {
            self = self.metric(name);
        }
        self
    }

    /// Number of weekly periods to return, from 1 to 40. The API default is 25.
    pub fn collection_period_count(mut self, count: u32) -> Self {
        self.collection_period_count = Some(count);
        self
    }

    /// The target of the request.
    pub fn target(&self) -> &Target {
        &self.target
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        self.target.validate()?;
        if let Some(count) = self.collection_period_count {
            if count == 0 || count > Self::MAX_COLLECTION_PERIOD_COUNT {
                return Err(format!(
                    "collection_period_count must be between 1 and {}: {}",
                    Self::MAX_COLLECTION_PERIOD_COUNT,
                    count
                ));
            }
        }
        Ok(())
    }

    /// JSON body sent to the API.
    pub fn to_json(&self) -> Value {
        let mut body = Map::new();
        self.target.insert_into(&mut body);
        if let Some(form_factor) = self.form_factor {
            body.insert("formFactor".into(), form_factor.as_str().into());
        }
        if !self.metrics.is_empty() {
            body.insert("metrics".into(), self.metrics.clone().into());
        }
        if let Some(count) = self.collection_period_count {
            body.insert("collectionPeriodCount".into(), count.into());
        }
        Value::Object(body)
    }
}
