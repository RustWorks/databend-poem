// Copyright 2021 Datafuse Labs
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::error::Error;

use common_meta_types::MetaError;
use common_metrics::register_counter_family;
use common_metrics::register_gauge;
use common_metrics::register_histogram_family_in_milliseconds;
use common_metrics::Counter;
use common_metrics::Family;
use common_metrics::Gauge;
use common_metrics::Histogram;
use common_metrics::VecLabels;
use lazy_static::lazy_static;

lazy_static! {
    static ref META_GRPC_CLIENT_REQUEST_DURATION_MS: Family<VecLabels, Histogram> =
        register_histogram_family_in_milliseconds("meta_grpc_client_request_duration_ms");
    static ref META_GRPC_CLIENT_REQUEST_INFLIGHT: Gauge =
        register_gauge("meta_grpc_client_request_inflight");
    static ref META_GRPC_CLIENT_REQUEST_SUCCESS: Family<VecLabels, Counter> =
        register_counter_family("meta_grpc_client_request_success");
    static ref META_GRPC_CLIENT_REQUEST_FAILED: Family<VecLabels, Counter> =
        register_counter_family("meta_grpc_client_request_fail");
    static ref META_GRPC_MAKE_CLIENT_FAIL: Family<VecLabels, Counter> =
        register_counter_family("meta_grpc_make_client_fail");
}

const LABEL_ENDPOINT: &str = "endpoint";
const LABEL_REQUEST: &str = "request";
const LABEL_ERROR: &str = "error";

pub fn record_meta_grpc_client_request_duration_ms(endpoint: &str, request: &str, duration: f64) {
    let labels = vec![
        (LABEL_ENDPOINT, endpoint.to_string()),
        (LABEL_REQUEST, request.to_string()),
    ];
    META_GRPC_CLIENT_REQUEST_DURATION_MS
        .get_or_create(&labels)
        .observe(duration);
}

pub fn incr_meta_grpc_client_request_inflight(val: i64) {
    META_GRPC_CLIENT_REQUEST_INFLIGHT.inc_by(val);
}

pub fn incr_meta_grpc_client_request_success(endpoint: &str, request: &str) {
    let labels = vec![
        (LABEL_ENDPOINT, endpoint.to_string()),
        (LABEL_REQUEST, request.to_string()),
    ];
    META_GRPC_CLIENT_REQUEST_SUCCESS
        .get_or_create(&labels)
        .inc();
}

pub fn incr_meta_grpc_client_request_failed(
    endpoint: &str,
    request: &str,
    err: &(dyn Error + 'static),
) {
    let err_name = err
        .downcast_ref::<MetaError>()
        .map(|e| e.name())
        .unwrap_or("unknown");
    let labels = vec![
        (LABEL_ENDPOINT, endpoint.to_string()),
        (LABEL_REQUEST, request.to_string()),
        (LABEL_ERROR, err_name.to_string()),
    ];
    META_GRPC_CLIENT_REQUEST_FAILED.get_or_create(&labels).inc();
}

pub fn incr_meta_grpc_make_client_fail(endpoint: &str) {
    let labels = vec![(LABEL_ENDPOINT, endpoint.to_string())];
    META_GRPC_MAKE_CLIENT_FAIL.get_or_create(&labels).inc();
}
