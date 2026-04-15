use serde::{Deserialize, Serialize};
use super::types::{*};
use super::event::{MatcherEvent};


#[derive(Debug, Clone, Copy, PartialEq, Eq,Serialize, Deserialize)]
pub enum OrderCmdType {
    // order
    PlaceOrder,       // 下单（进入风控→撮合→结算流程）
    MoveOrder,        // 移单（改价/改量，语义等同撤旧下新）
    CancelOrder,      // 撤单
    ReduceOrder,      // 减量（缩减挂单剩余数量）
    OrderBookRequest, // 查询订单簿状态（L2 行情等）

    // risk

    // system
    Reset,                // 重置引擎状态
    None,                  // 空操作（心跳/占位）
    PersistStateMatching, // 触发撮合引擎状态持久化
    PersistStateRisk,     // 触发风控引擎状态持久化
    GroupingControl,      // 分组控制（批量命令边界标记）
    ShutdownSignal,       // 关闭信号
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCommand {
    pub command: OrderCmdType,
    pub result_code: CmdResultCode,
    pub uid: UserId,
    pub order_id: OrderId,
    pub symbol: SymbolId,
    pub price: Price,
    pub reserve_price: Price,
    pub size: Size,
    pub action: OrderAction,
    pub order_type: OrderType,
    pub timestamp_ms: i64,
    pub events_group: u64,
    pub service_flags: i32,
    pub stop_price: Option<Price>,
    pub visible_size: Option<Size>,
    pub expire_time_ms: Option<i64>,
    pub matcher_events: Vec<MatcherEvent>

}

impl Default for OrderCommand {
    fn default() -> Self {
        Self {
            command: OrderCmdType::None,
            result_code: CmdResultCode::New,
            uid: 0,
            order_id: 0,
            symbol: String::new(),
            price: 0,
            reserve_price: 0,
            size: 0,
            action: OrderAction::Bid,
            order_type: OrderType::GTC,
            timestamp_ms: 0,
            events_group: 0,
            service_flags: 0,
            stop_price: None,
            visible_size: None,
            expire_time_ms: None,
            matcher_events: Vec::with_capacity(4),
        }
    }
}