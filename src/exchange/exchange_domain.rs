use crate::common::account_model::{
    Balance, ClientOrderId, KlineInterval, Order, OrderId, OrderRequest, Position, SubAccountId,
};
use thiserror::Error;

/// 业务域标签 (Domain Tag)
///
/// 这是一个标记 Trait，用于标识一个类型是“业务域标签”。
/// 业务域标签用于在 `AccessChannel` 中区分不同的信道。
pub trait DomainTag: Send + Sync + 'static {}

// --- Market Data Domains (Feed) ---

/// 订单簿域 (L2/L3)
#[derive(Debug, Clone, Copy)]
pub struct OrderBookDomain;
impl DomainTag for OrderBookDomain {}

/// 成交流域 (Trade Flow / Ticker)
#[derive(Debug, Clone, Copy)]
pub struct TradeFlowDomain;
impl DomainTag for TradeFlowDomain {}

/// K线域 (Candlestick / OHLCV)
#[derive(Debug, Clone, Copy)]
pub struct KlineDomain;
impl DomainTag for KlineDomain {}

// --- Trading Domains (Gateway) ---

/// 订单管理域 (Order Management)
#[derive(Debug, Clone, Copy)]
pub struct OrderDomain;
impl DomainTag for OrderDomain {}

/// 持仓管理域 (Position Management)
#[derive(Debug, Clone, Copy)]
pub struct PositionDomain;
impl DomainTag for PositionDomain {}

/// 资产余额域 (Balance / Asset)
#[derive(Debug, Clone, Copy)]
pub struct BalanceDomain;
impl DomainTag for BalanceDomain {}

// --- Network Domains (Connector) ---

/// 网络控制域 (Network Control)
#[derive(Debug, Clone, Copy)]
pub struct NetworkControlDomain;
impl DomainTag for NetworkControlDomain {}

/// 网络状态域 (Network Status)
#[derive(Debug, Clone, Copy)]
pub struct NetworkStatusDomain;
impl DomainTag for NetworkStatusDomain {}

/// 连接器控制域 (Connector Control)
/// 用于向连接器发送通用请求 (订阅、下单等)
#[derive(Debug, Clone, Copy)]
pub struct ConnectorControlDomain;
impl DomainTag for ConnectorControlDomain {}

// ================================================================================================
// Domain Specific Languages (DSL)
// ================================================================================================

// --- Connector DSL (Internal) ---

#[derive(Debug, Clone)]
pub enum NetworkCommand {
    /// 强制重连
    Reconnect,
    /// 断开连接（通常用于停机维护）
    Disconnect,
    /// 熔断：要求连接器尽可能快地取消所有挂单并停止交易
    CircuitBreak,
}

#[derive(Debug, Clone)]
pub enum NetworkStatus {
    Connected,
    Disconnected,
    Reconnecting,
    /// 严重错误，无法自动恢复
    FatalError(String),
    /// 当前网络延迟 (毫秒)
    Latency(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelType {
    OrderBook,
    TradeFlow,
    Kline(KlineInterval),
    AccountUpdate, // Balance/Position/Order updates
}

/// 连接器请求协议
/// Feed 和 Gateway 通过此协议向 Connector 发送请求。
/// 这是 Connector 域的“普通话”。
///
/// 使用泛型 `I` (Instrument) 来确保身份安全。
#[derive(Debug, Clone)]
pub enum ConnectorRequest<I> {
    // --- Feed Requests ---
    /// 订阅行情
    Subscribe {
        instrument: I,
        channel_type: ChannelType,
    },
    /// 取消订阅
    Unsubscribe {
        instrument: I,
        channel_type: ChannelType,
    },

    // --- Gateway Requests (Execution) ---
    /// 发送订单
    SubmitOrder {
        sub_account_id: SubAccountId,
        instrument: I,
        req: OrderRequest,
    },
    /// 取消订单
    CancelOrder {
        sub_account_id: SubAccountId,
        instrument: I,
        order_id: OrderId,
    },
    /// 修改订单
    AmendOrder {
        sub_account_id: SubAccountId,
        instrument: I,
        order_id: OrderId,
        req: OrderRequest,
    },

    // --- Gateway Requests (Batch Execution) ---
    /// 批量发送订单
    BatchSubmitOrder {
        sub_account_id: SubAccountId,
        instrument: I,
        reqs: Vec<OrderRequest>,
        atomic: bool,
    },
    /// 批量取消订单
    BatchCancelOrder {
        sub_account_id: SubAccountId,
        instrument: I,
        order_ids: Vec<OrderId>,
        atomic: bool,
    },
    /// 批量修改订单
    BatchAmendOrder {
        sub_account_id: SubAccountId,
        instrument: I,
        amends: Vec<(OrderId, OrderRequest)>,
        atomic: bool,
    },

    // --- Gateway Requests (Query) ---
    /// 查询当前挂单
    FetchOpenOrders {
        sub_account_id: SubAccountId,
        instrument: Option<I>,
    },
    /// 查询特定订单
    FetchOrder {
        sub_account_id: SubAccountId,
        instrument: I,
        order_id: OrderId,
    },
    /// 查询持仓
    FetchPositions {
        sub_account_id: SubAccountId,
        instrument: Option<I>,
    },
    /// 查询余额
    FetchBalances { sub_account_id: SubAccountId },
}

// --- Feed DSL ---

/// Feed 控制指令
/// 策略层通过此指令控制 Feed 的行为。
/// 使用泛型 `I` (Instrument) 来指定操作对象。
#[derive(Debug, Clone)]
pub enum FeedCommand<I> {
    /// 订阅特定标的的特定数据流
    Subscribe {
        instrument: I,
        channel_type: ChannelType,
    },
    /// 取消订阅
    Unsubscribe {
        instrument: I,
        channel_type: ChannelType,
    },
    /// 请求立即推送一次全量快照
    RequestSnapshot { instrument: I },
    /// 暂停推送（节省带宽）
    Suspend { instrument: I },
    /// 恢复推送
    Resume { instrument: I },
    /// 设置聚合频率（毫秒），0 表示实时推送
    SetFrequency { instrument: I, interval_ms: u64 },
}

// --- Gateway DSL ---

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Exchange error: {0}")]
    Exchange(String),
    #[error("Order not found")]
    OrderNotFound,
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

// --- Order Domain ---
#[derive(Debug, Clone)]
pub enum OrderCommand<I> {
    // Single Operations
    Create {
        sub_account_id: SubAccountId,
        instrument: I,
        req: OrderRequest,
    },
    Cancel {
        sub_account_id: SubAccountId,
        instrument: I,
        id: OrderId,
    },
    Amend {
        sub_account_id: SubAccountId,
        instrument: I,
        id: OrderId,
        req: OrderRequest,
    },

    // Batch Operations
    BatchCreate {
        sub_account_id: SubAccountId,
        instrument: I,
        reqs: Vec<OrderRequest>,
        /// If true, the batch is atomic (all or nothing), if supported by exchange.
        atomic: bool,
    },
    BatchCancel {
        sub_account_id: SubAccountId,
        instrument: I,
        ids: Vec<OrderId>,
        atomic: bool,
    },
    BatchAmend {
        sub_account_id: SubAccountId,
        instrument: I,
        amends: Vec<(OrderId, OrderRequest)>,
        atomic: bool,
    },

    // Query Operations
    GetOpenOrders {
        sub_account_id: SubAccountId,
        instrument: Option<I>,
    },
    GetOrder {
        sub_account_id: SubAccountId,
        instrument: I,
        id: OrderId,
    },
}

#[derive(Debug, Clone)]
pub enum OrderUpdate<I> {
    // Command Results (Ack/Nack)
    RequestAccepted {
        sub_account_id: SubAccountId,
        id: OrderId,
        client_id: Option<ClientOrderId>,
    },
    RequestRejected {
        sub_account_id: SubAccountId,
        id: Option<OrderId>,
        client_id: Option<ClientOrderId>,
        reason: String,
    },

    // State Changes (Push from Exchange)
    OrderNew(Order),
    OrderFilled(Order),
    OrderCanceled {
        sub_account_id: SubAccountId,
        id: OrderId,
        instrument: I,
    },
    OrderRejected {
        sub_account_id: SubAccountId,
        id: Option<OrderId>,
        reason: String,
    },
    OrderExpired {
        sub_account_id: SubAccountId,
        id: OrderId,
        instrument: I,
    },

    // Query Results
    OrderSnapshot(Vec<Order>),

    // Error
    Error(String),
}

// --- Position Domain ---
#[derive(Debug, Clone)]
pub enum PositionCommand<I> {
    Subscribe {
        sub_account_id: SubAccountId,
    },
    Query {
        sub_account_id: SubAccountId,
        instrument: Option<I>,
    },
}

#[derive(Debug, Clone)]
pub enum PositionUpdate {
    Snapshot(Vec<Position>),
    Update(Position),
    Error(String),
}

// --- Balance Domain ---
#[derive(Debug, Clone)]
pub enum BalanceCommand {
    Subscribe { sub_account_id: SubAccountId },
    Query { sub_account_id: SubAccountId },
}

#[derive(Debug, Clone)]
pub enum BalanceUpdate {
    Snapshot(Vec<Balance>),
    Update(Balance),
    Error(String),
}
