mod geom {
    include!(concat!(env!("OUT_DIR"), "/geom.rs"));
}

pub use geom::*;
