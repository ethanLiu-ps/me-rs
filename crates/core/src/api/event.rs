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
    // maker 被 STP 取消（携带 maker 信息）
    CancelMaker,
 }

// 撮合事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatcherEvent {
    pub event_type: MatcherEventType,
    pub size: Size,
    pub price: Price,
    pub trade_id: u64,
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
            trade_id: 0,
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
            trade_id: 0,
            bidder_hold_price: 0,
            maker_order_id: 0,
            maker_uid: 0,
        }
    }

    pub fn new_maker_cancel(size: Size, price: Price, maker_order_id: OrderId, maker_uid: UserId) -> Self {
        Self {
            event_type: MatcherEventType::CancelMaker,
            size,
            price,
            trade_id: 0,
            bidder_hold_price: 0,
            maker_order_id,
            maker_uid,
        }
    }
}
