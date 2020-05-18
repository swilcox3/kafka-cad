use opentelemetry::api::Provider;
use opentelemetry::sdk;
use tracing_subscriber::prelude::*;

pub fn init_tracer(jaeger_url: &str, service_name: &str) -> Result<(), Box<dyn std::error::Error>> {
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
    let tracer = provider.get_tracer("tracing");

    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let filter = tracing_subscriber::EnvFilter::from_default_env();
    tracing_subscriber::registry()
        .with(opentelemetry)
        .with(filter)
        .try_init()?;

    Ok(())
}
