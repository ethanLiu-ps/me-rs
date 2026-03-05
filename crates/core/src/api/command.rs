use serde::{Deserialize, Serialize};


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
    Nop,                  // 空操作（心跳/占位）
    PersistStateMatching, // 触发撮合引擎状态持久化
    PersistStateRisk,     // 触发风控引擎状态持久化
    GroupingControl,      // 分组控制（批量命令边界标记）
    ShutdownSignal,       // 关闭信号
}
