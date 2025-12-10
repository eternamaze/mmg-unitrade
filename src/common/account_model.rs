use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug, Display};
use std::hash::Hash;
use std::str::FromStr;

// --- IDs & Identity Space ---

/// 子账户身份特征 (SubAccount Identity Trait)
/// 任何想要作为子账户身份的类型必须实现此 Trait。
pub trait SubAccountIdentity:
    Debug + Clone + Copy + PartialEq + Eq + Hash + Serialize + for<'a> Deserialize<'a> + Display + FromStr + Send + Sync + 'static
{
}

impl<T> SubAccountIdentity for T where T: Debug + Clone + Copy + PartialEq + Eq + Hash + Serialize + for<'a> Deserialize<'a> + Display + FromStr + Send + Sync + 'static {}

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

/// 资产身份特征 (Asset Identity Trait)
/// 任何想要作为资产身份的类型必须实现此 Trait。
/// 这实现了资产定义的控制反转 (IoC)。
/// 
/// 要求：
/// - Debug, Display: 用于日志和显示
/// - Clone, Copy: 资产ID应该是轻量级的，像整数或短字符串枚举
/// - PartialEq, Eq, Hash: 用于作为 HashMap 的 Key
/// - Serialize, Deserialize: 用于序列化
/// - FromStr: 用于从字符串解析 (如 API 返回 "BTC" -> Asset::BTC)
/// - Send, Sync: 用于多线程
pub trait AssetIdentity:
    Debug + Clone + Copy + PartialEq + Eq + Hash + Serialize + for<'a> Deserialize<'a> + Display + FromStr + Send + Sync + 'static
{
}

// 自动为满足条件的类型实现 AssetIdentity
impl<T> AssetIdentity for T where T: Debug + Clone + Copy + PartialEq + Eq + Hash + Serialize + for<'a> Deserialize<'a> + Display + FromStr + Send + Sync + 'static {}

/// 交易对 (Trading Pair)
/// 由两个资产身份组成的逻辑组合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(bound = "A: AssetIdentity")]
pub struct TradingPair<A: AssetIdentity> {
    pub base: A,
    pub quote: A,
}

impl<A: AssetIdentity> TradingPair<A> {
    pub fn new(base: A, quote: A) -> Self {
        Self { base, quote }
    }

    pub fn symbol(&self) -> String {
        format!("{}{}", self.base, self.quote)
    }
}

impl<A: AssetIdentity> fmt::Display for TradingPair<A> {
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
pub struct Order<A: AssetIdentity, S: SubAccountIdentity> {
    pub sub_account_id: S,
    pub id: OrderId, // Exchange ID
    pub client_order_id: Option<ClientOrderId>,
    pub symbol: TradingPair<A>,
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
pub struct Position<A: AssetIdentity, S: SubAccountIdentity> {
    pub sub_account_id: S,
    pub symbol: TradingPair<A>,
    pub quantity: Decimal, // 正数为多，负数为空
    pub entry_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub leverage: Decimal,
}

#[derive(Debug, Clone)]
pub struct Balance<A: AssetIdentity, S: SubAccountIdentity> {
    pub sub_account_id: S,
    pub asset: A,
    pub total: Decimal,
    pub available: Decimal,
    pub locked: Decimal,
}
