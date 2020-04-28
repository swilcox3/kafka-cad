use crate::*;

async fn get_all_previous_objects(
    obj_client: &mut ObjClient,
    file: &str,
    entries: &Vec<UndoEntry>,
) -> Result<Vec<OptionChangeMsg>, Status> {
    let mut obj_ids = Vec::new();
    for entry in entries {
        //Get the offset - 1 so we get the previous state of the object
        obj_ids.push(ObjectAtOffset {
            offset: entry.offset - 1,
            obj_id: entry.obj_id.clone(),
        });
    }
    let mut input = GetObjectsInput {
        file: String::from(file),
        obj_ids,
    };
    let objs_msg = obj_client
        .get_objects(Request::new(input))
        .await?
        .into_inner();
    Ok(objs_msg.objects)
}

pub async fn invert_changes(
    obj_client: &mut ObjClient,
    file: &str,
    entries: &Vec<UndoEntry>,
) -> Result<Vec<ChangeMsg>, Status> {
    let previous = get_all_previous_objects(obj_client, file, entries).await?;
    let mut inverted = Vec::new();
    Ok(inverted)
}
