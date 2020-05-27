use anyhow::Result;
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

pub async fn begin_undo_event(client: &mut ApiClient, file: &String, user: &String) -> Result<()> {
    let undo_input = BeginUndoEventInput {
        file: file.clone(),
        user: user.clone(),
    };
    client.begin_undo_event(Request::new(undo_input)).await?;
    Ok(())
}

pub async fn undo_latest(
    client: &mut ApiClient,
    file: &String,
    user: &String,
    offset: i64,
) -> Result<i64> {
    let input = UndoLatestInput {
        prefix: Some(OpPrefixMsg {
            file: file.clone(),
            user: user.clone(),
            offset,
        }),
    };
    let output = client.undo_latest(Request::new(input)).await?.into_inner();
    Ok(output.offset)
}

pub async fn redo_latest(
    client: &mut ApiClient,
    file: &String,
    user: &String,
    offset: i64,
) -> Result<i64> {
    let input = RedoLatestInput {
        prefix: Some(OpPrefixMsg {
            file: file.clone(),
            user: user.clone(),
            offset,
        }),
    };
    let output = client.redo_latest(Request::new(input)).await?.into_inner();
    Ok(output.offset)
}

pub async fn move_objects(
    client: &mut ApiClient,
    prefix: &OpPrefixMsg,
    obj_ids: Vec<String>,
    delta: &Vector3Msg,
) -> Result<i64> {
    let input = MoveObjectsInput {
        prefix: Some(prefix.clone()),
        obj_ids,
        delta: Some(delta.clone()),
    };

    let output = client.move_objects(Request::new(input)).await?.into_inner();
    Ok(output.offset)
}

pub async fn delete_objects(
    client: &mut ApiClient,
    prefix: &OpPrefixMsg,
    ids: Vec<String>,
) -> Result<i64> {
    let input = DeleteObjectsInput {
        prefix: Some(prefix.clone()),
        obj_ids: ids,
    };

    let output = client
        .delete_objects(Request::new(input))
        .await?
        .into_inner();
    Ok(output.offset)
}

pub async fn join_objs_at_pt(
    client: &mut ApiClient,
    prefix: &OpPrefixMsg,
    id_1: &String,
    id_2: &String,
    guess: &Point3Msg,
) -> Result<i64> {
    let input = JoinObjectsAtPointInput {
        prefix: Some(prefix.clone()),
        first_id: id_1.clone(),
        second_id: id_2.clone(),
        guess: Some(guess.clone()),
    };

    let output = client
        .join_objects_at_point(Request::new(input))
        .await?
        .into_inner();
    Ok(output.offset)
}

pub async fn create_walls(
    client: &mut ApiClient,
    prefix: &OpPrefixMsg,
    walls: Vec<WallApiMsg>,
) -> Result<(i64, Vec<String>)> {
    let input = CreateWallsInput {
        prefix: Some(prefix.clone()),
        walls,
    };

    let output = client.create_walls(Request::new(input)).await?.into_inner();
    Ok((output.offset, output.obj_ids))
}
