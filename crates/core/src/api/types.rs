use serde::{Deserialize, Serialize};

pub type Size = i64;
pub type Price = i64;
pub type UserId = u64;
pub type OrderId = u64;
pub type SymbolId = String;
pub type Currency = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderAction {
    // 买
    Bid,
    // 卖
    Ask,
}

impl OrderAction {
    pub fn opposite(self) -> OrderAction {
        match self {
            OrderAction::Bid => OrderAction::Ask,
            OrderAction::Ask => OrderAction::Bid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    // good-til-cancelled
    GTC,
    // immediate-or-cancel
    IOC,
    // immediate-or-cancel with budget
    IOCWithBudget,
    // fill-or-kill
    FOK,
    // fok with budget
    FOKWithBudget,
    // post only, 只做maker，不吃单
    PostOnly,
    // 止损限价单
    StopLimit,
    // 止损市价单
    StopMaket,
    Iceberg,
    // 当日有效
    Day,
    // good-til-date
    GTD(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolType {
    // 现货
    Spot,
    // 期货
    FutureContract,
    // 永续合约
    PerpetualSwap,
    // 看涨期权
    CallOption,
    // 看跌期权
    PutOption,
}

// P2: 交易对运营状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolStatus {
    // 正常接单
    Trading,
    // 暂停，拒绝所有新单
    Suspended,
    // 期货结算期，仅允许平仓
    PreDelivery,
}

// P3: 自成交预防策略（STP）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StpMode {
    // 拒绝新进来的 taker，保留 maker（默认，保护 maker 公平性）
    CancelNew,
    // 取消 maker 挂单，接受新 taker
    CancelOld,
    // 双方都取消
    CancelBoth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmdResultCode {
    New,
    ValidForMatchingEngine,
    Success,
    Accepted,

    // matching
    MatchingInvalidOrderbookId,
    MatchingUnknownOrderId,
    MatchingUnsupportedCommand,
    MatchingMoveFailedPriceOverRiskLimit,
    MatchingReduceFailedWrongSize,
    MatchingInvalidOrderSize,

    // other
    InvalidSymbol,
    UnsupportedSymbolType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreSymbolSpecification {
    // --- 基础标识 ---

    // 交易对唯一标识，用于撮合引擎分片路由
    pub symbol_id: SymbolId,
    // 交易品种类型（现货/期货/永续/期权）
    pub symbol_type: SymbolType,
    // 基础货币（如 BTC/USDT 中的 BTC）
    pub base_currency: Currency,
    // 计价货币（如 BTC/USDT 中的 USDT）
    pub quote_currency: Currency,

    // --- 精度与手续费 ---

    // 基础货币精度系数，卖单冻结量 = size × base_scale_k
    pub base_scale_k: i64,
    // 计价货币精度系数，买单冻结量 = size × price × quote_scale_k
    pub quote_scale_k: i64,
    // 吃单手续费率（每单位数量），下单冻结时额外计入
    pub taker_fee: i64,
    // 挂单手续费率（每单位数量），结算时从成交额中扣除
    pub maker_fee: i64,
    // 买方保证金比例（衍生品场景，现货为 0）
    pub margin_buy: i64,
    // 卖方保证金比例（衍生品场景，现货为 0）
    pub margin_sell: i64,

    // --- P0: pre-match 合法性校验 ---

    // 价格最小步长；订单价格必须满足 price % tick_size == 0
    pub tick_size: Price,
    // 数量最小步长；订单数量必须满足 size % lot_size == 0
    pub lot_size: Size,
    // 最小下单数量（已按 lot_size 对齐）
    pub min_qty: Size,
    // 最小名义价值（size × price）；低于此值拒绝
    pub min_notional: i64,
    // 限价单最大数量上限
    pub max_limit_qty: Size,
    // 市价单最大数量上限
    pub max_market_qty: Size,

    // --- P1: 价格保护 ---

    // 限价单价格偏离 mid-price 的最大比例，单位 bps（1 bps = 0.01%）；0 表示不限制
    // 参考价优先级：mid-price → 单侧最优价 → last_trade_price → 跳过校验（冷启动）
    pub price_limit_ratio: i64,

    // --- P2: 运营状态 ---

    // 交易对当前状态，控制引擎是否接受新订单
    pub status: SymbolStatus,

    // --- P3: 自成交预防（STP）---

    // 自成交预防策略；STP 在价格/时间匹配后、Trade 事件生成前执行
    pub stp_mode: StpMode,
}

impl Default for CoreSymbolSpecification {
    fn default() -> Self {
        Self {
            symbol_id: "".to_string(),
            symbol_type: SymbolType::Spot,
            base_currency: "".to_string(),
            quote_currency: "".to_string(),
            base_scale_k: 1,
            quote_scale_k: 1,
            taker_fee: 0,
            maker_fee: 0,
            margin_buy: 0,
            margin_sell: 0,
            tick_size: 1,
            lot_size: 1,
            min_qty: 1,
            min_notional: 0,
            max_limit_qty: i64::MAX,
            max_market_qty: i64::MAX,
            price_limit_ratio: 0,
            status: SymbolStatus::Trading,
            stp_mode: StpMode::CancelNew,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2MarketData {
    pub ask_prices: Vec<Price>,
    pub ask_volumes: Vec<Size>,
    pub bid_prices: Vec<Price>,
    pub bid_volumes: Vec<Size>,
}

impl L2MarketData {
    pub fn new(depth: usize) -> Self {
        Self {
            ask_prices: Vec::with_capacity(depth),
            ask_volumes: Vec::with_capacity(depth),
            bid_prices: Vec::with_capacity(depth),
            bid_volumes: Vec::with_capacity(depth),
        }
    }
}
