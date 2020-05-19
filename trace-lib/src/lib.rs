use opentelemetry::api::{
    self, Context, HttpTextFormat, KeyValue, Provider, Span, TraceContextExt, Tracer,
};
use opentelemetry::global;
use opentelemetry::sdk;
use tracing_subscriber::prelude::*;

struct TonicMetadataMapCarrier<'a>(&'a tonic::metadata::MetadataMap);
impl<'a> api::Carrier for TonicMetadataMapCarrier<'a> {
    fn get(&self, key: &'static str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn set(&mut self, _key: &'static str, _value: String) {
        unimplemented!()
    }
}

struct TonicMetadataMapCarrierMut<'a>(&'a mut tonic::metadata::MetadataMap);
impl<'a> api::Carrier for TonicMetadataMapCarrierMut<'a> {
    fn get(&self, key: &'static str) -> Option<&str> {
        println!("Extracting key {:?}", key);
        self.0.get(key).and_then(|metadata| metadata.to_str().ok())
    }

    fn set(&mut self, key: &'static str, value: String) {
        println!("Inserting key {:?} with value {:?}", key, value);
        if let Ok(key) = tonic::metadata::MetadataKey::from_bytes(key.to_lowercase().as_bytes()) {
            self.0.insert(
                key,
                tonic::metadata::MetadataValue::from_str(&value).unwrap(),
            );
        }
    }
}

pub struct TracedRequest {}

impl TracedRequest {
    pub fn new<T>(
        msg: T,
        service_name: &'static str,
        func_name: &'static str,
    ) -> tonic::Request<T> {
        let mut req = tonic::Request::new(msg);
        inject_trace(req.metadata_mut(), service_name, func_name);
        req
    }
}

pub fn inject_trace(
    headers: &mut tonic::metadata::MetadataMap,
    service_name: &'static str,
    func_name: &'static str,
) {
    let propagator = api::TraceContextPropagator::new();
    let span = global::tracer(service_name).start(func_name);
    let cx = Context::current_with_span(span);
    propagator.inject_context(&cx, &mut TonicMetadataMapCarrierMut(headers));
}

pub fn propagate_trace<T: std::fmt::Debug>(
    request: &tonic::Request<T>,
    service_name: &'static str,
    func_name: &'static str,
) -> global::BoxedSpan {
    let propagator = api::TraceContextPropagator::new();
    let parent_cx = propagator.extract(&TonicMetadataMapCarrier(request.metadata()));
    let span = global::tracer(service_name).start_from_context(func_name, &parent_cx);
    span.set_attribute(KeyValue::new("msg", format!("{:?}", request)));
    span
}

pub fn init_tracer(
    jaeger_url: &str,
    service_name: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    let exporter = opentelemetry_jaeger::Exporter::builder()
        .with_agent_endpoint(jaeger_url.parse()?)
        .with_process(opentelemetry_jaeger::Process {
            service_name: String::from(service_name),
            tags: Vec::new(),
        })
        .init()?;
    let provider = sdk::Provider::builder()
        .with_simple_exporter(exporter)
        .with_config(sdk::Config {
            default_sampler: Box::new(sdk::Sampler::Always),
            ..Default::default()
        })
        .build();
    let tracer = provider.get_tracer(service_name);
    global::set_provider(provider);

    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let filter = tracing_subscriber::EnvFilter::from_default_env();
    tracing_subscriber::registry()
        .with(opentelemetry)
        .with(filter)
        .try_init()?;

    Ok(())
}
