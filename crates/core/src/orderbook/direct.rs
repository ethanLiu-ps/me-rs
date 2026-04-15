
use std::collections::BTreeMap;

use serde::{Serialize, Deserialize};
use slab::{Slab};
use ahash::{AHashMap};

use crate::api::{*};

type OrderIdx = usize;
type BucketIdx = usize;

/// 直接订单（使用 Slab 索引实现的双向链表，避免 Rc/RefCell 开销）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DirectOrder {
    order_id: OrderId,
    uid: UserId,
    price: Price,
    size: Size,
    filled: Size,
    action: OrderAction,
    reserve_price: Price,
    timestamp_ms: i64,
    next: Option<OrderIdx>,
    prev: Option<OrderIdx>,
    parent: BucketIdx,
}

/// 价格档位（桶），存储相同价格的一组订单
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Bucket {
    price: Price,
    volume: Size,
    num_orders: usize,
    tail: OrderIdx,
}






#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectOrderBook {
    symbol_spec: CoreSymbolSpecification,


    // 内存池，预分配订单和桶，减少分配开销
    orders: Slab<DirectOrder>,
    buckets: Slab<Bucket>,

    // 价格索引，快速定位最优价格
    ask_price_buckets: BTreeMap<Price, BucketIdx>, // 卖单 价格升序
    bid_price_buckets: BTreeMap<Price, BucketIdx>, // 买单 价格降序

    // 订单ID快速索引
    order_id_index: AHashMap<OrderId, OrderIdx>,

    // 最优订单快捷引用，类似 LMAX Disruptor 的快速路径
    best_ask_order: Option<OrderIdx>,
    best_bid_order: Option<OrderIdx>,
    
}


impl DirectOrderBook {
    pub fn place_gtc(&mut self, cmd: &mut OrderCommand) -> CmdResultCode {

        CmdResultCode::Success
    }
}


impl super::Orderbook for DirectOrderBook {
    fn new_order(&mut self, cmd: &mut OrderCommand) -> CmdResultCode {
        match cmd.order_type {
            OrderType::GTC => {
                return self.place_gtc(cmd);
            },
            _ => {
                return CmdResultCode::MatchingUnsupportedCommand;
            }
        }

    }


    fn serialize_state(&self) -> super::OrderBookState {
        super::OrderBookState::Direct(self.clone())
    }

}