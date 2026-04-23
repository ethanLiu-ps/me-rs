use super::*;

fn make_book() -> DirectOrderBook {
    DirectOrderBook {
        symbol_spec: CoreSymbolSpecification::default(),
        trade_seq: 0,
        orders: Slab::new(),
        buckets: Slab::new(),
        ask_price_buckets: BTreeMap::new(),
        bid_price_buckets: BTreeMap::new(),
        order_id_index: AHashMap::default(),
        best_ask_order: None,
        best_bid_order: None,
    }
}

fn bid(order_id: OrderId, uid: UserId, price: Price, size: Size) -> DirectOrder {
    DirectOrder {
        order_id,
        uid,
        price,
        size,
        filled: 0,
        action: OrderAction::Bid,
        reserve_price: price,
        timestamp_ms: 1_000_000,
        next: None,
        prev: None,
        parent: 0,
    }
}

fn ask(order_id: OrderId, uid: UserId, price: Price, size: Size) -> DirectOrder {
    DirectOrder {
        order_id,
        uid,
        price,
        size,
        filled: 0,
        action: OrderAction::Ask,
        reserve_price: price,
        timestamp_ms: 1_000_000,
        next: None,
        prev: None,
        parent: 0,
    }
}

// ---- insert_order: B3 — 第一次插入，书本为空 ----

#[test]
fn insert_single_bid_sets_best_bid() {
    let mut book = make_book();
    let idx = book.insert_order(bid(1, 10, 100, 50));

    assert_eq!(book.best_bid_order, Some(idx));
    assert_eq!(book.best_ask_order, None);
}

#[test]
fn insert_single_ask_sets_best_ask() {
    let mut book = make_book();
    let idx = book.insert_order(ask(1, 10, 100, 50));

    assert_eq!(book.best_ask_order, Some(idx));
    assert_eq!(book.best_bid_order, None);
}

#[test]
fn insert_single_bid_creates_bucket_with_correct_state() {
    let mut book = make_book();
    book.insert_order(bid(1, 10, 100, 50));

    assert_eq!(book.bid_price_buckets.len(), 1);
    let &bucket_idx = book.bid_price_buckets.get(&100).expect("bucket at 100");
    let bucket = &book.buckets[bucket_idx];
    assert_eq!(bucket.price, 100);
    assert_eq!(bucket.volume, 50);
    assert_eq!(bucket.num_orders, 1);
}

#[test]
fn insert_order_is_indexed_by_order_id() {
    let mut book = make_book();
    let returned_idx = book.insert_order(bid(42, 10, 100, 10));

    assert!(book.order_id_index.contains_key(&42));
    assert_eq!(book.order_id_index[&42], returned_idx);
    assert_eq!(book.orders[returned_idx].order_id, 42);
}

#[test]
fn insert_partial_fill_counts_remaining_size_for_volume() {
    let mut book = make_book();
    let mut order = bid(1, 10, 100, 100);
    order.filled = 40;
    book.insert_order(order);

    let &bucket_idx = book.bid_price_buckets.get(&100).unwrap();
    assert_eq!(book.buckets[bucket_idx].volume, 60);
}

// ---- insert_order: A1 — 桶已存在，当前桶是全局链表末尾 ----

#[test]
fn insert_two_bids_same_price_shares_one_bucket() {
    let mut book = make_book();
    book.insert_order(bid(1, 10, 100, 30));
    book.insert_order(bid(2, 11, 100, 20));

    assert_eq!(book.bid_price_buckets.len(), 1);
    let &bucket_idx = book.bid_price_buckets.get(&100).unwrap();
    assert_eq!(book.buckets[bucket_idx].num_orders, 2);
    assert_eq!(book.buckets[bucket_idx].volume, 50);
}

#[test]
fn insert_two_bids_same_price_preserves_fifo_order() {
    let mut book = make_book();
    let idx1 = book.insert_order(bid(1, 10, 100, 30));
    let idx2 = book.insert_order(bid(2, 11, 100, 20));

    // Global list: best_bid → idx1(head) → idx2(tail)
    assert_eq!(book.best_bid_order, Some(idx1));
    assert_eq!(book.orders[idx1].prev, None);
    assert_eq!(book.orders[idx1].next, Some(idx2));
    assert_eq!(book.orders[idx2].prev, Some(idx1));
    assert_eq!(book.orders[idx2].next, None);

    let &bucket_idx = book.bid_price_buckets.get(&100).unwrap();
    assert_eq!(book.buckets[bucket_idx].tail, idx2);
}

#[test]
fn insert_same_price_volume_accumulates() {
    let mut book = make_book();
    book.insert_order(ask(1, 10, 100, 10));
    book.insert_order(ask(2, 11, 100, 20));
    book.insert_order(ask(3, 12, 100, 30));

    let &bucket_idx = book.ask_price_buckets.get(&100).unwrap();
    assert_eq!(book.buckets[bucket_idx].volume, 60);
    assert_eq!(book.buckets[bucket_idx].num_orders, 3);
}

// ---- insert_order: A2 — 桶已存在，当前桶不是全局链表末尾 ----
// 触发 self.orders[next_idx].prev = Some(order_idx)

#[test]
fn insert_into_non_last_bucket_updates_successor_prev() {
    let mut book = make_book();
    let idx1 = book.insert_order(ask(1, 10, 100, 10)); // bucket@100, idx1.next=None
    let idx2 = book.insert_order(ask(2, 11, 200, 10)); // idx1.next 变为 Some(idx2)
    // 再向 bucket@100 插入：old_tail=idx1, next_of_old_tail=Some(idx2) → 触发 A2
    let idx3 = book.insert_order(ask(3, 12, 100, 10));

    // 全局链表: idx1 → idx3 → idx2
    assert_eq!(book.best_ask_order, Some(idx1));
    assert_eq!(book.orders[idx1].next, Some(idx3));
    assert_eq!(book.orders[idx3].prev, Some(idx1));
    assert_eq!(book.orders[idx3].next, Some(idx2));
    assert_eq!(book.orders[idx2].prev, Some(idx3)); // A2 分支写入的值
    assert_eq!(book.orders[idx2].next, None);

    let &bucket_100 = book.ask_price_buckets.get(&100).unwrap();
    assert_eq!(book.buckets[bucket_100].tail, idx3);
    assert_eq!(book.buckets[bucket_100].num_orders, 2);
    assert_eq!(book.buckets[bucket_100].volume, 20);
}

// ---- insert_order: B1 — 新桶，存在价格更优的前驱桶 ----

#[test]
fn insert_bid_worse_than_best_does_not_update_best_bid() {
    let mut book = make_book();
    let idx1 = book.insert_order(bid(1, 10, 200, 10));
    book.insert_order(bid(2, 11, 100, 10)); // worse

    assert_eq!(book.best_bid_order, Some(idx1));
}

#[test]
fn insert_ask_worse_than_best_does_not_update_best_ask() {
    let mut book = make_book();
    let idx1 = book.insert_order(ask(1, 10, 100, 10));
    book.insert_order(ask(2, 11, 200, 10)); // worse

    assert_eq!(book.best_ask_order, Some(idx1));
}

#[test]
fn insert_three_asks_links_global_list_in_ascending_price_order() {
    let mut book = make_book();
    let idx1 = book.insert_order(ask(1, 10, 200, 10));
    let idx2 = book.insert_order(ask(2, 11, 100, 10));
    let idx3 = book.insert_order(ask(3, 12, 150, 10));

    // Global: best_ask → @100 → @150 → @200
    assert_eq!(book.best_ask_order, Some(idx2));
    assert_eq!(book.orders[idx2].next, Some(idx3));
    assert_eq!(book.orders[idx3].prev, Some(idx2));
    assert_eq!(book.orders[idx3].next, Some(idx1));
    assert_eq!(book.orders[idx1].prev, Some(idx3));
    assert_eq!(book.orders[idx1].next, None);
}

#[test]
fn insert_three_bids_links_global_list_in_descending_price_order() {
    let mut book = make_book();
    let idx1 = book.insert_order(bid(1, 10, 100, 10));
    let idx2 = book.insert_order(bid(2, 11, 200, 10));
    let idx3 = book.insert_order(bid(3, 12, 150, 10));

    // Global: best_bid → @200 → @150 → @100
    assert_eq!(book.best_bid_order, Some(idx2));
    assert_eq!(book.orders[idx2].next, Some(idx3));
    assert_eq!(book.orders[idx3].prev, Some(idx2));
    assert_eq!(book.orders[idx3].next, Some(idx1));
    assert_eq!(book.orders[idx1].prev, Some(idx3));
    assert_eq!(book.orders[idx1].next, None);
}

#[test]
fn insert_two_orders_per_bucket_links_across_buckets() {
    let mut book = make_book();
    let idx1 = book.insert_order(ask(1, 10, 100, 10)); // 100-head
    let idx2 = book.insert_order(ask(2, 11, 100, 10)); // 100-tail
    let idx3 = book.insert_order(ask(3, 12, 200, 10)); // 200-head
    let idx4 = book.insert_order(ask(4, 13, 200, 10)); // 200-tail

    // Global: @100-head → @100-tail → @200-head → @200-tail
    assert_eq!(book.best_ask_order, Some(idx1));
    assert_eq!(book.orders[idx1].next, Some(idx2));
    assert_eq!(book.orders[idx2].next, Some(idx3));
    assert_eq!(book.orders[idx3].prev, Some(idx2));
    assert_eq!(book.orders[idx3].next, Some(idx4));
    assert_eq!(book.orders[idx4].prev, Some(idx3));
    assert_eq!(book.orders[idx4].next, None);
}

// ---- insert_order: B2 — 新桶，成为新的最优价格 ----

#[test]
fn insert_bid_better_than_best_updates_best_bid() {
    let mut book = make_book();
    book.insert_order(bid(1, 10, 100, 10));
    let idx2 = book.insert_order(bid(2, 11, 200, 10)); // new best

    assert_eq!(book.best_bid_order, Some(idx2));
}

#[test]
fn insert_ask_better_than_best_updates_best_ask() {
    let mut book = make_book();
    book.insert_order(ask(1, 10, 200, 10));
    let idx2 = book.insert_order(ask(2, 11, 100, 10)); // new best

    assert_eq!(book.best_ask_order, Some(idx2));
}

#[test]
fn insert_bids_at_different_prices_best_is_highest() {
    let mut book = make_book();
    let idx1 = book.insert_order(bid(1, 10, 100, 10));
    let idx2 = book.insert_order(bid(2, 11, 200, 10));

    // 200 is the better bid; global: @200 → @100
    assert_eq!(book.best_bid_order, Some(idx2));
    assert_eq!(book.bid_price_buckets.len(), 2);
    assert_eq!(book.orders[idx2].next, Some(idx1));
    assert_eq!(book.orders[idx1].prev, Some(idx2));
}

#[test]
fn insert_asks_at_different_prices_best_is_lowest() {
    let mut book = make_book();
    let idx1 = book.insert_order(ask(1, 10, 200, 10));
    let idx2 = book.insert_order(ask(2, 11, 100, 10));

    // 100 is the better ask; global: @100 → @200
    assert_eq!(book.best_ask_order, Some(idx2));
    assert_eq!(book.ask_price_buckets.len(), 2);
    assert_eq!(book.orders[idx2].next, Some(idx1));
    assert_eq!(book.orders[idx1].prev, Some(idx2));
}

// ---- remove_order: E1 — 桶内最后一单，删除桶 ----

#[test]
fn remove_only_order_clears_book_entirely() {
    let mut book = make_book();
    let idx = book.insert_order(bid(1, 10, 100, 50));

    book.remove_order(idx);

    assert_eq!(book.best_bid_order, None);
    assert!(!book.order_id_index.contains_key(&1));
    assert!(book.bid_price_buckets.is_empty());
    assert!(book.buckets.is_empty());
    assert!(book.orders.is_empty());
}

#[test]
fn remove_last_order_in_bucket_deletes_bucket_from_price_index() {
    let mut book = make_book();
    let idx = book.insert_order(ask(1, 10, 100, 10));

    book.remove_order(idx);

    assert!(!book.ask_price_buckets.contains_key(&100));
    assert!(book.buckets.is_empty());
}

#[test]
fn remove_non_best_order_does_not_change_best() {
    let mut book = make_book();
    let idx1 = book.insert_order(bid(1, 10, 200, 10)); // best
    let idx2 = book.insert_order(bid(2, 11, 100, 10)); // worse

    book.remove_order(idx2);

    assert_eq!(book.best_bid_order, Some(idx1));
    assert_eq!(book.orders[idx1].next, None);
}

// ---- remove_order: E2 — 桶内多单，删除 tail ----

#[test]
fn remove_tail_order_from_bucket_updates_tail_pointer() {
    let mut book = make_book();
    let idx1 = book.insert_order(bid(1, 10, 100, 10)); // head
    let idx2 = book.insert_order(bid(2, 11, 100, 20)); // tail

    book.remove_order(idx2);

    let &bucket_idx = book.bid_price_buckets.get(&100).unwrap();
    assert_eq!(book.buckets[bucket_idx].tail, idx1);
    assert_eq!(book.orders[idx1].next, None);
    assert_eq!(book.buckets[bucket_idx].num_orders, 1);
    assert_eq!(book.buckets[bucket_idx].volume, 10);
}

#[test]
fn remove_bucket_tail_that_precedes_next_bucket_relinks_cross_bucket() {
    let mut book = make_book();
    // ask: @100-A, @100-B, @200-A
    let idx1 = book.insert_order(ask(1, 10, 100, 10));
    let idx2 = book.insert_order(ask(2, 11, 100, 10));
    let idx3 = book.insert_order(ask(3, 12, 200, 10));

    // Remove @100-B (tail of bucket@100, global predecessor of @200-A)
    book.remove_order(idx2);

    // Global: @100-A → @200-A
    assert_eq!(book.orders[idx1].next, Some(idx3));
    assert_eq!(book.orders[idx3].prev, Some(idx1));

    let &bucket_100 = book.ask_price_buckets.get(&100).unwrap();
    assert_eq!(book.buckets[bucket_100].tail, idx1);
    assert_eq!(book.buckets[bucket_100].num_orders, 1);
}

// ---- remove_order: E3 — 桶内多单，删除非 tail ----

#[test]
fn remove_head_order_from_bucket_advances_head() {
    let mut book = make_book();
    let idx1 = book.insert_order(bid(1, 10, 100, 10)); // head
    let idx2 = book.insert_order(bid(2, 11, 100, 20)); // tail

    book.remove_order(idx1);

    assert_eq!(book.best_bid_order, Some(idx2));
    assert_eq!(book.orders[idx2].prev, None);
    assert_eq!(book.orders[idx2].next, None);

    let &bucket_idx = book.bid_price_buckets.get(&100).unwrap();
    assert_eq!(book.buckets[bucket_idx].num_orders, 1);
    assert_eq!(book.buckets[bucket_idx].volume, 20);
}

#[test]
fn remove_middle_order_relinks_neighbors_and_updates_volume() {
    let mut book = make_book();
    let idx1 = book.insert_order(bid(1, 10, 100, 10));
    let idx2 = book.insert_order(bid(2, 11, 100, 20));
    let idx3 = book.insert_order(bid(3, 12, 100, 30));

    book.remove_order(idx2);

    assert_eq!(book.orders[idx1].next, Some(idx3));
    assert_eq!(book.orders[idx3].prev, Some(idx1));
    assert_eq!(book.best_bid_order, Some(idx1));

    let &bucket_idx = book.bid_price_buckets.get(&100).unwrap();
    assert_eq!(book.buckets[bucket_idx].tail, idx3);
    assert_eq!(book.buckets[bucket_idx].num_orders, 2);
    assert_eq!(book.buckets[bucket_idx].volume, 40);
}

// ---- remove_order: C1 — 删除全局链表头（更新 best） ----

#[test]
fn remove_best_bid_promotes_next_price_level_to_best() {
    let mut book = make_book();
    let idx1 = book.insert_order(bid(1, 10, 200, 10)); // best
    let idx2 = book.insert_order(bid(2, 11, 100, 10)); // next

    book.remove_order(idx1);

    assert_eq!(book.best_bid_order, Some(idx2));
    assert_eq!(book.orders[idx2].prev, None);
}

#[test]
fn remove_best_ask_promotes_next_price_level_to_best() {
    let mut book = make_book();
    let idx1 = book.insert_order(ask(1, 10, 100, 10)); // best
    let idx2 = book.insert_order(ask(2, 11, 200, 10)); // next

    book.remove_order(idx1);

    assert_eq!(book.best_ask_order, Some(idx2));
    assert_eq!(book.orders[idx2].prev, None);
}

// ---- remove_order: 索引清理 ----

#[test]
fn remove_order_removes_it_from_id_index() {
    let mut book = make_book();
    let idx = book.insert_order(ask(99, 10, 500, 10));

    book.remove_order(idx);

    assert!(!book.order_id_index.contains_key(&99));
}

// ---- 复合场景: Slab 索引复用 ----

#[test]
fn insert_after_remove_slab_slot_reuse_is_correct() {
    let mut book = make_book();
    let idx_a = book.insert_order(bid(1, 10, 100, 10));
    let idx2 = book.insert_order(bid(2, 11, 200, 10)); // 保持 book 非空

    book.remove_order(idx_a); // 释放 slot，下次 insert 可能复用

    let idx_b = book.insert_order(bid(3, 12, 100, 10));

    // 新 order 数据正确，无旧 order 的幽灵数据
    assert_eq!(book.orders[idx_b].order_id, 3);
    assert_eq!(book.orders[idx_b].price, 100);
    assert!(book.order_id_index.contains_key(&3));
    assert!(!book.order_id_index.contains_key(&1));

    // 链表完整：@200(best) → @100
    assert_eq!(book.best_bid_order, Some(idx2));
    assert_eq!(book.orders[idx2].next, Some(idx_b));
    assert_eq!(book.orders[idx_b].prev, Some(idx2));
    assert_eq!(book.orders[idx_b].next, None);
}

// ---- 复合场景: 跨多价格档位全清 ----

#[test]
fn remove_all_orders_across_multiple_buckets_leaves_empty_book() {
    let mut book = make_book();
    let idx1 = book.insert_order(ask(1, 10, 100, 10));
    let idx2 = book.insert_order(ask(2, 11, 100, 20));
    let idx3 = book.insert_order(ask(3, 12, 200, 10));
    let idx4 = book.insert_order(ask(4, 13, 300, 10));

    book.remove_order(idx1);
    book.remove_order(idx2);
    book.remove_order(idx3);
    book.remove_order(idx4);

    assert_eq!(book.best_ask_order, None);
    assert!(book.ask_price_buckets.is_empty());
    assert!(book.buckets.is_empty());
    assert!(book.orders.is_empty());
    assert!(book.order_id_index.is_empty());
}

// ---- 复合场景: 买卖双边不互相干扰 ----

#[test]
fn bid_and_ask_sides_do_not_interfere_with_each_other() {
    let mut book = make_book();
    let bid_idx = book.insert_order(bid(1, 10, 100, 50));
    let ask_idx = book.insert_order(ask(2, 11, 200, 30));

    // 各自维护独立的 best 和 bucket
    assert_eq!(book.best_bid_order, Some(bid_idx));
    assert_eq!(book.best_ask_order, Some(ask_idx));
    assert_eq!(book.bid_price_buckets.len(), 1);
    assert_eq!(book.ask_price_buckets.len(), 1);

    // 双边链表互不相连
    assert_eq!(book.orders[bid_idx].next, None);
    assert_eq!(book.orders[ask_idx].next, None);
    assert_eq!(book.orders[bid_idx].prev, None);
    assert_eq!(book.orders[ask_idx].prev, None);

    // 删除一边不影响另一边
    book.remove_order(bid_idx);
    assert_eq!(book.best_bid_order, None);
    assert_eq!(book.best_ask_order, Some(ask_idx));
    assert!(book.bid_price_buckets.is_empty());
    assert_eq!(book.ask_price_buckets.len(), 1);
    assert_eq!(book.orders[ask_idx].order_id, 2);
}
