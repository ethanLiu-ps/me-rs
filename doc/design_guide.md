# 一、总体架构（交易所级模型）

```
                ┌──────────────┐
                │  API Gateway │
                └──────┬───────┘
                       │
                ┌──────▼───────┐
                │  Sequencer   │  ← 全局顺序生成
                └──────┬───────┘
                       │
        ┌──────────────┼──────────────┐
        ▼                              ▼
┌──────────────┐               ┌──────────────┐
│  Shard A     │               │  Shard B     │
│  (BTC_USDT)  │               │  (ETH_USDT)  │
└──────┬───────┘               └──────┬───────┘
       │                              │
┌──────▼──────────────────────────────▼──────┐
│         Single Thread Matching Core        │
└─────────────────────────────────────────────┘
       │
┌──────▼──────┐
│   EventBus  │
└──────┬──────┘
       │
┌──────▼──────────┐
│  Ledger / Risk  │
└─────────────────┘
```

---

# 二、核心原则（必须遵守）

### 1️⃣ 每个产品 = 单线程撮合

```
BTC_USDT → 线程1
ETH_USDT → 线程2
```

原因：

- 保证 price-time priority
- 保证 determinism
- 保证 replay 一致性
- 避免锁

---

### 2️⃣ 不允许并发修改 OrderBook

所有订单修改必须：

```
经过 sequencer
进入 ring buffer
由单线程处理
```

这是交易所生命线。

---

# 三、Sequencer 设计（公平性核心）

Sequencer 是：

> 所有订单的唯一顺序来源
> 

它必须：

- 单点生成 sequence
- 原子递增
- 绝对顺序

示例（Rust）：

```rust
struct Sequencer {
    seq: AtomicU64
}

fn next(&self) -> u64 {
    self.seq.fetch_add(1, Ordering::SeqCst)
}
```

所有请求：

```
API → Sequencer → 分发到 shard
```

---

# 四、RingBuffer + Pipeline（撮合核心）

采用 LMAX Disruptor 模式：

```
Producer (API)
    ↓
RingBuffer
    ↓
Consumer (Matching Thread)
```

每个 shard 有独立 ring buffer：

```
BTC_USDT shard:
    ring buffer (size=2^16)
    matching thread
```

---

## Pipeline 阶段

单线程处理顺序：

```
1. Validate
2. Risk check
3. Match
4. Update book
5. Generate trade events
6. Journal append
7. Publish events
```

全部在同一线程。

---

# 五、Risk 与 Matching 解耦

不要在撮合线程做复杂风控。

设计：

```
PreRisk (快速检查) → 撮合
PostRisk (异步审计)
```

PreRisk 只做：

- 余额是否足够
- 是否超过限额
- 是否冻结账户

必须 O(1)。

---

# 六、WAL + Snapshot + Replay（交易所必须）

### 1️⃣ WAL

每条指令写入 WAL：

```
append-only log
```

要求：

- 顺序写
- batch flush
- 不阻塞撮合线程

---

### 2️⃣ Snapshot

定期生成：

```
orderbook snapshot
+ balances snapshot
```

用于：

- 快速恢复
- 灾难重启

---

### 3️⃣ Replay

重启流程：

```
load snapshot
replay WAL
恢复一致状态
```

这要求：

> 撮合逻辑 100% deterministic
> 

---

# 七、分片设计（横向扩展）

扩展方式不是：

```
一个 book 多线程
```

而是：

```
多产品 → 多 shard
```

例如：

```
Shard 1 → BTC_USDT
Shard 2 → ETH_USDT
Shard 3 → SOL_USDT
```

可部署为：

```
matching-node-1
matching-node-2
matching-node-3
```

每个 shard：

- 独立内存
- 独立 WAL
- 独立 CPU core

---

# 八、性能目标（现实可达）

单 shard：

- 1–3 million orders/sec
- P99 < 10us
- 单笔处理 < 1us

10 shards：

- 10–20 million/sec

瓶颈通常不是撮合，而是：

- 网络
- JSON
- persistence

---

# 九、核心优化策略

### 1️⃣ 数据结构

- SOA 内存布局
- slab allocator
- 预分配 order pool

---

### 2️⃣ 避免 GC（Go）

- sync.Pool
- 无 map 扩容
- 无 interface{}

---

### 3️⃣ Cache 友好

- 价格 level 用 array + index
- 不用 tree（除非必要）

---

### 4️⃣ 批处理

- 批量 WAL flush
- 批量 publish

---

# 十、完整工程模块划分

```
matching-core/
 ├── sequencer/
 ├── shard/
 │     ├── ringbuffer
 │     ├── matching_engine
 │     ├── orderbook
 │     ├── risk
 │     ├── journal
 │     └── snapshot
 ├── gateway/
 ├── eventbus/
 └── ledger/
```

---

# 十一、为什么这是“交易所级”

✔ 单线程 deterministic

✔ 可 replay

✔ 可审计

✔ 可分片

✔ 可横向扩展

✔ 支持冷热恢复

✔ 性能可预测

---

# 十二、最后给你一句核心思想

> 交易所撮合的高性能，不来自“并发”，
> 
> 
> 而来自“单线程极致优化 + 分片横向扩展”。
>
me/
├─ crates/
│  ├─ core/                # Performance Core（绝对零抽象、无 IO）
│  │  ├─ src/
│  │  │  ├─ model.rs       # Command/Event/IDs/Price/Qty 等 primitives
│  │  │  ├─ book/          # OrderBook + price-time priority
│  │  │  ├─ engine.rs      # Matching loop（单线程状态机）
│  │  │  ├─ risk.rs        # PreRisk（O(1) 快速检查）
│  │  │  └─ replay.rs      # 纯逻辑 replay（读的是 decoded Command）
│  │  └─ Cargo.toml
│  ├─ app/                 # Application Layer（用例编排 + Ports）
│  │  ├─ src/
│  │  │  ├─ ports.rs       # trait ports：Wal, Publisher, Clock, Balances...
│  │  │  ├─ usecase.rs     # place/cancel/amend orchestration
│  │  │  └─ router.rs      # shard 路由（symbol -> shard）
│  │  └─ Cargo.toml
│  ├─ adapters/            # 具体实现（NATS/Kafka, file WAL, HTTP/FIX, DB writer）
│  │  ├─ src/
│  │  │  ├─ wal_file.rs
│  │  │  ├─ pub_nats.rs
│  │  │  ├─ ingress_http.rs
│  │  │  ├─ ingress_fix.rs
│  │  │  └─ persistence.rs
│  │  └─ Cargo.toml
│  └─ bin/                 # 组装：把 core + app + adapters wiring 在一起
│     ├─ src/main.rs
│     └─ Cargo.toml
└─ Cargo.toml
