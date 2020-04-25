use tonic::transport::Channel;
use tonic::Request;

mod object_state {
    tonic::include_proto!("object_state");
}

mod submit {
    tonic::include_proto!("submit");
}

use object_state::*;
use submit::*;
pub type ObjClient = submit_changes_client::SubmitChangesClient<Channel>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = submit_changes_client::SubmitChangesClient::connect("http://127.0.0.1:6003")
        .await
        .unwrap();
    let user = uuid::Uuid::new_v4().to_string();
    let object = ChangeMsg {
        id: uuid::Uuid::new_v4().to_string(),
        user: user.clone(),
        change_type: Some(change_msg::ChangeType::Add(ObjectMsg {
            dependencies: None,
            results: None,
            obj_data: Vec::new(),
        })),
    };
    let input = SubmitChangesInput {
        file: uuid::Uuid::new_v4().to_string(),
        user,
        offset: 0,
        changes: vec![object],
    };
    let output = client
        .submit_changes(Request::new(input))
        .await
        .unwrap()
        .into_inner();
    println!("{:?}", output);
    Ok(())
}
