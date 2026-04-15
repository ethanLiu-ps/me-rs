
mod direct;


use serde::{Serialize, Deserialize};

use crate::api::command::OrderCommand;
use crate::api::types::CmdResultCode;
use direct::DirectOrderBook;

pub trait Orderbook {
    // 按订单类型(GTC/IOC/FOK等)撮合：先尝试与对手方成交，未成交部分挂单或拒绝，结果写入 cmd.matcher_events
    fn new_order(&mut self, cmd: &mut OrderCommand) -> CmdResultCode;


    fn serialize_state(&self) -> OrderBookState;
}



#[derive(Serialize, Deserialize)]
pub enum OrderBookState {
    Direct(DirectOrderBook),
}
