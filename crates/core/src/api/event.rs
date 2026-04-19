use serde::{Deserialize, Serialize};

use super::types::{OrderId, Price, Size, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatcherEventType {
    // 撮合成交
    Trade,
    // 撮合拒绝
    Reject,
    // 撮合减量
    Reduce,
}

// 撮合事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatcherEvent {
    pub event_type: MatcherEventType,
    pub size: Size,
    pub price: Price,
    // 买单预留价格
    pub bidder_hold_price: Price,
    pub maker_order_id: OrderId,
    pub maker_uid: UserId,
}

impl Default for MatcherEvent {
    fn default() -> Self {
        Self {
            event_type: MatcherEventType::Trade,
            size: 0,
            price: 0,
            bidder_hold_price: 0,
            maker_order_id: 0,
            maker_uid: 0,
        }
    }
}

impl MatcherEvent {
    pub fn new_reject(size: Size, price: Price) -> Self {
        Self {
            event_type: MatcherEventType::Reject,
            size,
            price,
            bidder_hold_price: 0,
            maker_order_id: 0,
            maker_uid: 0,
        }
    }
}
