//! Metric names accepted and returned by the CrUX API, and the Core Web
//! Vitals thresholds.

use serde::{Deserialize, Serialize};

/// Largest Contentful Paint, milliseconds. Core Web Vital.
pub const LARGEST_CONTENTFUL_PAINT: &str = "largest_contentful_paint";
/// Interaction to Next Paint, milliseconds. Core Web Vital.
pub const INTERACTION_TO_NEXT_PAINT: &str = "interaction_to_next_paint";
/// Cumulative Layout Shift, unitless. Core Web Vital.
pub const CUMULATIVE_LAYOUT_SHIFT: &str = "cumulative_layout_shift";
/// First Contentful Paint, milliseconds.
pub const FIRST_CONTENTFUL_PAINT: &str = "first_contentful_paint";
/// Time to First Byte, milliseconds.
pub const EXPERIMENTAL_TIME_TO_FIRST_BYTE: &str = "experimental_time_to_first_byte";
/// Round trip time of the network connection, milliseconds.
pub const ROUND_TRIP_TIME: &str = "round_trip_time";
/// Share of page loads by navigation type (`navigate`, `reload`, `back_forward_cache`, ...).
/// Returned as fractions, not a histogram.
pub const NAVIGATION_TYPES: &str = "navigation_types";
/// Share of page loads by device (`phone`, `desktop`, `tablet`).
/// Returned as fractions, and only when no form factor is set in the request.
pub const FORM_FACTORS: &str = "form_factors";
/// Share of LCP elements that are images or text. Returned as fractions.
pub const LARGEST_CONTENTFUL_PAINT_RESOURCE_TYPE: &str = "largest_contentful_paint_resource_type";
/// LCP subpart: time to first byte of the page, for image LCPs. Percentiles only.
pub const LARGEST_CONTENTFUL_PAINT_IMAGE_TIME_TO_FIRST_BYTE: &str =
    "largest_contentful_paint_image_time_to_first_byte";
/// LCP subpart: delay before the LCP image starts loading. Percentiles only.
pub const LARGEST_CONTENTFUL_PAINT_IMAGE_RESOURCE_LOAD_DELAY: &str =
    "largest_contentful_paint_image_resource_load_delay";
/// LCP subpart: time to load the LCP image. Percentiles only.
pub const LARGEST_CONTENTFUL_PAINT_IMAGE_RESOURCE_LOAD_DURATION: &str =
    "largest_contentful_paint_image_resource_load_duration";
/// LCP subpart: delay between the image load and the paint. Percentiles only.
pub const LARGEST_CONTENTFUL_PAINT_IMAGE_ELEMENT_RENDER_DELAY: &str =
    "largest_contentful_paint_image_element_render_delay";

/// The three Core Web Vitals.
pub const CORE_WEB_VITALS: [&str; 3] = [
    LARGEST_CONTENTFUL_PAINT,
    INTERACTION_TO_NEXT_PAINT,
    CUMULATIVE_LAYOUT_SHIFT,
];

/// Assessment of a 75th percentile value against the published thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    /// Good.
    Good,
    /// Needs improvement.
    NeedsImprovement,
    /// Poor.
    Poor,
}

/// `(good_max, poor_min)` thresholds for a metric, or `None` when the metric
/// has no published thresholds.
///
/// A value is good when it is at most `good_max`, poor when it is above
/// `poor_min`, and needs improvement in between.
pub fn thresholds(metric: &str) -> Option<(f64, f64)> {
    match metric {
        LARGEST_CONTENTFUL_PAINT => Some((2500.0, 4000.0)),
        INTERACTION_TO_NEXT_PAINT => Some((200.0, 500.0)),
        CUMULATIVE_LAYOUT_SHIFT => Some((0.1, 0.25)),
        FIRST_CONTENTFUL_PAINT => Some((1800.0, 3000.0)),
        EXPERIMENTAL_TIME_TO_FIRST_BYTE => Some((800.0, 1800.0)),
        _ => None,
    }
}

/// Rates a 75th percentile value.
///
/// # Example
///
/// ```
/// use google_crux_api::metric::{self, Rating};
///
/// assert_eq!(metric::rate(metric::LARGEST_CONTENTFUL_PAINT, 2500.0), Some(Rating::Good));
/// assert_eq!(metric::rate(metric::INTERACTION_TO_NEXT_PAINT, 350.0), Some(Rating::NeedsImprovement));
/// assert_eq!(metric::rate(metric::CUMULATIVE_LAYOUT_SHIFT, 0.3), Some(Rating::Poor));
/// assert_eq!(metric::rate(metric::ROUND_TRIP_TIME, 100.0), None);
/// ```
pub fn rate(metric: &str, p75: f64) -> Option<Rating> {
    let (good, poor) = thresholds(metric)?;
    Some(if p75 <= good {
        Rating::Good
    } else if p75 <= poor {
        Rating::NeedsImprovement
    } else {
        Rating::Poor
    })
}
