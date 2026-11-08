# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0]

### Added

- `CruxApi::query_record` and `query_history_record`, with `_raw` variants, using an API key
  or an OAuth access token.
- Typed records with lenient number parsing (string CLS values, `"NaN"` in history).
- Metric name constants, Core Web Vitals thresholds and `Record::core_web_vitals`.
- Re-export of `reqwest` for `with_client`.
- Live tests against the real API (`tests/live.rs`, ignored by default) and a weekly CI run.
