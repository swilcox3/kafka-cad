use crate::*;
use trace_lib::TracedRequest;

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
    debug!("Going to get objects: {:?}", obj_ids);
    let input = GetObjectsInput {
        file: String::from(file),
        obj_ids,
    };
    let objs_msg = obj_client
        .get_objects(TracedRequest::new(input))
        .await?
        .into_inner();
    Ok(objs_msg.objects)
}

fn invert_changes_inner(
    user: &str,
    source: change_msg::ChangeSource,
    entries: Vec<UndoEntry>,
    previous: Vec<OptionChangeMsg>,
) -> Vec<ChangeMsg> {
    let mut inverted = Vec::new();
    for (current, prev) in entries.into_iter().zip(previous.into_iter()) {
        let source = source.clone();
        match prev.change {
            Some(prev_change) => match current.change_type {
                UndoChangeType::Add => {
                    inverted.push(ChangeMsg {
                        user: String::from(user),
                        change_type: Some(change_msg::ChangeType::Delete(DeleteMsg {
                            id: current.obj_id,
                        })),
                        change_source: Some(source),
                    });
                }
                UndoChangeType::Modify => match prev_change.change_type {
                    Some(change_msg::ChangeType::Add(prev_object)) => {
                        inverted.push(ChangeMsg {
                            user: String::from(user),
                            change_type: Some(change_msg::ChangeType::Modify(prev_object)),
                            change_source: Some(source),
                        });
                    }
                    Some(change_msg::ChangeType::Modify(prev_object)) => {
                        inverted.push(ChangeMsg {
                            user: String::from(user),
                            change_type: Some(change_msg::ChangeType::Modify(prev_object)),
                            change_source: Some(source),
                        });
                    }
                    Some(change_msg::ChangeType::Delete(..)) => {
                        error!("Invalid modify coming after a delete");
                    }
                    None => {
                        error!("No data to undo back to");
                    }
                },
                UndoChangeType::Delete => match prev_change.change_type {
                    Some(change_msg::ChangeType::Add(prev_object)) => {
                        inverted.push(ChangeMsg {
                            user: String::from(user),
                            change_type: Some(change_msg::ChangeType::Add(prev_object)),
                            change_source: Some(source),
                        });
                    }
                    Some(change_msg::ChangeType::Modify(prev_object)) => {
                        inverted.push(ChangeMsg {
                            user: String::from(user),
                            change_type: Some(change_msg::ChangeType::Add(prev_object)),
                            change_source: Some(source),
                        });
                    }
                    Some(change_msg::ChangeType::Delete(..)) => {
                        error!("Object got deleted twice");
                    }
                    None => {
                        error!("No data to undo back to");
                    }
                },
                UndoChangeType::NotSet => match prev_change.change_type {
                    Some(change_msg::ChangeType::Add(prev_object)) => {
                        inverted.push(ChangeMsg {
                            user: String::from(user),
                            change_type: Some(change_msg::ChangeType::Add(prev_object)),
                            change_source: Some(source),
                        });
                    }
                    Some(change_msg::ChangeType::Modify(prev_object)) => {
                        inverted.push(ChangeMsg {
                            user: String::from(user),
                            change_type: Some(change_msg::ChangeType::Add(prev_object)),
                            change_source: Some(source),
                        });
                    }
                    Some(change_msg::ChangeType::Delete(..)) => {
                        error!("Object not set after a delete");
                    }
                    None => {
                        error!("No data to undo back to");
                    }
                },
            },
            None => {
                inverted.push(ChangeMsg {
                    user: String::from(user),
                    change_type: Some(change_msg::ChangeType::Delete(DeleteMsg {
                        id: current.obj_id,
                    })),
                    change_source: Some(source),
                });
            }
        }
    }
    inverted
}

pub async fn invert_changes(
    obj_client: &mut ObjClient,
    file: &str,
    user: &str,
    source: change_msg::ChangeSource,
    entries: Vec<UndoEntry>,
) -> Result<Vec<ChangeMsg>, Status> {
    let previous = get_all_previous_objects(obj_client, file, &entries).await?;
    info!("Got previous: {:?}", previous);
    Ok(invert_changes_inner(user, source, entries, previous))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invert_changes_inner() {
        let user_1 = String::from("test user");
        let user_2 = String::from("prev user");
        let source = change_msg::ChangeSource::Undo(String::from("test event"));
        let obj_1 = String::from("obj_1");
        let obj_2 = String::from("obj_2");
        let obj_3 = String::from("obj_3");
        let entries = vec![
            UndoEntry {
                obj_id: obj_1.clone(),
                offset: 1,
                change_type: UndoChangeType::Add,
            },
            UndoEntry {
                obj_id: obj_2.clone(),
                offset: 2,
                change_type: UndoChangeType::Modify,
            },
            UndoEntry {
                obj_id: obj_3.clone(),
                offset: 3,
                change_type: UndoChangeType::Delete,
            },
        ];
        let previous = vec![
            OptionChangeMsg { change: None },
            OptionChangeMsg {
                change: Some(ChangeMsg {
                    user: user_2.clone(),
                    change_type: Some(change_msg::ChangeType::Add(ObjectMsg {
                        id: obj_2.clone(),
                        dependencies: None,
                        obj_data: String::from("Object 2 data").into_bytes(),
                    })),
                    change_source: Some(change_msg::ChangeSource::UserAction(EmptyMsg {})),
                }),
            },
            OptionChangeMsg {
                change: Some(ChangeMsg {
                    user: user_2.clone(),
                    change_type: Some(change_msg::ChangeType::Modify(ObjectMsg {
                        id: obj_3.clone(),
                        dependencies: None,
                        obj_data: String::from("Object 3 data").into_bytes(),
                    })),
                    change_source: Some(change_msg::ChangeSource::UserAction(EmptyMsg {})),
                }),
            },
        ];
        let inverted = invert_changes_inner(&user_1, source.clone(), entries, previous);
        let answers = vec![
            ChangeMsg {
                user: user_1.clone(),
                change_type: Some(change_msg::ChangeType::Delete(DeleteMsg { id: obj_1 })),
                change_source: Some(source.clone()),
            },
            ChangeMsg {
                user: user_1.clone(),
                change_type: Some(change_msg::ChangeType::Modify(ObjectMsg {
                    id: obj_2.clone(),
                    dependencies: None,
                    obj_data: String::from("Object 2 data").into_bytes(),
                })),
                change_source: Some(source.clone()),
            },
            ChangeMsg {
                user: user_1.clone(),
                change_type: Some(change_msg::ChangeType::Add(ObjectMsg {
                    id: obj_3.clone(),
                    dependencies: None,
                    obj_data: String::from("Object 3 data").into_bytes(),
                })),
                change_source: Some(source.clone()),
            },
        ];
        //Splitting up these asserts so we zero in on which one is wrong
        assert_eq!(inverted[0], answers[0]);
        assert_eq!(inverted[1], answers[1]);
        assert_eq!(inverted[2], answers[2]);
    }
}
