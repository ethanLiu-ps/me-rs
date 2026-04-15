# Symbol Specification — `CoreSymbolSpecification`

> **权威文档**：本文件定义撮合引擎接受的交易对配置规格，是 pre-match 校验的唯一依据。
> 所有违反此规格的订单必须在进入撮合循环之前被拒绝（`CmdResultCode::InvalidSymbol` 或 `MatchingInvalidOrderSize`）。

---

## 一、字段全集（Phase 1）

### 基础标识（已实现）

| 字段 | 类型 | 说明 |
|------|------|------|
| `symbol_id` | `SymbolId` | 交易对唯一标识，用于撮合引擎分片路由 |
| `symbol_type` | `SymbolType` | 品种类型：Spot / FutureContract / PerpetualSwap / CallOption / PutOption |
| `base_currency` | `Currency` | 基础货币（如 BTC/USDT 中的 BTC） |
| `quote_currency` | `Currency` | 计价货币（如 BTC/USDT 中的 USDT） |

### 精度与手续费（已实现）

| 字段 | 类型 | 说明 |
|------|------|------|
| `base_scale_k` | `i64` | 基础货币精度系数；卖单冻结量 = `size × base_scale_k` |
| `quote_scale_k` | `i64` | 计价货币精度系数；买单冻结量 = `size × price × quote_scale_k` |
| `taker_fee` | `i64` | 吃单手续费率（每单位数量，固定点数），下单冻结时额外计入 |
| `maker_fee` | `i64` | 挂单手续费率（每单位数量），结算时从成交额中扣除 |
| `margin_buy` | `i64` | 买方保证金比例（衍生品；现货为 0） |
| `margin_sell` | `i64` | 卖方保证金比例（衍生品；现货为 0） |

### P0 — Pre-match 合法性校验（新增）

**必须全部实现。** 任何缺失都会导致非法订单漏进撮合循环。

| 字段 | 类型 | 说明 | Bybit 对应 |
|------|------|------|-----------|
| `tick_size` | `Price` | 价格最小步长。订单价格必须满足 `price % tick_size == 0`。 | `tickSize` |
| `lot_size` | `Size` | 数量最小步长。订单数量必须满足 `size % lot_size == 0`。 | `basePrecision` |
| `min_qty` | `Size` | 最小下单数量（已按 lot_size 对齐）。`size < min_qty` 则拒绝。 | `minOrderQty` |
| `min_notional` | `i64` | 最小名义价值（`size × price`）。低于此值拒绝。对现货尤其重要。 | `minOrderAmt` |
| `max_limit_qty` | `Size` | 限价单最大数量上限。`size > max_limit_qty` 则拒绝。 | `maxLimitOrderQty` |
| `max_market_qty` | `Size` | 市价单最大数量上限。`size > max_market_qty` 则拒绝。 | `maxMarketOrderQty` |

**校验顺序（pre-match，优先于一切）：**
```
1. tick_size 整除校验（price）
2. lot_size 整除校验（size）
3. min_qty 下限
4. max_limit_qty / max_market_qty 上限（按订单类型区分）
5. min_notional（size × price >= min_notional）
```

### P1 — 价格保护（新增）

防止极端偏离市价的挂单扰乱订单簿。

| 字段 | 类型 | 说明 | Bybit 对应 |
|------|------|------|-----------|
| `price_limit_ratio` | `i64` | 限价单价格偏离参考价的最大比例，单位 bps（1 bps = 0.01%）。`0` 表示不限制。 | `priceLimitRatioX/Y` |

**校验逻辑（由 MatchingEngine 在撮合入口执行，因为需要读订单簿状态）：**

参考价格优先级（`ref_price`）：
1. 订单簿双侧均有挂单 → `(best_bid + best_ask) / 2`（mid-price）
2. 仅单侧有挂单 → 取存在的那一侧价格
3. 订单簿为空 → 取 `last_trade_price` 兜底
4. `last_trade_price` 也为 0（全新交易对冷启动）→ 跳过价格保护校验

使用 mid-price 而非 last_trade_price 的原因：
- **抗操纵**：移动 mid-price 需要同时推动 bid 和 ask 两侧；而 last_trade_price 仅需一笔小成交即可拉偏
- **实时性**：流动性差的盘口 last_trade_price 可能极度滞后；mid-price 始终反映当前盘口
- **成本为零**：撮合引擎已持有 best_bid/best_ask 指针（DirectOrderBook 链表头），无额外查找开销

具体判断：
- 买单：`price > ref_price × (1 + price_limit_ratio / 10000)` → 拒绝
- 卖单：`price < ref_price × (1 - price_limit_ratio / 10000)` → 拒绝

### P2 — 运营状态（新增）

控制引擎是否接受该交易对的新订单。

| 字段 | 类型 | 说明 |
|------|------|------|
| `status` | `SymbolStatus` | `Trading`：正常接单；`Suspended`：暂停，拒绝所有新单；`PreDelivery`：期货结算期，仅允许平仓。 |

**校验逻辑：** Pipeline 的第一步——若 `status != Trading`，直接返回 `CmdResultCode::InvalidSymbol`，不进入风控和撮合。

### P3 — 自成交预防（新增）

CLAUDE.md 明确规定 STP 为所有交易对的强制要求。

| 字段 | 类型 | 说明 |
|------|------|------|
| `stp_mode` | `StpMode` | `CancelNew`（默认）：拒绝新进来的单，保留挂单；`CancelOld`：取消挂单，接受新单；`CancelBoth`：双方都取消。 |

**校验时机：** 价格/时间撮合命中之后、生成 Trade 事件之前。发现 `maker_uid == taker_uid` 时按 `stp_mode` 处理。

---

## 二、完整字段结构（Rust 参考）

```rust
pub enum SymbolStatus {
    Trading,      // 正常交易
    Suspended,    // 暂停（拒绝所有新单）
    PreDelivery,  // 期货结算期（仅允许平仓）
}

pub enum StpMode {
    CancelNew,   // 拒绝新进来的 taker（默认，保留 maker 公平性）
    CancelOld,   // 取消 maker 挂单，接受新 taker
    CancelBoth,  // 双方都取消
}

pub struct CoreSymbolSpecification {
    // --- 基础标识 ---
    pub symbol_id: SymbolId,
    pub symbol_type: SymbolType,
    pub base_currency: Currency,
    pub quote_currency: Currency,

    // --- 精度与手续费 ---
    pub base_scale_k: i64,
    pub quote_scale_k: i64,
    pub taker_fee: i64,
    pub maker_fee: i64,
    pub margin_buy: i64,
    pub margin_sell: i64,

    // --- P0: pre-match 合法性校验（必须） ---
    pub tick_size: Price,        // 价格最小步长
    pub lot_size: Size,          // 数量最小步长
    pub min_qty: Size,           // 最小下单量
    pub min_notional: i64,       // 最小名义价值（price × qty）
    pub max_limit_qty: Size,     // 限价单最大量
    pub max_market_qty: Size,    // 市价单最大量

    // --- P1: 价格保护 ---
    pub price_limit_ratio: i64,  // 单位 bps，0 = 不限制

    // --- P2: 运营状态 ---
    pub status: SymbolStatus,

    // --- P3: 自成交预防 ---
    pub stp_mode: StpMode,
}
```

---

## 三、Default 值约定

| 字段 | Default | 说明 |
|------|---------|------|
| `tick_size` | `1` | 最细精度，不限制（由 scale_k 隐含） |
| `lot_size` | `1` | 最细精度 |
| `min_qty` | `1` | 至少下 1 单位 |
| `min_notional` | `0` | 不限制最小名义 |
| `max_limit_qty` | `i64::MAX` | 不限制 |
| `max_market_qty` | `i64::MAX` | 不限制 |
| `price_limit_ratio` | `0` | 不启用价格保护 |
| `status` | `SymbolStatus::Trading` | 默认可交易 |
| `stp_mode` | `StpMode::CancelNew` | 保护 maker 公平性 |

---

## 四、与 Bybit v5 API 的对应关系（现货）

| Bybit 字段 | 本引擎字段 | 备注 |
|-----------|-----------|------|
| `tickSize` | `tick_size` | Bybit 为浮点字符串；本引擎为整型固定点 |
| `basePrecision` | `lot_size` | qty step |
| `quotePrecision` | `quote_scale_k` | 精度系数 |
| `minOrderQty` | `min_qty` | |
| `minOrderAmt` | `min_notional` | 名义价值下限 |
| `maxLimitOrderQty` | `max_limit_qty` | |
| `maxMarketOrderQty` | `max_market_qty` | |
| `priceLimitRatioX/Y` | `price_limit_ratio` | 简化为单一比例 |
| `status` | `status` | |
| N/A | `stp_mode` | Bybit 在账户层控制，本引擎在撮合层强制 |
