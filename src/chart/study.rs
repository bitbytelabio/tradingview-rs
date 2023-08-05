use crate::{
    chart::Interval,
    prelude::*,
    socket::{DataServer, SocketMessage},
    utils::{format_packet, gen_id, gen_session_id, parse_packet},
    UA,
};

struct Study {
    id: String,
}
