/// Simple trait for metrics instrumentation
pub trait MetricsCollector {
    fn inc_counter(&self, name: &str);
    fn observe_gauge(&self, name: &str, value: f64);
}
