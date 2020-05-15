use actix_web::{middleware, App, HttpServer};

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    HttpServer::new(|| {
        App::new()
            // enable logger
            .wrap(middleware::Logger::default())
            // websocket route
            .service(actix_files::Files::new("/", "/dist/"))
    })
    .bind("0.0.0.0:80")?
    .run()
    .await
}
