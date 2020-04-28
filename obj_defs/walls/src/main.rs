use actix_protobuf::*;
use actix_web::*;
use log::*;

mod walls {
    include!(concat!(env!("OUT_DIR"), "/walls.rs"));
}
use walls::*;

mod object_state {
    include!(concat!(env!("OUT_DIR"), "/object_state.rs"));
}
use object_state::*;

mod representation {
    include!(concat!(env!("OUT_DIR"), "/representation.rs"));
}
use representation::*;

mod obj_defs {
    include!(concat!(env!("OUT_DIR"), "/obj_defs.rs"));
}
use obj_defs::*;

async fn create(msg: ProtoBuf<CreateWallsInput>) -> Result<HttpResponse> {
    HttpResponse::Ok().protobuf(msg.0) // <- send response
}

async fn recalculate(msg: ProtoBuf<RecalculateInput>) -> Result<HttpResponse> {
    HttpResponse::Ok().protobuf(msg.0) // <- send response
}

async fn client_representation(msg: ProtoBuf<ClientRepresentationInput>) -> Result<HttpResponse> {
    HttpResponse::Ok().protobuf(msg.0) // <- send response
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let run_url = std::env::var("RUN_URL").unwrap();
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            .service(web::resource("/create").route(web::post().to(create)))
            .service(web::resource("/recalculate").route(web::post().to(recalculate)))
            .service(
                web::resource("/client-representation").route(web::get().to(client_representation)),
            )
    })
    .bind(run_url)?
    .run()
    .await
    .unwrap();
    Ok(())
}
