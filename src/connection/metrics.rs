use prometheus::{register_int_counter_vec, register_int_gauge_vec, IntCounterVec, IntGaugeVec};

lazy_static::lazy_static! {
    pub static ref CONNECTIONS_ACCEPTED: IntCounterVec = register_int_counter_vec!(
        "kvstore_connections_accepted_total",
        "Total number of connections accepted",
        &["role"]
    ).unwrap();

    pub static ref CONNECTIONS_EVICTED: IntCounterVec = register_int_counter_vec!(
        "kvstore_connections_evicted_total",
        "Total number of connections evicted",
        &["reason"]
    ).unwrap();

    pub static ref CONNECTIONS_ACTIVE: IntGaugeVec = register_int_gauge_vec!(
        "kvstore_connections_active",
        "Number of currently active connections",
        &["role"]
    ).unwrap();
}

pub fn inc_accepted(role: &str) {
    CONNECTIONS_ACCEPTED.with_label_values(&[role]).inc();
}

pub fn inc_evicted(reason: &str) {
    CONNECTIONS_EVICTED.with_label_values(&[reason]).inc();
}

pub fn inc_active(role: &str) {
    CONNECTIONS_ACTIVE.with_label_values(&[role]).inc();
}

pub fn dec_active(role: &str) {
    CONNECTIONS_ACTIVE.with_label_values(&[role]).dec();
}