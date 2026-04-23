use std::collections::BTreeMap;
use std::ops::Bound::*; 

use ahash::AHashMap;
use serde::{Deserialize, Serialize};
use slab::Slab;

use crate::api::{event::MatcherEvent, *};

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
    next: Option<OrderIdx>, // toward tail
    prev: Option<OrderIdx>, // toward head
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

impl Bucket {
    pub fn new(price: Price, tail: OrderIdx, volume: Size) -> Self {
        Self {
            price,
            volume,
            num_orders: 1,
            tail,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectOrderBook {
    symbol_spec: CoreSymbolSpecification,
    trade_seq: u64,

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
    // gtc good util cancel
    pub fn place_gtc(&mut self, cmd: &mut OrderCommand) {
        // 重复订单直接拒绝
        if self.order_id_index.contains_key(&cmd.order_id) {
            cmd.matcher_events
                .push(MatcherEvent::new_reject(cmd.size, cmd.price));
            cmd.result_code = CmdResultCode::MatchingDuplicateOrderId;
            return
        }

        let filled = self.try_match(cmd);
        // 部分成交，剩余挂单
        if filled < cmd.size {
            let order_idx = self.orders.insert(DirectOrder { 
                order_id: cmd.order_id, 
                uid: cmd.uid, price: cmd.price, 
                size: cmd.size, 
                filled, 
                action: cmd.action, 
                reserve_price: cmd.reserve_price, 
                timestamp_ms: cmd.timestamp_ms, 
                next: None, prev: None, parent: 0 
            });
            self.order_id_index.insert(cmd.order_id, order_idx);
            self.insert_order(order_idx);

        }

    }


    pub fn insert_order(&mut self, order_idx: OrderIdx) {
        let (price, is_ask) = {
            let order = &self.orders[order_idx];
            (order.price, order.action.is_ask())
        };
        let buckets_map =   if is_ask { &mut self.ask_price_buckets} else {&mut self.bid_price_buckets};
      
        let order_size = self.orders[order_idx].size - self.orders[order_idx].filled;
        if let Some(&bucket_idx) = buckets_map.get(&price) {
            // 将新数据放到桶的尾部
            let old_tail = self.buckets[bucket_idx].tail;
            let next_of_old_tail = self.orders[old_tail].next;
            self.orders[old_tail].next = Some(order_idx);

            self.orders[order_idx].next = next_of_old_tail;
            self.orders[order_idx].prev = Some(old_tail);
            self.orders[order_idx].parent = bucket_idx;

            if let Some(next_idx) = next_of_old_tail {
                self.orders[next_idx].prev = Some(order_idx);
            }
            
            self.buckets[bucket_idx].tail = order_idx;
            self.buckets[bucket_idx].volume += order_size;
            self.buckets[bucket_idx].num_orders += 1;

            return
        } else {
            // 创建新的桶时，更改前后桶的next指针，让前后按照next串起来
            let bucket_idx = self.buckets.insert(Bucket::new(
                price,
                order_idx,
                order_size,
            ));
            buckets_map.insert(price, bucket_idx);
            self.orders[order_idx].parent = bucket_idx;

            // 如果是ask需要升序，bid需要降序， ask: prev -> next, 变为 prev-> new -> next
            // ask的prev是价格升序的前一个，bid的prev是价格降序的前一个
            // 要是没找到前一个，则说明当前就是Best_price，通过best_price来更新 next
            let next_bucket_idx = if is_ask {
                buckets_map.range(..price).last().map(|(_, &bucket_idx)| bucket_idx)

            } else {
                buckets_map.range((Excluded(&price), Unbounded)).next().map(|(_, &bucket_idx)| bucket_idx)
            };
            if let Some(next_bucket_idx) = next_bucket_idx {
                let prev_order_idx = self.buckets[next_bucket_idx].tail;
                let old_next_order_idx = self.orders[prev_order_idx].next;

                self.orders[order_idx].prev = Some(prev_order_idx);
                self.orders[order_idx].next = old_next_order_idx;

                self.orders[prev_order_idx].next = Some(order_idx);

                if let Some(old_next_order_idx) = old_next_order_idx {
                    self.orders[old_next_order_idx].prev = Some(order_idx);
                }
            } else {
                if let Some(next_order_idx) = if is_ask { self.best_ask_order} else {self.best_bid_order} {
                    self.orders[next_order_idx].prev = Some(order_idx);
                    self.orders[order_idx].next = Some(next_order_idx);
                }

                if is_ask {
                    self.best_ask_order = Some(order_idx);
                } else {
                    self.best_bid_order = Some(order_idx);
                }
            }
        }

    }

    pub fn remove_order(&mut self, order_idx: OrderIdx) {
        // 需要先断开关系最后再删除order
        // 如果是best_price，需要更新best_price
        // 如果bucket没有单子了需要删除bucket
    }

    pub fn try_match(&mut self, cmd: &mut OrderCommand) -> Size{
        0

    }
}

impl super::Orderbook for DirectOrderBook {
    fn new_order(&mut self, cmd: &mut OrderCommand) -> CmdResultCode {


        match cmd.order_type {
            OrderType::GTC => {
                self.place_gtc(cmd);

                CmdResultCode::Success
            }
            _ => {
                CmdResultCode::MatchingUnsupportedCommand
            }
        }
    }

    fn serialize_state(&self) -> super::OrderBookState {
        super::OrderBookState::Direct(self.clone())
    }
}
