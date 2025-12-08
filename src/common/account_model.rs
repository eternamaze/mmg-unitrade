use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

// --- IDs & Identity Space ---

/// 子账户身份 (SubAccount Identity)
/// 系统中存在的有限子账户集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubAccountId {
    Main,
    Sub1,
    Sub2,
    Sub3,
    Sub4,
    Sub5,
    // Future: Add more sub-accounts here
}

impl fmt::Display for SubAccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// 交易所身份 (Venue Identity)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VenueId {
    Binance,
    Okx,
    Bybit,
    Coinbase,
    Mock,
}

impl fmt::Display for VenueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// 资产身份 (Asset Identity)
/// 只有在此枚举中定义的资产才被系统承认。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssetId {
    BTC,
    ETH,
    USDT,
    USDC,
    BNB,
    SOL,
    // Future: Add more assets here
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl FromStr for AssetId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "BTC" => Ok(AssetId::BTC),
            "ETH" => Ok(AssetId::ETH),
            "USDT" => Ok(AssetId::USDT),
            "USDC" => Ok(AssetId::USDC),
            "BNB" => Ok(AssetId::BNB),
            "SOL" => Ok(AssetId::SOL),
            _ => Err(format!("Unknown AssetId: {}", s)),
        }
    }
}

/// 交易对 (Trading Pair)
/// 由两个资产身份组成的逻辑组合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradingPair {
    pub base: AssetId,
    pub quote: AssetId,
}

impl TradingPair {
    pub fn new(base: AssetId, quote: AssetId) -> Self {
        Self { base, quote }
    }

    pub fn symbol(&self) -> String {
        format!("{}{}", self.base, self.quote)
    }
}

impl fmt::Display for TradingPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

/// K线周期 (Kline Interval)
/// 系统支持的有限时间分辨率集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KlineInterval {
    S1,
    M1,
    M3,
    M5,
    M15,
    M30,
    H1,
    H2,
    H4,
    H6,
    H8,
    H12,
    D1,
    D3,
    W1,
    Mo1,
}

impl fmt::Display for KlineInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            KlineInterval::S1 => "1s",
            KlineInterval::M1 => "1m",
            KlineInterval::M3 => "3m",
            KlineInterval::M5 => "5m",
            KlineInterval::M15 => "15m",
            KlineInterval::M30 => "30m",
            KlineInterval::H1 => "1h",
            KlineInterval::H2 => "2h",
            KlineInterval::H4 => "4h",
            KlineInterval::H6 => "6h",
            KlineInterval::H8 => "8h",
            KlineInterval::H12 => "12h",
            KlineInterval::D1 => "1d",
            KlineInterval::D3 => "3d",
            KlineInterval::W1 => "1w",
            KlineInterval::Mo1 => "1M",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for KlineInterval {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1s" => Ok(KlineInterval::S1),
            "1m" => Ok(KlineInterval::M1),
            "3m" => Ok(KlineInterval::M3),
            "5m" => Ok(KlineInterval::M5),
            "15m" => Ok(KlineInterval::M15),
            "30m" => Ok(KlineInterval::M30),
            "1h" => Ok(KlineInterval::H1),
            "2h" => Ok(KlineInterval::H2),
            "4h" => Ok(KlineInterval::H4),
            "6h" => Ok(KlineInterval::H6),
            "8h" => Ok(KlineInterval::H8),
            "12h" => Ok(KlineInterval::H12),
            "1d" => Ok(KlineInterval::D1),
            "3d" => Ok(KlineInterval::D3),
            "1w" => Ok(KlineInterval::W1),
            "1M" => Ok(KlineInterval::Mo1),
            _ => Err(format!("Unknown KlineInterval: {}", s)),
        }
    }
}

// Content Types (Not Identities)
// 这些类型属于内容空间，因为它们是无限的、连续的、不可枚举的。
// 它们只是数据的载体，不具备系统层面的身份安全性。

pub type OrderId = String;
pub type ClientOrderId = String;
pub type TradeId = String;

// --- Domain Models ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Limit,
    Market,
    StopLimit,
    StopMarket,
    TakeProfitLimit,
    TakeProfitMarket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    Gtc, // Good Till Cancel
    Ioc, // Immediate or Cancel
    Fok, // Fill or Kill
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerType {
    LastPrice,
    MarkPrice,
    IndexPrice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionInstruction {
    PostOnly,
    ReduceOnly,
    // Iceberg is usually defined by a visible_quantity field, but we can keep a tag here too if needed.
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

#[derive(Debug, Clone)]
pub struct OrderRequest {
    // Removed routing info (sub_account_id, symbol) to be pure
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<Decimal>,
    pub quantity: Decimal,
    pub client_order_id: Option<ClientOrderId>,

    // Advanced attributes
    pub time_in_force: Option<TimeInForce>,
    pub trigger_price: Option<Decimal>,
    pub trigger_type: Option<TriggerType>,
    /// For Iceberg orders: the quantity to be displayed in the order book.
    /// If None, it's a normal order (fully visible).
    pub visible_quantity: Option<Decimal>,
    pub instructions: Vec<ExecutionInstruction>,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub sub_account_id: SubAccountId,
    pub id: OrderId, // Exchange ID
    pub client_order_id: Option<ClientOrderId>,
    pub symbol: TradingPair,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<Decimal>,
    pub quantity: Decimal,
    pub filled_quantity: Decimal,
    pub average_price: Option<Decimal>,
    pub status: OrderStatus,
    pub update_time: u64,
    // TODO: Add advanced attributes to Order state as well if needed
}

#[derive(Debug, Clone)]
pub struct Position {
    pub sub_account_id: SubAccountId,
    pub symbol: TradingPair,
    pub quantity: Decimal, // 正数为多，负数为空
    pub entry_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub leverage: Decimal,
}

#[derive(Debug, Clone)]
pub struct Balance {
    pub sub_account_id: SubAccountId,
    pub asset: AssetId,
    pub total: Decimal,
    pub available: Decimal,
    pub locked: Decimal,
}
