# DirectOrderBook 实现指南

本文档是 me-rs `DirectOrderBook` 的完整实现参考，以 mermaid 流程图串联所有业务逻辑。

---

## 一、数据结构关系

```mermaid
graph TD
    subgraph OB["🏛️ DirectOrderBook（撮合状态核心）"]
        direction TB
        STORE["📦 存储池\norders: Slab&lt;DirectOrder&gt;\nbuckets: Slab&lt;Bucket&gt;"]
        INDEX["🔍 索引\nask_price_buckets: BTreeMap&lt;Price, BucketIdx&gt;\nbid_price_buckets: BTreeMap&lt;Price, BucketIdx&gt;\norder_id_index: AHashMap&lt;OrderId, OrderIdx&gt;"]
        BEST["⭐ 盘口快照\nbest_ask_order: Option&lt;OrderIdx&gt;\nbest_bid_order: Option&lt;OrderIdx&gt;"]
    end

    DO["📋 DirectOrder\n────────────\norder_id / uid\nprice / size / filled\naction / reserve_price\ntimestamp_ms\nnext / prev（双向链表指针）\nparent → BucketIdx"]

    BK["📊 Bucket\n────────────\nprice\nvolume（该价位剩余总量）\nnum_orders\ntail（指向最新挂单）"]

    STORE -->|"分配 order_idx"| DO
    STORE -->|"分配 bucket_idx"| BK
    INDEX -.->|"price → bucket"| BK
    INDEX -.->|"order_id → order"| DO
    BEST -.->|"指向链表头\n（时间最早，优先撮合）"| DO
    DO -->|"parent 反向引用"| BK

    style OB fill:#e8eaf6,stroke:#5c6bc0,color:#1a237e
    style DO fill:#d4edda,stroke:#28a745,color:#155724
    style BK fill:#fff3cd,stroke:#ffc107,color:#856404
```

**链表结构说明**

同一价位的订单通过双向链表连接，`tail` 指向**最新**挂单（时间最后），`best_ask_order`/`best_bid_order` 指向**最优价格**的订单（时间最早，优先撮合）。

```
best_ask_order
      ↓
[OrderA, price=100, prev=None, next=OrderB]   ← 最早挂单，优先被撮合
      ↓ next
[OrderB, price=100, prev=OrderA, next=OrderC]
      ↓ next
[OrderC, price=100, prev=OrderB, next=None]   ← 最新挂单，Bucket.tail 指向它

Bucket(price=100).tail = OrderC
```

撮合时沿 `next` 方向遍历（从 `best_ask_order` 往 `next` 走），确保时间优先。

---

## 二、订单生命周期

```mermaid
flowchart TD
    START(["🟢 new_order()"])
    VALIDATE["① Pre-match 校验\n• 重复 order_id\n• size % lot_size\n• price % tick_size\n• min_qty / min_notional"]
    MATCH["② try_match\n价格优先 → 时间优先\n遍历对手盘，累计 filled"]
    D1{"filled\n== size?"}
    D2{"order_type?"}
    REST[["③ Resting\n余量在盘口等待"]]
    MOVE["④ Moved\n• remove_order 旧价位\n• order.price = new_price\n• try_match 重新撮合"]
    D3{"filled\n== 剩余量?"}
    DONE(["✓ FullyFilled"])
    REJ_V(["✗ Rejected\n校验失败"])
    REJ_IOC(["✗ Rejected\n余量取消"])
    CANCEL(["✗ Cancelled"])

    START --> VALIDATE
    VALIDATE -->|"校验失败"| REJ_V
    VALIDATE -->|"通过"| MATCH
    MATCH --> D1
    D1 -->|"是"| DONE
    D1 -->|"否，有余量"| D2
    D2 -->|"IOC / FOK"| REJ_IOC
    D2 -->|"GTC"| REST
    REST -->|"cancel_order"| CANCEL
    REST -->|"move_order"| MOVE
    REST -.->|"被动撮合\n（对手方下单触发）"| DONE
    MOVE --> D3
    D3 -->|"是，完全成交"| DONE
    D3 -->|"否，有余量"| REST

    classDef ok   fill:#d4edda,stroke:#28a745,color:#155724
    classDef fail fill:#f8d7da,stroke:#dc3545,color:#721c24
    classDef proc fill:#d1ecf1,stroke:#17a2b8,color:#0c5460
    classDef wait fill:#fff3cd,stroke:#ffc107,color:#856404
    classDef gate fill:#e8eaf6,stroke:#5c6bc0,color:#1a237e

    class START,DONE ok
    class REJ_V,REJ_IOC,CANCEL fail
    class VALIDATE,MATCH,MOVE proc
    class REST wait
    class D1,D2,D3 gate
```

---

## 三、new_order 入口路由

```mermaid
flowchart TD
    A(["🟢 new_order(cmd: &mut OrderCommand)"])
    B{"order_type?"}

    C["place_gtc(cmd)"]
    D["place_ioc(cmd)"]
    E["place_fok_budget(cmd)"]
    F["cmd.result_code =\nUnsupportedCommand\nreturn"]

    G(["✓ cmd.result_code = Success"])
    H(["✗ cmd.result_code = UnsupportedCommand"])

    A --> B
    B -->|"GTC"| C
    B -->|"IOC"| D
    B -->|"FOKWithBudget"| E
    B -->|"其他"| F

    C --> G
    D --> G
    E --> G
    F --> H

    style A fill:#c8f7c5,stroke:#28a745
    style B fill:#e8eaf6,stroke:#5c6bc0
    style C fill:#d1ecf1,stroke:#17a2b8
    style D fill:#d1ecf1,stroke:#17a2b8
    style E fill:#d1ecf1,stroke:#17a2b8
    style F fill:#f8d7da,stroke:#dc3545
    style G fill:#d4edda,stroke:#28a745
    style H fill:#f8d7da,stroke:#dc3545
```

> **me-rs 风格**：各 `place_*` 函数返回 `()`，结果写回 `cmd.result_code`，不 return CmdResultCode。

---

## 四、Pre-match 校验（进入任何撮合前）

```mermaid
flowchart TD
    A(["🟢 validate_order(cmd)"])
    B{"order_id 重复?"}
    C{"size % lot_size == 0?"}
    D{"price % tick_size == 0?"}
    E{"size >= min_qty?"}
    F{"size × price >= min_notional?"}
    G(["✓ 通过，进入撮合"])

    Z1["✗ DuplicateOrderId\npush Reject event\nreturn"]
    Z2["✗ InvalidSize\n（不是 lot_size 整数倍）\nreturn"]
    Z3["✗ InvalidPrice\n（不是 tick_size 整数倍）\nreturn"]
    Z4["✗ InvalidSize\n（低于 min_qty）\nreturn"]
    Z5["✗ InvalidSize\n（低于 min_notional）\nreturn"]

    A --> B
    B -->|"是"| Z1
    B -->|"否"| C
    C -->|"否"| Z2
    C -->|"是"| D
    D -->|"否"| Z3
    D -->|"是"| E
    E -->|"否"| Z4
    E -->|"是"| F
    F -->|"否"| Z5
    F -->|"是"| G

    style A fill:#c8f7c5,stroke:#28a745
    style G fill:#d4edda,stroke:#28a745
    style B fill:#e8eaf6,stroke:#5c6bc0
    style C fill:#e8eaf6,stroke:#5c6bc0
    style D fill:#e8eaf6,stroke:#5c6bc0
    style E fill:#e8eaf6,stroke:#5c6bc0
    style F fill:#e8eaf6,stroke:#5c6bc0
    style Z1 fill:#f8d7da,stroke:#dc3545
    style Z2 fill:#f8d7da,stroke:#dc3545
    style Z3 fill:#f8d7da,stroke:#dc3545
    style Z4 fill:#f8d7da,stroke:#dc3545
    style Z5 fill:#f8d7da,stroke:#dc3545
```

---

## 五、place_gtc 流程

```mermaid
flowchart TD
    A(["🟢 place_gtc(cmd)"])
    B["try_match(cmd)\n→ 返回 filled"]
    C{"filled == cmd.size?"}
    D(["✓ 完全成交\n无需挂单"])

    subgraph REST["③ 余量挂单（顺序固定）"]
        direction TB
        E["① orders.insert(DirectOrder {\n  order_id, uid, price,\n  size, filled, action,\n  reserve_price, timestamp_ms,\n  next: None, prev: None, parent: 0\n})\n→ 返回 order_idx"]
        F["② order_id_index.insert\norder_id → order_idx"]
        G["③ insert_order(order_idx)\n加入价位链表"]
        E --> F --> G
    end

    H(["✓ 挂单完成"])

    A --> B --> C
    C -->|"是"| D
    C -->|"否，有余量"| E
    G --> H

    style A fill:#c8f7c5,stroke:#28a745
    style B fill:#d1ecf1,stroke:#17a2b8
    style C fill:#e8eaf6,stroke:#5c6bc0
    style D fill:#d4edda,stroke:#28a745
    style H fill:#d4edda,stroke:#28a745
    style REST fill:#fff3cd,stroke:#ffc107
```

**顺序依赖**：`orders.insert` → `order_id_index.insert` → `insert_order`
- `insert_order` 需要读 `orders[order_idx]` → `orders.insert` 必须最先
- `order_id_index` 在 `insert_order` 之前插入，确保 cancel 请求可以立即找到订单

---

## 六、try_match 核心撮合

```mermaid
flowchart TD
    START(["🟢 try_match(cmd)"])

    subgraph INIT["① 选择对手盘"]
        direction TB
        B{"is_bid?"}
        C["maker_idx =\nbest_ask_order"]
        D["maker_idx =\nbest_bid_order"]
        B -->|"买单"| C
        B -->|"卖单"| D
    end

    E{"maker_idx 存在\n且价格可成交?"}
    EMPTY(["✓ return filled = 0"])

    subgraph LOOP["② 撮合循环：while let Some(idx) = maker_idx"]
        direction TB
        G{"remaining =\ncmd.size - filled\n== 0?"}
        H{"maker 价格\n仍可成交?"}
        I["trade_size =\nremaining.min(maker.size - maker.filled)"]

        STP{"maker.uid == taker.uid?\n（STP 检查）"}
        STP_REJ["⚠️ push Reject(cmd.size - filled)\nreturn filled\n不修改任何 maker / bucket 状态"]

        J["maker.filled += trade_size\nbucket.volume -= trade_size\nfilled += trade_size"]
        M["push Trade event\n• maker_order_id, maker_uid\n• bidder_hold_price"]

        N{"maker 完全成交?"}
        NEXT["maker_idx = maker.next\n（时间顺序向前走）"]

        subgraph CLEAN["maker 完全成交清理"]
            direction TB
            O["bucket.num_orders -= 1\norder_id_index.remove\norders.remove"]
            P{"bucket 已空?"}
            Q["price_buckets.remove\nbuckets.remove"]
            O --> P
            P -->|"是"| Q
        end

        G -->|"否"| H
        H -->|"是"| I
        I --> STP
        STP -->|"否"| J
        J --> M --> N
        N -->|"否"| NEXT
        N -->|"是"| O
        P -->|"否"| NEXT
        Q --> NEXT
    end

    BREAK["break 循环"]

    subgraph FINAL["③ 更新盘口指针"]
        direction TB
        T{"is_bid?"}
        U["best_ask_order = maker_idx"]
        V["best_bid_order = maker_idx"]
        T -->|"是"| U
        T -->|"否"| V
    end

    DONE(["✓ return filled"])

    START --> B
    C --> E
    D --> E
    E -->|"否"| EMPTY
    E -->|"是"| G
    G -->|"是"| BREAK
    H -->|"否"| BREAK
    STP -->|"是 Cancel New"| STP_REJ
    NEXT --> G
    BREAK --> T
    U --> DONE
    V --> DONE

    style START fill:#c8f7c5,stroke:#28a745
    style DONE fill:#d4edda,stroke:#28a745
    style EMPTY fill:#d4edda,stroke:#28a745
    style STP_REJ fill:#f8d7da,stroke:#dc3545
    style BREAK fill:#fff3cd,stroke:#ffc107
    style INIT fill:#e8eaf6,stroke:#5c6bc0
    style LOOP fill:#fff3cd,stroke:#ffc107
    style CLEAN fill:#d1ecf1,stroke:#17a2b8
    style FINAL fill:#e8eaf6,stroke:#5c6bc0
```

**关键点**：
- **STP 位置**：在计算 `trade_size` **之后**、修改任何状态（`maker.filled` / `bucket.volume`）**之前**检查。触发时直接 return，maker 状态保持不变。
- **remaining 重算**：每次循环开始需重新计算 `remaining = cmd.size - filled`，不是常量。
- **循环方向**：沿 `maker.next` 遍历（同价位从时间最早向最新推进）；跨价位由 `best_*_order` 指针自然承接。
- **循环出口**：`maker_idx` 最终值即为新的最优价指针——要么是部分成交的 maker（仍在链上），要么是下一个价位的首单。

---

## 七、insert_order 挂单入链表

```mermaid
flowchart TD
    START(["🟢 insert_order(order_idx)"])
    A["读取 order.price, order.action"]
    B{"price_buckets\n已有此价位?"}

    subgraph EXIST["路径 A：价位已存在 → 追加到 tail 之后"]
        direction TB
        D["old_tail = bucket.tail"]
        E["bucket.tail = order_idx\nbucket.volume += remaining\nbucket.num_orders += 1"]
        F["链表追加：\nold_tail.next = order_idx\norder.prev = old_tail\norder.next = None\norder.parent = bucket_idx"]
        D --> E --> F
    end

    subgraph NEW["路径 B：新价位 → 创建 Bucket + 定位插入点"]
        direction TB
        G["buckets.insert(Bucket {\n  price, volume,\n  num_orders: 1,\n  tail: order_idx\n})"]
        H["price_buckets.insert\nprice → bucket_idx\norder.parent = bucket_idx"]
        I{"找相邻价位（更差一档）\nAsk: 价格 &lt; price 的最高桶\nBid: 价格 &gt; price 的最低桶"}

        J["插入到相邻桶 tail 之后：\nnext_of_lower = lower_tail.next\nlower_tail.next = order_idx\norder.prev = lower_tail\norder.next = next_of_lower\nif next_of_lower: orders[next_of_lower].prev = order_idx"]

        K["⭐ 成为新最优价：\nold_best = best_ask/bid_order\nold_best.prev = order_idx\nbest_ask/bid_order = order_idx\norder.prev = None\norder.next = old_best"]

        G --> H --> I
        I -->|"找到相邻桶"| J
        I -->|"无相邻桶"| K
    end

    DONE(["✓ 挂单完成"])

    START --> A --> B
    B -->|"是"| D
    B -->|"否"| G
    F --> DONE
    J --> DONE
    K --> DONE

    style START fill:#c8f7c5,stroke:#28a745
    style DONE fill:#d4edda,stroke:#28a745
    style A fill:#d1ecf1,stroke:#17a2b8
    style B fill:#e8eaf6,stroke:#5c6bc0
    style EXIST fill:#d1ecf1,stroke:#17a2b8
    style NEW fill:#fff3cd,stroke:#ffc107
```

---

## 八、cancel / move / reduce 对比

```mermaid
flowchart TB
    subgraph CANCEL["🚫 cancel_order（完全撤单）"]
        direction TB
        CA["order_id_index.get(order_id)"]
        CB{"uid 匹配?"}
        CX["✗ UnknownOrderId"]
        CC["order_id_index.remove"]
        CD["remove_order\n（从价位链表摘除）"]
        CE["orders.remove\n（回收 Slab slot）"]
        CF["push Reject event\nsize = 剩余量"]
        CA --> CB
        CB -->|"否"| CX
        CB -->|"是"| CC --> CD --> CE --> CF
    end

    subgraph MOVE["🔄 move_order（改价重挂）"]
        direction TB
        MA["order_id_index.get(order_id)"]
        MB{"风控检查\nnew_price &gt; reserve?"}
        MX["✗ InvalidReservePrice"]
        MC["remove_order\n（从旧价位移除）"]
        MD["order.price = new_price"]
        ME["⚠️ temp_cmd.size =\norder.size - order.filled\n（传剩余量，不是原始总量）"]
        MF["try_match(temp_cmd)\n→ filled_in_move"]
        MG{"filled_in_move ==\norder.size - order.filled?"}
        MH["order_id_index.remove\norders.remove"]
        MI["⚠️ order.filled += filled_in_move\n（累加，不是覆盖）\ninsert_order 新价位"]
        MA --> MB
        MB -->|"是"| MX
        MB -->|"否"| MC --> MD --> ME --> MF --> MG
        MG -->|"完全成交"| MH
        MG -->|"有余量"| MI
    end

    subgraph REDUCE["➖ reduce_order（减量，保持时间优先级）"]
        direction TB
        RA["order_id_index.get(order_id)"]
        RB{"reduce_by == 剩余量?"}
        RC["全部减掉：\norder_id_index.remove\nremove_order\norders.remove"]
        RD["部分减少：\norder.size -= reduce_by\nbucket.volume -= reduce_by"]
        RE["push Reject event\nsize = reduce_by"]
        RA --> RB
        RB -->|"是"| RC --> RE
        RB -->|"否"| RD --> RE
    end

    style CANCEL fill:#f8d7da,stroke:#dc3545
    style MOVE fill:#fff3cd,stroke:#ffc107
    style REDUCE fill:#d1ecf1,stroke:#17a2b8
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
