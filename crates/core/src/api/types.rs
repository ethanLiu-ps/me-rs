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
    CurrencyExchangePair,
    // 期货
    FutureContract,
    // 永续合约
    PerpetualSwap,
    // 看涨期权
    CallOption,
    // 看跌期权
    PutOption,
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
    MatchingUnsupportedOrderType,
    MatchingMoveFailedPriceOverRiskLimit,
    MatchingReduceFailedWrongSize,
    MatchingInvalidOrderSize,

    // other
    InvalidSymbol,
    UnsupportedSymbolType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreSymbolSpecification {
    // 交易对唯一标识，用于撮合引擎分片路由
    pub symbol_id: SymbolId,
    // 交易品种类型（现货/期货/永续/期权）
    pub symbol_type: SymbolType,
    // 基础货币（如 BTC/USDT 中的 BTC）
    pub base_currency: Currency,
    // 计价货币（如 BTC/USDT 中的 USDT）
    pub quote_currency: Currency,
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
}

impl Default for CoreSymbolSpecification {
    fn default() -> Self {
        Self {
            symbol_id: "".to_string(),
            symbol_type: SymbolType::CurrencyExchangePair,
            base_currency: "".to_string(),
            quote_currency: "".to_string(),
            base_scale_k: 0,
            quote_scale_k: 0,
            taker_fee: 0,
            maker_fee: 0,
            margin_buy: 0,
            margin_sell: 0,
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
