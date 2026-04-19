# DirectOrderBook 实现指南

本文档是 me-rs `DirectOrderBook` 的完整实现参考，以 mermaid 流程图串联所有业务逻辑。

---

## 一、数据结构关系

```mermaid
graph TD
    OB["DirectOrderBook"]

    OB -->|"orders: Slab&lt;DirectOrder&gt;"| DO["DirectOrder\n───────────\norder_id\nuid\nprice / size / filled\naction / reserve_price\ntimestamp_ms\nnext / prev / parent"]

    OB -->|"buckets: Slab&lt;Bucket&gt;"| BK["Bucket\n───────────\nprice\nvolume  ← 该价位剩余总量\nnum_orders\ntail    ← 指向最新挂单"]

    OB -->|"ask_price_buckets\nBTreeMap&lt;Price,BucketIdx&gt;"| BK
    OB -->|"bid_price_buckets\nBTreeMap&lt;Price,BucketIdx&gt;"| BK
    OB -->|"order_id_index\nAHashMap&lt;OrderId,OrderIdx&gt;"| DO
    OB -->|"best_ask_order\nOption&lt;OrderIdx&gt;"| DO
    OB -->|"best_bid_order\nOption&lt;OrderIdx&gt;"| DO

    DO -->|"parent: BucketIdx"| BK
```

**链表结构说明**

同一价位的订单通过双向链表连接，`tail` 指向**最新**挂单（时间最后），`best_ask_order`/`best_bid_order` 指向**最优价格**的订单（时间最早，优先撮合）。

```
best_ask_order
      ↓
[OrderA, price=100, next=None, prev=OrderB]   ← 最早挂单，优先被撮合
      ↑ next
[OrderB, price=100, next=OrderA, prev=OrderC]
      ↑ next
[OrderC, price=100, next=OrderB, prev=None]   ← 最新挂单，Bucket.tail 指向它

Bucket(price=100).tail = OrderC
```

撮合时沿 `prev` 方向遍历（从 `best_ask_order` 往 `prev` 走），确保时间优先。

---

## 二、订单生命周期

```mermaid
stateDiagram-v2
    [*] --> PreMatch: new_order()

    PreMatch --> Rejected: 校验失败\n(重复ID / tick_size / min_notional 等)
    PreMatch --> Matching: 进入撮合

    Matching --> FullyFilled: filled == size
    Matching --> PartialFilled: 0 < filled < size
    Matching --> NoFill: filled == 0

    PartialFilled --> Resting: GTC → 余量挂单
    NoFill --> Resting: GTC → 全量挂单
    PartialFilled --> Rejected: IOC/FOK → 余量 Reject event
    NoFill --> Rejected: IOC/FOK → 全量 Reject event

    Resting --> Matching: 被动撮合（对手方下单触发）
    Resting --> Cancelled: cancel_order()
    Resting --> Moved: move_order() → 移价

    Moved --> Matching: 新价格尝试撮合
    Moved --> Resting: 撮合后余量继续挂单

    FullyFilled --> [*]
    Cancelled --> [*]
    Rejected --> [*]
```

---

## 三、new_order 入口路由

```mermaid
flowchart TD
    A([new_order\ncmd: &mut OrderCommand]) --> B{order_type?}

    B -->|GTC| C[place_gtc]
    B -->|IOC| D[place_ioc]
    B -->|FOKWithBudget| E[place_fok_budget]
    B -->|其他| F[cmd.result_code =\nUnsupportedCommand\nreturn]

    C --> G[cmd.result_code = Success]
    D --> G
    E --> G
```

> **me-rs 风格**：各 `place_*` 函数返回 `()`，结果写回 `cmd.result_code`，不 return CmdResultCode。

---

## 四、Pre-match 校验（进入任何撮合前）

```mermaid
flowchart TD
    A([validate_order\ncmd]) --> B{order_id 重复?}
    B -->|是| Z1[result_code = DuplicateOrderId\npush Reject event\nreturn]
    B -->|否| C{size % lot_size == 0?}
    C -->|否| Z2[result_code = InvalidSize\nreturn]
    C -->|是| D{price % tick_size == 0?}
    D -->|否| Z3[result_code = InvalidPrice\nreturn]
    D -->|是| E{size >= min_qty?}
    E -->|否| Z4[result_code = InvalidSize\nreturn]
    E -->|是| F{size × price >= min_notional?}
    F -->|否| Z5[result_code = InvalidSize\nreturn]
    F -->|是| G([通过，进入撮合])
```

---

## 五、place_gtc 流程

```mermaid
flowchart TD
    A([place_gtc]) --> B[try_match\n返回 filled]
    B --> C{filled == cmd.size?}
    C -->|是，完全成交| D([结束\n无需挂单])
    C -->|否，余量| E["orders.insert(DirectOrder {\n  order_id, uid, price,\n  size, filled, action,\n  reserve_price, timestamp_ms,\n  next: None, prev: None, parent: 0\n})"]
    E --> F[order_id_index.insert\norder_id → order_idx]
    F --> G[insert_order\norder_idx]
    G --> H([结束])
```

**注意**：`orders.insert` → `order_id_index.insert` → `insert_order` 顺序固定：
- `insert_order` 需要读 `orders[order_idx]`（已插入）
- `order_id_index` 在 `insert_order` 之前插入，确保 cancel 请求可以立即找到订单

---

## 六、try_match 核心撮合

```mermaid
flowchart TD
    A([try_match\ncmd]) --> B{is_bid?}
    B -->|买单| C[maker_idx = best_ask_order]
    B -->|卖单| D[maker_idx = best_bid_order]

    C --> E{maker_idx 存在\n且价格可成交?}
    D --> E
    E -->|否| Z([return filled=0])
    E -->|是| F["循环：while let Some(idx) = maker_idx"]

    F --> G{remaining == 0?}
    G -->|是| DONE[break]
    G -->|否| H{价格仍可成交?}
    H -->|否| DONE

    H -->|是| I["trade_size =\nremaining.min(maker.size - maker.filled)"]

    I --> K{maker.uid == taker.uid?\nSTP 检查}
    K -->|是 Cancel New| L["push Reject(cmd.size - filled)\nreturn filled\n⚠️ 不修改任何状态"]
    K -->|否| J[maker.filled += trade_size\nbucket.volume -= trade_size\nfilled += trade_size]
    J --> M[push Trade event\n包含 maker_order_id + maker_uid\nbidder_hold_price]

    M --> N{maker 完全成交?}
    N -->|否| DONE
    N -->|是| O[bucket.num_orders -= 1\norder_id_index.remove\norders.remove]
    O --> P{bucket 已空?}
    P -->|是| Q[price_buckets.remove\nbuckets.remove]
    P -->|否| R[继续]
    Q --> R
    R --> S[maker_idx = maker.prev]
    S --> F

    DONE --> T{is_bid?}
    T -->|是| U[best_ask_order = maker_idx]
    T -->|否| V[best_bid_order = maker_idx]
    U --> W([return filled])
    V --> W
```

**STP 位置**：在计算 `trade_size` **之后**、修改任何状态（`maker.filled` / `bucket.volume`）**之前**检查。触发时直接 return，maker 状态保持不变。

**remaining 重算**：每次循环开始需重新计算 `remaining = cmd.size - filled`，不是常量。

---

## 七、insert_order 挂单入链表

```mermaid
flowchart TD
    A([insert_order\norder_idx]) --> B[取 order.price, order.action]
    B --> C{price_buckets\n已有此价位?}

    C -->|是，Bucket 存在| D[old_tail = bucket.tail]
    D --> E[bucket.tail = order_idx\nbucket.volume += remaining\nbucket.num_orders += 1]
    E --> F["链表插入（追加到 old_tail 之后）:\nold_tail.prev = order_idx\nprev_of_old_tail.next = order_idx\norder.next = old_tail\norder.prev = prev_of_old_tail\norder.parent = bucket_idx"]
    F --> Z([结束])

    C -->|否，新价位| G["buckets.insert(Bucket {\n  price, volume, num_orders:1, tail:order_idx\n})"]
    G --> H[price_buckets.insert\nprice → bucket_idx\norder.parent = bucket_idx]
    H --> I{找相邻价位\n插入链表位置}

    I -->|"Ask: 找价格 < price 的最高桶\nBid: 找价格 > price 的最低桶"| J{找到相邻桶?}

    J -->|是| K["插入到相邻桶 tail 之前:\nlower_tail.prev = order_idx\nprev_of_lower.next = order_idx\norder.next = lower_tail\norder.prev = prev_of_lower"]
    K --> Z

    J -->|否，成为新的最优价| L[old_best = best_ask/bid_order]
    L --> M["old_best.next = order_idx\nbest_ask/bid_order = order_idx\norder.next = None\norder.prev = old_best"]
    M --> Z
```

---

## 八、cancel / move / reduce 对比

```mermaid
flowchart LR
    subgraph cancel_order
        CA[order_id_index.get] --> CB{uid 匹配?}
        CB -->|否| CX[UnknownOrderId]
        CB -->|是| CC[order_id_index.remove]
        CC --> CD[remove_order]
        CD --> CE[orders.remove]
        CE --> CF[push Reject\n全部余量]
    end

    subgraph move_order
        MA[order_id_index.get] --> MB{风控检查\nnew_price > reserve?}
        MB -->|是| MX[InvalidReservePrice]
        MB -->|否| MC[remove_order\n从旧价位移除]
        MC --> MD[order.price = new_price]
        MD --> ME["temp_cmd.size =\norder.size - order.filled\n⚠️ 传剩余量，不是原始总量"]
        ME --> MF[try_match temp_cmd\n返回 filled_in_move]
        MF --> MG{filled_in_move ==\norder.size - order.filled?}
        MG -->|是，完全成交| MH[order_id_index.remove\norders.remove]
        MG -->|否，余量继续挂单| MI["order.filled += filled_in_move\n⚠️ 累加，不是覆盖\ninsert_order 新价位"]
    end

    subgraph reduce_order
        RA[order_id_index.get] --> RB{reduce_by == remaining?}
        RB -->|是，全部减掉| RC[order_id_index.remove\nremove_order\norders.remove]
        RB -->|否，部分减少| RD[order.size -= reduce_by\nbucket.volume -= reduce_by]
        RC --> RE[push Reject\nreduce_by 数量]
        RD --> RE
    end
```

**关键差异**：
- `cancel`：移除整个订单，生成 Reject event
- `move`：移除后重新撮合，可能部分成交再挂回，**丢失原有时间优先级**；传给 try_match 的 size 必须是剩余量（`order.size - order.filled`），成交后用 `+=` 累加而非覆盖
- `reduce`：不移除订单，直接减少 size，**保留时间优先级**

---

## 九、事件生成规则

| 操作 | 条件 | 生成事件 | event_type |
|------|------|---------|-----------|
| try_match | 每笔撮合 | `MatcherEvent::new_trade(size, price, maker_order_id, maker_uid, bidder_hold_price)` | `Trade` |
| place_gtc | STP 触发 | `MatcherEvent::new_reject(remaining, price)` | `Reject` |
| place_ioc | 未成交余量 | `MatcherEvent::new_reject(rejected, price)` | `Reject` |
| place_fok_budget | 预算不足 / 流动性不足 | `MatcherEvent::new_reject(size, price)` | `Reject` |
| cancel_order | 成功撤单 | `MatcherEvent::new_reject(remaining, price)` | `Reject` |
| reduce_order | 成功减量 | `MatcherEvent::new_reject(reduce_by, price)` | `Reduce` |
| move_order | 依赖内部 try_match | 继承 try_match 生成的 Trade events | `Trade` |

> `bidder_hold_price`：taker 是买方时填 `cmd.reserve_price`，taker 是卖方时填 `maker.reserve_price`。下游风控层用它计算买方的退款差价。

---

## 十、me-rs 实现映射

以下对应关系来自与 matching-core 的对比，实现时注意替换：

| 概念 | matching-core 写法 | me-rs 写法 |
|------|-------------------|-----------|
| 撮合事件 | `MatcherTradeEvent` | `MatcherEvent` |
| 事件 maker 字段 | `matched_order_id` / `matched_order_uid` | `maker_order_id` / `maker_uid` |
| 时间字段 | `order.timestamp` | `order.timestamp_ms` |
| 结果码 | `CommandResultCode` | `CmdResultCode` |
| 函数返回值 | `-> CmdResultCode` | `-> ()` 写回 `cmd.result_code` |
| 重复订单 | try_match 后 reject（bug） | 直接 reject，不撮合 |
| STP 模式 | 未实现 | 必须实现，默认 CancelNew |

---

## 十一、已知边界问题

**FOK + STP 的交互**

`check_budget_to_fill` 按 `bucket.volume` 累计计算预算，不感知哪些 maker 属于同一 uid。若路径中存在自成交 maker：

1. 预算检查通过 → 进入 try_match
2. try_match 遇到 STP → 提前终止
3. 实际成交量 < FOK 要求 → FOK 语义被破坏

**修正方向**：`check_budget_to_fill` 需要接收 `taker_uid`，跳过与 taker 同 uid 的 maker 的 volume 贡献；或在 STP 触发时整体 reject（即 FOK + Cancel New = 全单取消）。

---

## 十二、实现验证清单

实现每个函数后，用以下场景验证：

**场景 1：基础 GTC 挂单**
```
下单：GTC Buy 10 @ 100
预期：无对手方 → 挂单，best_bid_order = 此订单，matcher_events 为空
```

**场景 2：完全撮合**
```
已有：Ask 10 @ 100 挂单
下单：GTC Buy 10 @ 100
预期：Trade event (size=10, price=100)，两单均移除，best_ask/bid = None
```

**场景 3：部分撮合 + 余量挂单**
```
已有：Ask 5 @ 100 挂单
下单：GTC Buy 10 @ 100
预期：Trade event (size=5)，Ask 移除，Buy 余量 5 挂单
```

**场景 4：IOC 余量取消**
```
已有：Ask 5 @ 100 挂单
下单：IOC Buy 10 @ 100
预期：Trade event (size=5) + Reject event (size=5)，无挂单
```

**场景 5：STP 自成交**
```
uid=A 挂 Ask 10 @ 100
uid=A 下 GTC Buy 10 @ 100
预期：Reject event (size=10)，原 Ask 保留，无 Trade event
```

**场景 6：cancel_order**
```
已有：GTC Ask 10 @ 100（已 partial fill 4）
撤单：cancel order_id=X
预期：Reject event (size=6，即余量)，订单从 book 移除
```
