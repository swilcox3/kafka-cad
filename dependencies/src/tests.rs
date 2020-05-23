//! The dependency graph is versioned along with the model state.  Each RefIdMsg is treated as a publisher with a subscribers table.  The set key points to a set of GeometryIDs that depend on the first RefIdMsg.
//! When that working set is submitted, we serialize each dep set and attach it as a blob mapped to the publisher's RefIdMsg in the history.  The history then stores
//! a hash of GeometryIDs to Vec<RefIdMsg>.
use super::*;
use change_msg::ChangeType;
use prost::Message;
use redis::aio::MultiplexedConnection;
use ref_id_msg::RefType;
use std::collections::HashSet;
use uuid::Uuid;

#[macro_export]
macro_rules! set {
        ( $( $x:expr ),* ) => {  // Match zero or more comma delimited items
            {
                let mut temp_set = HashSet::new();  // Create a mutable HashSet
                $(
                    temp_set.insert($x); // Insert each item matched into the HashSet
                )*
                temp_set // Return the populated HashSet
            }
        };
    }

async fn test_get_conn() -> MultiplexedConnection {
    let _ = env_logger::Builder::new()
        .filter_module("dependencies", log::LevelFilter::Trace)
        .is_test(true)
        .try_init();
    let env_opt = std::env::var("REDIS_URL");
    let redis_url = if let Ok(url) = env_opt {
        url
    } else {
        String::from("redis://127.0.0.1:6379")
    };
    let client = redis::Client::open(redis_url).unwrap();
    let (conn, fut) = client.get_multiplexed_async_connection().await.unwrap();
    tokio::spawn(fut);
    conn
}

fn ref_id_msg(id: String, ref_type: RefType, index: u64) -> RefIdMsg {
    RefIdMsg {
        id,
        ref_type: ref_type as i32,
        index,
    }
}

fn ref_msg(owner: &RefIdMsg, other: &RefIdMsg) -> ReferenceMsg {
    ReferenceMsg {
        owner: Some(owner.clone()),
        other: Some(other.clone()),
    }
}

fn ref_msg_bin(owner: &RefIdMsg, other: &RefIdMsg) -> Vec<u8> {
    let msg = ref_msg(owner, other);
    let mut encoded = Vec::new();
    msg.encode(&mut encoded).unwrap();
    encoded
}

fn opt_ref_msg(owner: &RefIdMsg, other: &RefIdMsg) -> OptionReferenceMsg {
    OptionReferenceMsg {
        reference: Some(ref_msg(owner, other)),
    }
}

fn add_change_msg(id: String, references: Vec<OptionReferenceMsg>) -> Vec<u8> {
    let msg = ChangeMsg {
        user: "Doesn't matter".to_string(),
        change_type: Some(ChangeType::Add(ObjectMsg {
            id,
            dependencies: Some(DependenciesMsg { references }),
            obj_data: Vec::new(),
        })),
    };
    let mut bytes = Vec::new();
    msg.encode(&mut bytes).unwrap();
    bytes
}

fn modify_change_msg(id: String, references: Vec<OptionReferenceMsg>) -> Vec<u8> {
    let msg = ChangeMsg {
        user: "Doesn't matter".to_string(),
        change_type: Some(ChangeType::Modify(ObjectMsg {
            id,
            dependencies: Some(DependenciesMsg { references }),
            obj_data: Vec::new(),
        })),
    };
    let mut bytes = Vec::new();
    msg.encode(&mut bytes).unwrap();
    bytes
}

fn delete_change_msg(id: String) -> Vec<u8> {
    let msg = ChangeMsg {
        user: "Doesn't matter".to_string(),
        change_type: Some(ChangeType::Delete(DeleteMsg { id })),
    };
    let mut bytes = Vec::new();
    msg.encode(&mut bytes).unwrap();
    bytes
}

fn set_exists_within_range(
    mut set: HashSet<Vec<u8>>,
    base: &Vec<ReferenceMsg>,
    index: usize,
    size: usize,
) -> bool {
    for i in index..index + size {
        let entry = base.get(i).unwrap();
        let mut encoded = Vec::new();
        entry.encode(&mut encoded).unwrap();
        set.remove(&encoded);
    }
    if set.len() > 0 {}
    set.len() == 0
}

fn equals(input: Vec<ReferenceMsg>, answers: Vec<HashSet<Vec<u8>>>) -> bool {
    let mut cur_index = 0;
    for set in answers {
        let size = set.len();
        if !set_exists_within_range(set, &input, cur_index, size) {
            return false;
        }
        cur_index += size;
    }
    true
}

#[tokio_macros::test]
async fn test_deps_simple() {
    let file = Uuid::new_v4().to_string();
    let mut conn = test_get_conn().await;

    let obj_0_id = Uuid::new_v4().to_string();
    let obj_0_pt_0 = ref_id_msg(obj_0_id.clone(), RefType::ProfilePoint, 0);
    let obj_0_pt_1 = ref_id_msg(obj_0_id.clone(), RefType::ProfilePoint, 1);
    let obj_1_id = Uuid::new_v4().to_string();
    log::trace!("obj_0: {:?}", obj_0_id);
    log::trace!("obj_1: {:?}", obj_1_id);
    let obj_1_pt_0 = ref_id_msg(obj_1_id.clone(), RefType::ProfilePoint, 0);
    let obj_1_pt_1 = ref_id_msg(obj_1_id.clone(), RefType::ProfilePoint, 1);
    let obj_0 = add_change_msg(
        obj_0_id.clone(),
        vec![opt_ref_msg(&obj_0_pt_0, &obj_1_pt_1)],
    );
    let obj_1 = add_change_msg(
        obj_1_id.clone(),
        vec![
            opt_ref_msg(&obj_1_pt_1, &obj_0_pt_0),
            opt_ref_msg(&obj_1_pt_0, &obj_0_pt_1),
            opt_ref_msg(&obj_1_pt_0, &obj_1_pt_1),
        ],
    );

    update_deps(&mut conn, &file, 0, &obj_0).await.unwrap();

    update_deps(&mut conn, &file, 1, &obj_1).await.unwrap();

    let results = get_all_deps(&mut conn, &file, 1, &vec![obj_0_pt_1.clone()])
        .await
        .unwrap();

    assert!(equals(
        results,
        vec![set![ref_msg_bin(&obj_1_pt_0, &obj_0_pt_1)]]
    ));

    let results = get_all_deps(&mut conn, &file, 1, &vec![obj_0_pt_0.clone()])
        .await
        .unwrap();

    assert!(equals(
        results,
        vec![
            set![ref_msg_bin(&obj_1_pt_1, &obj_0_pt_0)],
            set![ref_msg_bin(&obj_1_pt_0, &obj_1_pt_1)]
        ]
    ));
}

#[tokio_macros::test]
async fn test_deps_versioned() {
    let file = Uuid::new_v4().to_string();
    let mut conn = test_get_conn().await;

    let obj_0_id = Uuid::new_v4().to_string();
    let obj_0_pt_0 = ref_id_msg(obj_0_id.clone(), RefType::ProfilePoint, 0);
    let obj_0_pt_1 = ref_id_msg(obj_0_id.clone(), RefType::ProfilePoint, 1);
    let obj_1_id = Uuid::new_v4().to_string();
    let obj_1_pt_0 = ref_id_msg(obj_1_id.clone(), RefType::ProfilePoint, 0);
    let obj_1_pt_1 = ref_id_msg(obj_1_id.clone(), RefType::ProfilePoint, 1);

    log::trace!("obj_0: {:?}", obj_0_id);
    log::trace!("obj_1: {:?}", obj_1_id);

    let obj_0 = add_change_msg(
        obj_0_id.clone(),
        vec![opt_ref_msg(&obj_0_pt_1, &obj_1_pt_0)],
    );
    let obj_1 = add_change_msg(
        obj_1_id.clone(),
        vec![opt_ref_msg(&obj_1_pt_1, &obj_0_pt_0)],
    );

    update_deps(&mut conn, &file, 0, &obj_0).await.unwrap();
    update_deps(&mut conn, &file, 1, &obj_1).await.unwrap();
    let obj_1 = modify_change_msg(
        obj_1_id.clone(),
        vec![
            opt_ref_msg(&obj_1_pt_0, &obj_0_pt_1),
            opt_ref_msg(&obj_1_pt_0, &obj_1_pt_1),
        ],
    );
    update_deps(&mut conn, &file, 2, &obj_1).await.unwrap();

    let results = get_all_deps(&mut conn, &file, 2, &vec![obj_0_pt_0.clone()])
        .await
        .unwrap();
    trace!("results: {:#?}", results);

    assert!(equals(
        results,
        vec![
            set![ref_msg_bin(&obj_1_pt_1, &obj_0_pt_0)],
            set![ref_msg_bin(&obj_1_pt_0, &obj_1_pt_1)]
        ]
    ));

    let obj_0 = delete_change_msg(obj_0_id.clone());
    update_deps(&mut conn, &file, 3, &obj_0).await.unwrap();

    let results = get_all_deps(&mut conn, &file, 3, &vec![obj_0_pt_0.clone()])
        .await
        .unwrap();
    assert!(equals(results, vec![]));

    let results = get_all_deps(&mut conn, &file, 3, &vec![obj_1_pt_1.clone()])
        .await
        .unwrap();
    assert!(equals(
        results,
        vec![set![ref_msg_bin(&obj_1_pt_0, &obj_1_pt_1)]]
    ));
}

#[tokio_macros::test]
async fn test_get_all_deps() {
    let file = Uuid::new_v4().to_string();
    let mut conn = test_get_conn().await;

    let wall_0_id = Uuid::new_v4().to_string();
    let wall_0_pt_1 = ref_id_msg(wall_0_id.clone(), RefType::ProfilePoint, 1);
    let wall_1_id = Uuid::new_v4().to_string();
    let wall_1_pt_0 = ref_id_msg(wall_1_id.clone(), RefType::ProfilePoint, 0);
    let wall_1_pt_1 = ref_id_msg(wall_1_id.clone(), RefType::ProfilePoint, 1);
    let wall_1_line_0 = ref_id_msg(wall_1_id.clone(), RefType::ProfileLine, 0);
    let wall_1_rect_0 = ref_id_msg(wall_1_id.clone(), RefType::ProfilePlane, 0);
    let window_id = Uuid::new_v4().to_string();
    let window_pt_0 = ref_id_msg(window_id.clone(), RefType::ProfilePoint, 0);
    let window_pt_1 = ref_id_msg(window_id.clone(), RefType::ProfilePoint, 1);
    let dim_0_id = Uuid::new_v4().to_string();
    let dim_0_pt_0 = ref_id_msg(dim_0_id.clone(), RefType::ProfilePoint, 0);
    let dim_0_pt_1 = ref_id_msg(dim_0_id.clone(), RefType::ProfilePoint, 1);
    let dim_1_id = Uuid::new_v4().to_string();
    let dim_1_pt_0 = ref_id_msg(dim_1_id.clone(), RefType::ProfilePoint, 0);
    let dim_1_pt_1 = ref_id_msg(dim_1_id.clone(), RefType::ProfilePoint, 1);

    log::trace!("wall 0: {:?}", wall_0_id);
    log::trace!("wall 1: {:?}", wall_1_id);
    log::trace!("window: {:?}", window_id);
    log::trace!("dim 0: {:?}", dim_0_id);
    log::trace!("dim 1: {:?}", dim_1_id);

    let wall_0 = add_change_msg(
        wall_0_id.clone(),
        vec![opt_ref_msg(&wall_0_pt_1, &wall_1_pt_0)],
    );

    let wall_1 = add_change_msg(
        wall_1_id.clone(),
        vec![
            opt_ref_msg(&wall_1_pt_0, &wall_0_pt_1),
            opt_ref_msg(&wall_1_line_0, &wall_1_pt_0),
            opt_ref_msg(&wall_1_line_0, &wall_1_pt_1),
            opt_ref_msg(&wall_1_rect_0, &window_pt_0),
        ],
    );

    let dim_0 = add_change_msg(
        dim_0_id.clone(),
        vec![
            opt_ref_msg(&dim_0_pt_0, &wall_1_pt_0),
            opt_ref_msg(&dim_0_pt_1, &wall_1_rect_0),
        ],
    );

    let dim_1 = add_change_msg(
        dim_1_id.clone(),
        vec![
            opt_ref_msg(&dim_1_pt_0, &wall_1_pt_0),
            opt_ref_msg(&dim_1_pt_1, &wall_1_pt_1),
        ],
    );

    let window = add_change_msg(
        window_id.clone(),
        vec![
            opt_ref_msg(&window_pt_0, &wall_1_line_0),
            opt_ref_msg(&window_pt_1, &wall_1_line_0),
        ],
    );
    update_deps(&mut conn, &file, 0, &wall_0).await.unwrap();
    update_deps(&mut conn, &file, 1, &wall_1).await.unwrap();
    update_deps(&mut conn, &file, 2, &dim_0).await.unwrap();
    update_deps(&mut conn, &file, 3, &dim_1).await.unwrap();
    update_deps(&mut conn, &file, 4, &window).await.unwrap();

    let results = get_all_deps(&mut conn, &file, 4, &vec![wall_0_pt_1.clone()])
        .await
        .unwrap();
    log::info!("Got results: {:#?}", results);
    assert!(equals(
        results,
        vec![
            set![ref_msg_bin(&wall_1_pt_0, &wall_0_pt_1)],
            set![
                ref_msg_bin(&dim_0_pt_0, &wall_1_pt_0),
                ref_msg_bin(&dim_1_pt_0, &wall_1_pt_0),
                ref_msg_bin(&wall_1_line_0, &wall_1_pt_0)
            ],
            set![
                ref_msg_bin(&window_pt_0, &wall_1_line_0),
                ref_msg_bin(&window_pt_1, &wall_1_line_0)
            ],
            set![ref_msg_bin(&wall_1_rect_0, &window_pt_0)],
            set![ref_msg_bin(&dim_0_pt_1, &wall_1_rect_0)],
        ]
    ));

    let results = get_all_deps(&mut conn, &file, 4, &vec![window_pt_0.clone()])
        .await
        .unwrap();
    log::info!("Got results: {:#?}", results);
    assert!(equals(
        results,
        vec![
            set![ref_msg_bin(&wall_1_rect_0, &window_pt_0)],
            set![ref_msg_bin(&dim_0_pt_1, &wall_1_rect_0)]
        ]
    ));

    let results = get_all_deps(&mut conn, &file, 0, &vec![window_pt_0.clone()])
        .await
        .unwrap();
    log::info!("Got results: {:#?}", results);
    assert!(equals(results, vec![]));
}
