use anyhow::{anyhow, Result};
use tonic::transport::Channel;
use tonic::Request;

mod geom {
    tonic::include_proto!("geom");
}
pub use geom::*;

pub mod api {
    tonic::include_proto!("api");
}
pub use api::*;

pub type ApiClient = api_client::ApiClient<Channel>;

pub async fn begin_undo_event(
    client: &mut ApiClient,
    file: &String,
    user: &String,
) -> Result<()> {
    let undo_input = BeginUndoEventInput {
        file: file.clone(),
        user: user.clone(),
    };
    client
        .begin_undo_event(Request::new(undo_input))
        .await?;
    Ok(())
}

pub async fn undo_latest(client: &mut ApiClient, file: &String, user: &String, offset: i64) -> Result<i64> {
    let input = UndoLatestInput {
        prefix: Some(OpPrefixMsg {
            file: file.clone(),
            user: user.clone(),
            offset
        })
    };
    let output = client.undo_latest(Request::new(input)).await?.into_inner();
    Ok(output.offset)
}

pub async fn redo_latest(client: &mut ApiClient, file: &String, user: &String, offset: i64) -> Result<i64> {
    let input = RedoLatestInput {
        prefix: Some(OpPrefixMsg {
            file: file.clone(),
            user: user.clone(),
            offset
        })
    };
    let output = client.redo_latest(Request::new(input)).await?.into_inner();
    Ok(output.offset)
}

/*pub async fn move_objects(
    client: &mut ApiClient,
    prefix: &OpPrefixMsg,
    obj_ids: Vec<String>,
    delta: &Vector3Msg,
) -> Result<u64> {
    let input = MoveObjectsInput {
        prefix: Some(prefix.clone()),
        obj_ids,
        delta: Some(delta.clone()),
    };

    let output = client.move_objects(Request::new(input)).await?.into_inner();
    Ok(output.change)
}*/

/*pub async fn delete_objects(
    client: &mut ApiClient,
    prefix: &OpPrefixMsg,
    ids: Vec<String>,
) -> Result<u64> {
    let input = DeleteObjectsInput {
        prefix: Some(prefix.clone()),
        obj_ids: ids,
    };

    let output = client
        .delete_objects(Request::new(input))
        .await?
        .into_inner();
    Ok(output.change)
}*/

pub async fn create_wall(
    client: &mut ApiClient,
    prefix: &OpPrefixMsg,
    first_pt: &Point3Msg,
    second_pt: &Point3Msg,
    width: f64,
    height: f64,
) -> Result<(i64, String)> {
    let input = CreateWallsInput {
        prefix: Some(prefix.clone()),
        walls: vec![ WallApiMsg {
        first_pt: Some(first_pt.clone()),
        second_pt: Some(second_pt.clone()),
        width,
        height,
        }]
    };

    let mut output = client.create_walls(Request::new(input)).await?.into_inner();
    Ok((output.offset, output.obj_ids.pop().ok_or(anyhow!("No ids returned"))?))
}