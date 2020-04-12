use geom_client_rs::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = GeomKernelClient::connect(String::from("http://0.0.0.0:50051")).await?;
    let input = MakePrismInput {
        first_pt: Some(Point3Msg {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }),
        second_pt: Some(Point3Msg {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        }),
        width: 1.0,
        height: 1.0,
    };
    let response = make_prism(&mut client, input).await?;
    println!("{:?}", response);
    Ok(())
}
