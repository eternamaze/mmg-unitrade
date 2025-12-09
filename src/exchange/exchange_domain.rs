/// 业务域标签 (Domain Tag)
///
/// 这是一个标记 Trait，用于标识一个类型是“业务域标签”。
/// 业务域标签用于在 `AccessChannel` 中区分不同的信道。
pub trait DomainTag: Send + Sync + 'static {}

// --- Market Data Domains (Feed) ---

/// 订单簿域 (L2/L3)
pub struct OrderBookDomain;
impl DomainTag for OrderBookDomain {}

/// 成交流域 (Trade Flow / Ticker)
pub struct TradeFlowDomain;
impl DomainTag for TradeFlowDomain {}

/// K线域 (Candlestick / OHLCV)
pub struct KlineDomain;
impl DomainTag for KlineDomain {}

// --- Trading Domains (Gateway) ---

/// 订单管理域 (Order Management)
pub struct OrderDomain;
impl DomainTag for OrderDomain {}

/// 持仓管理域 (Position Management)
pub struct PositionDomain;
impl DomainTag for PositionDomain {}

/// 资产余额域 (Balance / Asset)
pub struct BalanceDomain;
impl DomainTag for BalanceDomain {}

// --- Network Domains (Connector) ---

/// 网络控制域 (Network Control)
pub struct NetworkControlDomain;
impl DomainTag for NetworkControlDomain {}

/// 网络状态域 (Network Status)
pub struct NetworkStatusDomain;
impl DomainTag for NetworkStatusDomain {}
