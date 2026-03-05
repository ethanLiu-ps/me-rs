use serde::{Deserialize, Serialize};

pub type Size = i64;
pub type Price = i64;
pub type UserId = u64;
pub type OrderId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderAction {
    // 买
    Bid,
    // 卖
    Ask,
}

impl OrderAction {
    pub fn opposite(self) -> OrderAction {
        match self {
            OrderAction::Bid => OrderAction::Ask,
            OrderAction::Ask => OrderAction::Bid,
        }
    }
}

pub enum OrderType {
    // good-til-cancelled
    GTC,
    // immediate-or-cancel
    IOC,
    // fill-or-kill
    FOK,
    // post only, 只做maker，不吃单
    PostOnly,
    // 止损限价单
    StopLimit,
}
