//! The dependency graph is versioned along with the model state.  Each RefIdMsg is treated as a publisher with a subscribers table.  The set key points to a set of GeometryIDs that depend on the first RefIdMsg.
//! When that working set is submitted, we serialize each dep set and attach it as a blob mapped to the publisher's RefIdMsg in the history.  The history then stores
//! a hash of GeometryIDs to Vec<RefIdMsg>.
use super::*;
use change_msg::ChangeType;
use indexmap::IndexSet;
use log::*;
use prost::Message;
use redis::aio::MultiplexedConnection;
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
        .filter_module("model_state", log::LevelFilter::Trace)
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

fn ref_id_msg(id: String, index: u64) -> RefIdMsg {
    RefIdMsg {
        id,
        ref_type: 3,
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
        id,
        change_type: Some(ChangeType::Add(ObjectMsg {
            dependencies: Some(DependenciesMsg { references }),
            results: None,
            obj_data: Vec::new(),
        })),
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
        //println!("removing {:?}", entry);
        set.remove(&encoded);
    }
    if set.len() > 0 {
        //println!("left {:?}", set);
    }
    //println!("");
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
    let offset = 0;
    let max_len = 5;

    let obj_0_id = Uuid::new_v4().to_string();
    let obj_0_pt_0 = ref_id_msg(obj_0_id.clone(), 0);
    let obj_0_pt_1 = ref_id_msg(obj_0_id.clone(), 1);
    let obj_1_id = Uuid::new_v4().to_string();
    trace!("obj_0: {:?}", obj_0_id);
    trace!("obj_1: {:?}", obj_1_id);
    let obj_1_pt_0 = ref_id_msg(obj_1_id.clone(), 0);
    let obj_1_pt_1 = ref_id_msg(obj_1_id.clone(), 1);
    let obj_1 = add_change_msg(
        obj_1_id.clone(),
        vec![
            opt_ref_msg(&obj_1_pt_1, &obj_0_pt_0),
            opt_ref_msg(&obj_1_pt_0, &obj_0_pt_1),
            opt_ref_msg(&obj_1_pt_0, &obj_1_pt_1),
        ],
    );

    update_deps(&mut conn, &file, offset, &obj_1, max_len)
        .await
        .unwrap();

    let results = get_all_deps(&mut conn, &file, offset, &vec![obj_0_pt_1.clone()])
        .await
        .unwrap();

    assert!(equals(
        results,
        vec![set![ref_msg_bin(&obj_1_pt_0, &obj_0_pt_1)]]
    ));

    /*let results = get_all_deps_inner(&mut conn, &file, &op_id, 0, vec![obj_0_pt_0.clone()])
        .await
        .unwrap();

    assert!(equals(
        results,
        vec![
            set![get_ref(&obj_1_pt_1, &obj_0_pt_0)],
            set![get_ref(&obj_1_pt_0, &obj_1_pt_1)]
        ]
    ));*/
}

/*#[tokio_macros::test]
async fn test_deps_versioned() {
    let file = Uuid::new_v4().to_string();
    let op_id_1 = OperationID::new_v4().to_string();
    let mut conn = test_get_conn().await;

    let obj_0 = Uuid::new_v4().to_string();
    let obj_0_pt_0 = ref_id_msg(obj_0.clone(), 0);
    let obj_0_pt_1 = ref_id_msg(obj_0.clone(), 1);
    let obj_1 = Uuid::new_v4().to_string();
    let obj_1_pt_0 = ref_id_msg(obj_1.clone(), 0);
    let obj_1_pt_1 = ref_id_msg(obj_1.clone(), 1);

    trace!("obj_0: {:?}", obj_0);
    trace!("obj_1: {:?}", obj_1);

    set_pub_sub(
        &mut conn,
        &file,
        &op_id_1,
        &obj_0_pt_0,
        &DepChange::Add(obj_1_pt_1.clone()),
    )
    .await
    .unwrap();
    let submitted_key = working_set::prepare_dep_working_set_for_submit(&mut conn, &file, &op_id_1)
        .await
        .unwrap();
    let cur_change = objects::add_revision(&mut conn, &file, String::new(), submitted_key)
        .await
        .unwrap();
    dep_history::add_change_id_to_dep_history(&mut conn, &file, &obj_0_pt_0, cur_change)
        .await
        .unwrap();
    let op_id_2 = OperationID::new_v4().to_string();

    set_pub_sub(
        &mut conn,
        &file,
        &op_id_2,
        &obj_0_pt_1,
        &DepChange::Add(obj_1_pt_0.clone()),
    )
    .await
    .unwrap();

    set_pub_sub(
        &mut conn,
        &file,
        &op_id_2,
        &obj_1_pt_1,
        &DepChange::Add(obj_1_pt_0.clone()),
    )
    .await
    .unwrap();

    let results = get_all_deps_inner(
        &mut conn,
        &file,
        &op_id_2,
        cur_change,
        vec![obj_0_pt_0.clone()],
    )
    .await
    .unwrap();
    trace!("results: {:?}", results);

    assert!(equals(
        results,
        vec![
            set![get_ref(&obj_1_pt_1, &obj_0_pt_0)],
            set![get_ref(&obj_1_pt_0, &obj_1_pt_1)]
        ]
    ));
}

#[tokio_macros::test]
async fn test_get_all_deps() {
    let file = Uuid::new_v4().to_string();
    let op_id = OperationID::new_v4().to_string();
    let mut conn = test_get_conn().await;

    let wall_0 = Uuid::new_v4().to_string();
    let wall_0_pt_1 = ref_id_msg(wall_0, 1);
    let wall_1 = Uuid::new_v4().to_string();
    let wall_1_pt_0 = ref_id_msg(wall_1, 0);
    let wall_1_pt_1 = ref_id_msg(wall_1, 1);
    let wall_1_line_0 = ref_id_msg(wall_1, 2);
    let wall_1_rect_0 = ref_id_msg(wall_1, 3);
    let window = Uuid::new_v4().to_string();
    let window_pt_0 = ref_id_msg(window, 0);
    let window_pt_1 = ref_id_msg(window, 1);
    let dim_0 = Uuid::new_v4().to_string();
    let dim_0_pt_0 = ref_id_msg(dim_0, 0);
    let dim_0_pt_1 = ref_id_msg(dim_0, 1);
    let dim_1 = Uuid::new_v4().to_string();
    let dim_1_pt_0 = ref_id_msg(dim_1, 0);
    let dim_1_pt_1 = ref_id_msg(dim_1, 1);

    trace!("wall 0: {:?}", wall_0);
    trace!("wall 1: {:?}", wall_1);
    trace!("window: {:?}", window);
    trace!("dim 0: {:?}", dim_0);
    trace!("dim 1: {:?}", dim_1);

    set_pub_sub(
        &mut conn,
        &file,
        &op_id,
        &wall_0_pt_1,
        &DepChange::Add(wall_1_pt_0),
    )
    .await
    .unwrap();
    set_pub_sub(
        &mut conn,
        &file,
        &op_id,
        &wall_1_pt_0,
        &DepChange::Add(wall_0_pt_1),
    )
    .await
    .unwrap();
    set_pub_sub(
        &mut conn,
        &file,
        &op_id,
        &wall_1_pt_0,
        &DepChange::Add(dim_0_pt_0),
    )
    .await
    .unwrap();
    set_pub_sub(
        &mut conn,
        &file,
        &op_id,
        &wall_1_pt_0,
        &DepChange::Add(dim_1_pt_0),
    )
    .await
    .unwrap();
    set_pub_sub(
        &mut conn,
        &file,
        &op_id,
        &wall_1_pt_0,
        &DepChange::Add(wall_1_line_0),
    )
    .await
    .unwrap();
    set_pub_sub(
        &mut conn,
        &file,
        &op_id,
        &wall_1_pt_1,
        &DepChange::Add(wall_1_line_0),
    )
    .await
    .unwrap();
    set_pub_sub(
        &mut conn,
        &file,
        &op_id,
        &wall_1_pt_1,
        &DepChange::Add(dim_1_pt_1),
    )
    .await
    .unwrap();
    set_pub_sub(
        &mut conn,
        &file,
        &op_id,
        &wall_1_line_0,
        &DepChange::Add(window_pt_0),
    )
    .await
    .unwrap();
    set_pub_sub(
        &mut conn,
        &file,
        &op_id,
        &wall_1_line_0,
        &DepChange::Add(window_pt_1),
    )
    .await
    .unwrap();
    set_pub_sub(
        &mut conn,
        &file,
        &op_id,
        &wall_1_rect_0,
        &DepChange::Add(dim_0_pt_1),
    )
    .await
    .unwrap();
    set_pub_sub(
        &mut conn,
        &file,
        &op_id,
        &window_pt_0,
        &DepChange::Add(wall_1_rect_0),
    )
    .await
    .unwrap();

    let results = get_all_deps_inner(&mut conn, &file, &op_id, 0, vec![wall_0_pt_1])
        .await
        .unwrap();
    info!("Got results: {:?}", results);
    assert!(equals(
        results,
        vec![
            set![get_ref(&wall_1_pt_0, &wall_0_pt_1)],
            set![
                get_ref(&dim_0_pt_0, &wall_1_pt_0),
                get_ref(&dim_1_pt_0, &wall_1_pt_0),
                get_ref(&wall_1_line_0, &wall_1_pt_0)
            ],
            set![
                get_ref(&window_pt_0, &wall_1_line_0),
                get_ref(&window_pt_1, &wall_1_line_0)
            ],
            set![get_ref(&wall_1_rect_0, &window_pt_0)],
            set![get_ref(&dim_0_pt_1, &wall_1_rect_0)],
        ]
    ));

    let results = get_all_deps_inner(&mut conn, &file, &op_id, 0, vec![window_pt_0])
        .await
        .unwrap();
    assert!(equals(
        results,
        vec![
            set![get_ref(&wall_1_rect_0, &window_pt_0)],
            set![get_ref(&dim_0_pt_1, &wall_1_rect_0)]
        ]
    ));
}

#[tokio_macros::test]
async fn test_clear_deps() {
    let file = Uuid::new_v4().to_string();
    let op_id = OperationID::new_v4().to_string();
    let cur_change = 0u64;
    let mut obj_1 = TestObject::new("first");
    let obj_1_id = obj_1.get_id().clone();
    let mut obj_2 = TestObject::new("second");
    let obj_2_id = obj_2.get_id().clone();
    obj_1.set_ref(
        RefType::ProfilePoint,
        0,
        RefResult::Point(Point3f::new(0.0, 1.0, 1.0)),
        ref_id_msg(obj_2_id, 0),
        &None,
    );
    obj_2.set_ref(
        RefType::ProfilePoint,
        0,
        RefResult::Point(Point3f::new(2.0, 1.0, 1.0)),
        ref_id_msg(obj_1_id, 0),
        &None,
    );
    let mut conn = test_get_conn().await;
    let data_1 = Box::new(obj_1) as Box<dyn Data>;
    let data_2 = Box::new(obj_2) as Box<dyn Data>;
    working_set::add_obj_refs_to_working_set(&mut conn, &file, &op_id, cur_change, &data_1)
        .await
        .unwrap();
    working_set::add_obj_refs_to_working_set(&mut conn, &file, &op_id, cur_change, &data_2)
        .await
        .unwrap();
    let submitted_key = prepare_dep_working_set_for_submit(&mut conn, &file, &op_id)
        .await
        .unwrap();
    let dep_ids = get_all_dep_keys_from_key(&mut conn, &submitted_key)
        .await
        .unwrap();
    for dep_id in &dep_ids {
        add_change_id_to_dep_history(&mut conn, &file, dep_id, cur_change)
            .await
            .unwrap();
    }

    clear_dependencies(&mut conn, &file, &submitted_key)
        .await
        .unwrap();
    for dep_id in &dep_ids {
        let res =
            dep_history::get_closest_change_id_from_dep_history(&mut conn, &file, dep_id, 0).await;
        assert!(res.is_err());
    }
    let exists: bool = conn.exists(submitted_key).await.unwrap();
    assert!(!exists);
}*/
