//! Response types of `records:queryRecord` and `records:queryHistoryRecord`.
//!
//! The API sends some numbers as strings (CLS values such as `"0.05"`) and
//! marks missing history points with `"NaN"` or `null`. All numeric fields
//! here are `Option<f64>` and accept any of those forms.

use crate::metric::{self, Rating};
use crate::request::{EffectiveConnectionType, FormFactor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

fn lenient_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok().filter(|v| v.is_finite()),
        _ => None,
    }
}

fn de_opt_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    let value = Option::<Value>::deserialize(d)?;
    Ok(value.as_ref().and_then(lenient_f64))
}

fn de_vec_opt_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Option<f64>>, D::Error> {
    let values = Option::<Vec<Value>>::deserialize(d)?.unwrap_or_default();
    Ok(values.iter().map(lenient_f64).collect())
}

fn de_map_f64<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, f64>, D::Error> {
    let values = Option::<BTreeMap<String, Value>>::deserialize(d)?.unwrap_or_default();
    Ok(values
        .into_iter()
        .filter_map(|(k, v)| lenient_f64(&v).map(|v| (k, v)))
        .collect())
}

/// Response of `records:queryRecord`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryRecordResponse {
    /// The data.
    #[serde(default)]
    pub record: Record,
    /// Present when the requested URL was changed before the lookup.
    pub url_normalization_details: Option<UrlNormalization>,
}

/// Response of `records:queryHistoryRecord`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryHistoryResponse {
    /// The data.
    #[serde(default)]
    pub record: HistoryRecord,
    /// Present when the requested URL was changed before the lookup.
    pub url_normalization_details: Option<UrlNormalization>,
}

/// How the API changed the requested URL (for example removed a fragment or
/// followed a known redirect).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlNormalization {
    /// The URL in the request.
    pub original_url: Option<String>,
    /// The URL that was looked up.
    pub normalized_url: Option<String>,
}

/// What the data is about.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    /// Set for origin queries.
    pub origin: Option<String>,
    /// Set for URL queries.
    pub url: Option<String>,
    /// Set when the request had a form factor.
    pub form_factor: Option<FormFactor>,
    /// Set when the request had a connection type (current record only).
    pub effective_connection_type: Option<EffectiveConnectionType>,
}

/// A calendar date.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Date {
    /// Year.
    pub year: i32,
    /// Month, 1 to 12.
    pub month: u32,
    /// Day, 1 to 31.
    pub day: u32,
}

impl Display for Date {
    /// Formats as `YYYY-MM-DD`.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// The 28 days the data covers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPeriod {
    /// First day, inclusive.
    pub first_date: Date,
    /// Last day, inclusive.
    pub last_date: Date,
}

/// The latest 28-day aggregate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    /// What the data is about.
    #[serde(default)]
    pub key: Key,
    /// Metrics keyed by name. See [`crate::metric`] for the names.
    #[serde(default)]
    pub metrics: BTreeMap<String, Metric>,
    /// The period the data covers.
    pub collection_period: Option<CollectionPeriod>,
}

impl Record {
    /// Returns one metric by name.
    pub fn metric(&self, name: &str) -> Option<&Metric> {
        self.metrics.get(name)
    }

    /// 75th percentile of a metric.
    pub fn p75(&self, name: &str) -> Option<f64> {
        self.metric(name)?.p75()
    }
}

/// One metric of a [`Record`].
///
/// Most metrics have a `histogram` and `percentiles`. A few
/// ([`NAVIGATION_TYPES`](crate::metric::NAVIGATION_TYPES),
/// [`FORM_FACTORS`](crate::metric::FORM_FACTORS), ...) only have `fractions`,
/// and the LCP subparts only have `percentiles`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metric {
    /// Buckets with the share of page loads in each.
    #[serde(default)]
    pub histogram: Vec<Bin>,
    /// Percentiles.
    pub percentiles: Option<Percentiles>,
    /// Share of page loads per label, for fraction metrics.
    #[serde(default, deserialize_with = "de_map_f64")]
    pub fractions: BTreeMap<String, f64>,
}

impl Metric {
    /// 75th percentile.
    pub fn p75(&self) -> Option<f64> {
        self.percentiles.as_ref()?.p75
    }
}

/// One histogram bucket.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Bin {
    /// Lower bound, inclusive.
    #[serde(default, deserialize_with = "de_opt_f64")]
    pub start: Option<f64>,
    /// Upper bound, exclusive. `None` for the last bucket.
    #[serde(default, deserialize_with = "de_opt_f64")]
    pub end: Option<f64>,
    /// Share of page loads in this bucket, from `0.0` to `1.0`.
    #[serde(default, deserialize_with = "de_opt_f64")]
    pub density: Option<f64>,
}

/// Percentiles of a metric.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Percentiles {
    /// 75th percentile.
    #[serde(default, deserialize_with = "de_opt_f64")]
    pub p75: Option<f64>,
}

/// A weekly series of 28-day aggregates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    /// What the data is about.
    #[serde(default)]
    pub key: Key,
    /// Metrics keyed by name. Each series has one value per collection period.
    #[serde(default)]
    pub metrics: BTreeMap<String, MetricTimeseries>,
    /// The periods, oldest first.
    #[serde(default)]
    pub collection_periods: Vec<CollectionPeriod>,
}

impl HistoryRecord {
    /// Returns one metric by name.
    pub fn metric(&self, name: &str) -> Option<&MetricTimeseries> {
        self.metrics.get(name)
    }

    /// 75th percentile of a metric for each period, oldest first.
    /// Periods without data have `None`.
    ///
    /// # Example
    ///
    /// ```
    /// use google_crux_api::{metric, HistoryRecord};
    ///
    /// let record: HistoryRecord = serde_json::from_value(serde_json::json!({
    ///   "key": {"origin": "https://example.com"},
    ///   "metrics": {
    ///     "cumulative_layout_shift": {"percentilesTimeseries": {"p75s": ["0.12", "NaN", "0.08"]}}
    ///   },
    ///   "collectionPeriods": [
    ///     {"firstDate": {"year": 2026, "month": 9, "day": 7}, "lastDate": {"year": 2026, "month": 10, "day": 4}},
    ///     {"firstDate": {"year": 2026, "month": 9, "day": 14}, "lastDate": {"year": 2026, "month": 10, "day": 11}},
    ///     {"firstDate": {"year": 2026, "month": 9, "day": 21}, "lastDate": {"year": 2026, "month": 10, "day": 18}}
    ///   ]
    /// })).unwrap();
    ///
    /// let series = record.p75_series(metric::CUMULATIVE_LAYOUT_SHIFT);
    /// assert_eq!(series.len(), 3);
    /// assert_eq!(series[0].1, Some(0.12));
    /// assert_eq!(series[1].1, None);
    /// assert_eq!(series[2].0.last_date.to_string(), "2026-10-18");
    /// ```
    pub fn p75_series(&self, name: &str) -> Vec<(CollectionPeriod, Option<f64>)> {
        let p75s = self
            .metric(name)
            .and_then(|m| m.percentiles_timeseries.as_ref())
            .map(|p| p.p75s.as_slice())
            .unwrap_or_default();
        self.collection_periods
            .iter()
            .enumerate()
            .map(|(i, period)| (*period, p75s.get(i).copied().flatten()))
            .collect()
    }
}

/// One metric of a [`HistoryRecord`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricTimeseries {
    /// Buckets with the share of page loads in each period.
    #[serde(default)]
    pub histogram_timeseries: Vec<TimeseriesBin>,
    /// Percentiles for each period.
    pub percentiles_timeseries: Option<TimeseriesPercentiles>,
    /// Share of page loads per label for each period, for fraction metrics.
    #[serde(default)]
    pub fraction_timeseries: BTreeMap<String, FractionTimeseries>,
}

/// One histogram bucket over time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TimeseriesBin {
    /// Lower bound, inclusive.
    #[serde(default, deserialize_with = "de_opt_f64")]
    pub start: Option<f64>,
    /// Upper bound, exclusive. `None` for the last bucket.
    #[serde(default, deserialize_with = "de_opt_f64")]
    pub end: Option<f64>,
    /// Share of page loads in this bucket for each period.
    #[serde(default, deserialize_with = "de_vec_opt_f64")]
    pub densities: Vec<Option<f64>>,
}

/// Percentiles over time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TimeseriesPercentiles {
    /// 75th percentile for each period.
    #[serde(default, deserialize_with = "de_vec_opt_f64")]
    pub p75s: Vec<Option<f64>>,
}

/// Fractions of one label over time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FractionTimeseries {
    /// Share for each period.
    #[serde(default, deserialize_with = "de_vec_opt_f64")]
    pub fractions: Vec<Option<f64>>,
}

/// A metric's 75th percentile with its rating and the share of page loads
/// that were good, needed improvement, or were poor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MetricSummary {
    /// 75th percentile.
    pub p75: f64,
    /// Rating of the 75th percentile.
    pub rating: Option<Rating>,
    /// Share of good page loads (first histogram bucket).
    pub good: Option<f64>,
    /// Share of page loads that need improvement (second bucket).
    pub needs_improvement: Option<f64>,
    /// Share of poor page loads (third bucket).
    pub poor: Option<f64>,
}

impl MetricSummary {
    fn from_metric(name: &str, metric: &Metric) -> Option<MetricSummary> {
        let p75 = metric.p75()?;
        let density = |i: usize| metric.histogram.get(i).and_then(|b| b.density);
        Some(MetricSummary {
            p75,
            rating: metric::rate(name, p75),
            good: density(0),
            needs_improvement: density(1),
            poor: density(2),
        })
    }
}

/// Core Web Vitals and related metrics of a [`Record`] in one flat struct.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CoreWebVitals {
    /// Largest Contentful Paint, milliseconds.
    pub lcp: Option<MetricSummary>,
    /// Interaction to Next Paint, milliseconds.
    pub inp: Option<MetricSummary>,
    /// Cumulative Layout Shift.
    pub cls: Option<MetricSummary>,
    /// First Contentful Paint, milliseconds.
    pub fcp: Option<MetricSummary>,
    /// Time to First Byte, milliseconds.
    pub ttfb: Option<MetricSummary>,
}

impl CoreWebVitals {
    /// Whether the page passes the Core Web Vitals assessment: every Core Web
    /// Vital that has data is good. `None` when LCP and CLS both have no data.
    ///
    /// INP is left out when it has no data, as Google does for pages with
    /// few interactions.
    pub fn passes(&self) -> Option<bool> {
        if self.lcp.is_none() && self.cls.is_none() {
            return None;
        }
        Some(
            [self.lcp, self.inp, self.cls]
                .iter()
                .flatten()
                .all(|m| m.rating == Some(Rating::Good)),
        )
    }
}

impl Record {
    /// Summary of the Core Web Vitals and FCP / TTFB.
    pub fn core_web_vitals(&self) -> CoreWebVitals {
        let summary = |name: &str| MetricSummary::from_metric(name, self.metric(name)?);
        CoreWebVitals {
            lcp: summary(metric::LARGEST_CONTENTFUL_PAINT),
            inp: summary(metric::INTERACTION_TO_NEXT_PAINT),
            cls: summary(metric::CUMULATIVE_LAYOUT_SHIFT),
            fcp: summary(metric::FIRST_CONTENTFUL_PAINT),
            ttfb: summary(metric::EXPERIMENTAL_TIME_TO_FIRST_BYTE),
        }
    }
}
