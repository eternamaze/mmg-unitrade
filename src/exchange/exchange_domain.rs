use crate::common::account_model::{
    AssetIdentity, Balance, ClientOrderId, KlineInterval, Order, OrderId, OrderRequest, Position, SubAccountIdentity,
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

/// 网络控制指令集
///
/// 这些指令用于控制连接器的底层网络行为。
/// 它们通常由系统监控组件（如 CircuitBreaker 或 Admin Dashboard）发出。
#[derive(Debug, Clone)]
pub enum NetworkCommand {
    /// 强制重连
    ///
    /// 指示连接器立即断开当前连接并尝试重新建立连接。
    /// 这通常用于处理网络僵死或长时间无数据的情况。
    Reconnect,

    /// 断开连接
    ///
    /// 指示连接器断开连接并停止尝试重连。
    /// 通常用于系统停机维护或优雅退出。
    Disconnect,

    /// 熔断
    ///
    /// 一种紧急状态指令。指示连接器：
    /// 1. 立即断开网络连接。
    /// 2. (如果可能) 在断开前尝试取消所有挂单。
    /// 3. 拒绝后续的所有非恢复性请求。
    CircuitBreak,
}

/// 网络状态原语
///
/// 描述连接器当前的网络健康状况。
/// 连接器会通过广播通道定期或在状态变化时推送此状态。
#[derive(Debug, Clone)]
pub enum NetworkStatus {
    /// 已连接且正常工作
    Connected,
    /// 已断开连接
    Disconnected,
    /// 正在尝试建立连接
    Reconnecting,
    /// 发生严重错误，无法自动恢复
    ///
    /// 此时通常需要人工干预或系统级重启。
    /// 包含错误描述信息。
    FatalError(String),
    /// 当前网络延迟 (毫秒)
    ///
    /// 这是一个特殊的“状态”，用于推送心跳延迟数据。
    /// 它不代表连接状态的改变，而是连接质量的指标。
    Latency(u64),
}

/// 数据通道类型原语
///
/// 枚举了系统支持的所有标准数据流类型。
/// 用于在订阅请求中指定需要的数据。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelType {
    /// 订单簿数据流 (OrderBook)
    ///
    /// 通常指增量更新或快照推送，用于维护本地订单簿副本。
    OrderBook,

    /// 成交流数据流 (Trade / Ticker)
    ///
    /// 逐笔成交数据，包含价格、数量、方向和时间戳。
    TradeFlow,

    /// K线数据流 (Candlestick)
    ///
    /// 包含指定时间周期的 OHLCV 数据。
    /// 携带一个 `KlineInterval` 参数指定周期。
    Kline(KlineInterval),

    /// 账户更新数据流 (Private)
    ///
    /// 包含余额变化、持仓变化和订单状态变化。
    /// 这是一个私有频道，通常需要鉴权。
    AccountUpdate, // Balance/Position/Order updates
}

/// 连接器请求协议 (Connector Protocol)
///
/// 这是 Feed 和 Gateway 与 Connector 交互的核心 DSL。
/// 它定义了所有可以向交易所发送的抽象指令。
///
/// Connector 的实现者负责将这些抽象指令翻译成具体的交易所 API 调用（REST 或 WebSocket 消息）。
///
/// 使用泛型 `I` (Instrument) 来确保类型安全的标的引用。
#[derive(Debug, Clone)]
pub enum ConnectorRequest<I, S: SubAccountIdentity> {
    // --- Feed Requests ---

    /// 订阅行情指令
    ///
    /// 请求 Connector 建立对特定标的、特定类型数据流的监听。
    /// 成功后，数据将通过 `ConnectorChannels` 广播。
    Subscribe {
        /// 订阅的目标标的
        instrument: I,
        /// 订阅的数据类型 (如 OrderBook, TradeFlow)
        channel_type: ChannelType,
    },

    /// 取消订阅指令
    ///
    /// 请求 Connector 停止监听特定数据流。
    Unsubscribe {
        instrument: I,
        channel_type: ChannelType,
    },

    // --- Gateway Requests (Execution) ---

    /// 提交订单指令
    ///
    /// 请求 Connector 向交易所发送一个新的订单。
    SubmitOrder {
        /// 子账户 ID (用于多账户管理)
        sub_account_id: S,
        /// 交易标的
        instrument: I,
        /// 订单详细参数 (价格、数量、类型等)
        req: OrderRequest,
    },

    /// 取消订单指令
    ///
    /// 请求 Connector 撤销一个未完全成交的订单。
    CancelOrder {
        sub_account_id: S,
        instrument: I,
        /// 要取消的订单 ID (交易所 ID)
        order_id: OrderId,
    },

    /// 修改订单指令
    ///
    /// 请求 Connector 修改一个现有订单的参数 (如价格或数量)。
    /// 注意：并非所有交易所都支持原子修改，部分实现可能需要先撤后发。
    AmendOrder {
        sub_account_id: S,
        instrument: I,
        order_id: OrderId,
        req: OrderRequest,
    },

    // --- Gateway Requests (Batch Execution) ---

    /// 批量提交订单指令
    ///
    /// 请求 Connector 一次性发送多个订单。
    BatchSubmitOrder {
        sub_account_id: S,
        instrument: I,
        reqs: Vec<OrderRequest>,
        /// 是否要求原子性 (All-or-Nothing)。
        /// 如果交易所不支持原子性，Connector 实现应尽可能模拟或忽略此标志并记录警告。
        atomic: bool,
    },

    /// 批量取消订单指令
    BatchCancelOrder {
        sub_account_id: S,
        instrument: I,
        order_ids: Vec<OrderId>,
        atomic: bool,
    },

    /// 批量修改订单指令
    BatchAmendOrder {
        sub_account_id: S,
        instrument: I,
        amends: Vec<(OrderId, OrderRequest)>,
        atomic: bool,
    },

    // --- Gateway Requests (Query) ---

    /// 查询当前挂单指令
    ///
    /// 请求 Connector 获取当前未结束的订单列表。
    /// 结果通常通过 `OrderUpdate` 通道异步返回，或作为 Future 的结果返回 (取决于具体实现模式)。
    FetchOpenOrders {
        sub_account_id: S,
        /// 可选的标的过滤。如果为 None，则查询该账户下所有标的的挂单。
        instrument: Option<I>,
    },

    /// 查询特定订单指令
    ///
    /// 请求 Connector 获取某个特定订单的最新状态。
    FetchOrder {
        sub_account_id: S,
        instrument: I,
        order_id: OrderId,
    },

    /// 查询持仓指令
    ///
    /// 请求 Connector 获取当前的持仓信息。
    FetchPositions {
        sub_account_id: S,
        instrument: Option<I>,
    },

    /// 查询余额指令
    ///
    /// 请求 Connector 获取账户的资产余额信息。
    FetchBalances { sub_account_id: S },
}

// --- Feed DSL ---

/// Feed 控制指令集
///
/// 策略层 (Strategy) 通过此指令集控制 Feed Actor 的行为。
/// Feed Actor 作为一个中间层，负责管理对 Connector 的订阅请求，并可能提供数据聚合、过滤或重放功能。
///
/// 使用泛型 `I` (Instrument) 来指定操作对象。
#[derive(Debug, Clone)]
pub enum FeedCommand<I> {
    /// 订阅指令
    ///
    /// 指示 Feed 开始提供特定标的的数据。
    /// Feed 收到此指令后，通常会向 Connector 发送 `ConnectorRequest::Subscribe`。
    Subscribe {
        instrument: I,
        channel_type: ChannelType,
    },

    /// 取消订阅指令
    Unsubscribe {
        instrument: I,
        channel_type: ChannelType,
    },

    /// 请求快照指令
    ///
    /// 请求 Feed 立即推送一次当前的全量数据快照（如当前的 OrderBook）。
    /// 这对于策略启动时的状态初始化非常有用。
    RequestSnapshot { instrument: I },

    /// 暂停推送指令
    ///
    /// 指示 Feed 暂时停止向策略层推送数据，但保持与 Connector 的连接。
    /// 用于节省带宽或在策略暂停时减少处理开销。
    Suspend { instrument: I },

    /// 恢复推送指令
    Resume { instrument: I },

    /// 设置聚合频率指令
    ///
    /// 指示 Feed 对高频数据进行聚合（如 Throttling 或 Conflation）。
    /// `interval_ms`: 聚合窗口大小（毫秒）。0 表示实时推送。
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

/// 订单控制指令集
///
/// 策略层或 Gateway 通过此指令集管理订单生命周期。
/// 涵盖了单笔订单和批量订单的创建、取消、修改以及查询操作。
///
/// 使用泛型 `I` (Instrument) 来指定操作对象。
#[derive(Debug, Clone)]
pub enum OrderCommand<I, S: SubAccountIdentity> {
    // --- Single Operations ---

    /// 创建订单指令
    ///
    /// 请求创建一个新的订单。
    Create {
        sub_account_id: S,
        instrument: I,
        req: OrderRequest,
    },

    /// 取消订单指令
    ///
    /// 请求取消一个指定的订单。
    Cancel {
        sub_account_id: S,
        instrument: I,
        id: OrderId,
    },

    /// 修改订单指令
    ///
    /// 请求修改一个现有订单的参数（如价格或数量）。
    Amend {
        sub_account_id: S,
        instrument: I,
        id: OrderId,
        req: OrderRequest,
    },

    // --- Batch Operations ---

    /// 批量创建订单指令
    ///
    /// 请求一次性创建多个订单。
    /// `atomic`: 如果为 true，且交易所支持，则这些订单要么全部成功，要么全部失败。
    BatchCreate {
        sub_account_id: S,
        instrument: I,
        reqs: Vec<OrderRequest>,
        /// If true, the batch is atomic (all or nothing), if supported by exchange.
        atomic: bool,
    },

    /// 批量取消订单指令
    BatchCancel {
        sub_account_id: S,
        instrument: I,
        ids: Vec<OrderId>,
        atomic: bool,
    },

    /// 批量修改订单指令
    BatchAmend {
        sub_account_id: S,
        instrument: I,
        amends: Vec<(OrderId, OrderRequest)>,
        atomic: bool,
    },

    // --- Query Operations ---

    /// 查询挂单指令
    ///
    /// 查询当前未完成的订单列表。
    /// `instrument`: 可选。如果提供，则只查询该标的的挂单；否则查询所有。
    GetOpenOrders {
        sub_account_id: S,
        instrument: Option<I>,
    },

    /// 查询特定订单指令
    GetOrder {
        sub_account_id: S,
        instrument: I,
        id: OrderId,
    },
}

/// 订单状态更新
///
/// 描述订单生命周期中的各种事件。
/// 这些事件由 Connector 推送给 Gateway 或 Strategy。
#[derive(Debug, Clone)]
pub enum OrderUpdate<I, A: AssetIdentity, S: SubAccountIdentity> {
    // --- Command Results (Ack/Nack) ---

    /// 请求已接受
    ///
    /// 交易所已收到并接受了订单请求（Create/Cancel/Amend）。
    /// 注意：这不代表订单已成交，只代表请求合法且已进入撮合引擎。
    RequestAccepted {
        sub_account_id: S,
        id: OrderId,
        client_id: Option<ClientOrderId>,
    },

    /// 请求被拒绝
    ///
    /// 交易所拒绝了订单请求。
    /// 可能原因：参数错误、余额不足、风控限制等。
    RequestRejected {
        sub_account_id: S,
        id: Option<OrderId>,
        client_id: Option<ClientOrderId>,
        reason: String,
    },

    // --- State Changes (Push from Exchange) ---

    /// 新订单确认
    ///
    /// 订单已成功挂在订单簿上。
    OrderNew(Order<A, S>),

    /// 订单成交
    ///
    /// 订单的部分或全部数量已成交。
    /// 包含成交价格、数量等详细信息。
    OrderFilled(Order<A, S>),

    /// 订单已取消
    ///
    /// 订单已被成功撤销。
    OrderCanceled {
        sub_account_id: S,
        id: OrderId,
        instrument: I,
    },

    /// 订单被拒绝 (被动)
    ///
    /// 订单在进入撮合引擎后被拒绝（例如 PostOnly 订单立即成交）。
    OrderRejected {
        sub_account_id: S,
        id: Option<OrderId>,
        reason: String,
    },

    /// 订单过期
    ///
    /// 订单因时间限制（如 FOK, IOC, GTC）而过期取消。
    OrderExpired {
        sub_account_id: S,
        id: OrderId,
        instrument: I,
    },

    // --- Query Results ---

    /// 订单快照
    ///
    /// 查询操作返回的当前挂单列表快照。
    OrderSnapshot(Vec<Order<A, S>>),

    // --- Error ---

    /// 通用错误
    Error(String),
}

// --- Position Domain ---

/// 持仓控制指令集
#[derive(Debug, Clone)]
pub enum PositionCommand<I, S: SubAccountIdentity> {
    /// 订阅持仓更新
    ///
    /// 请求 Connector 推送持仓变化数据。
    Subscribe {
        sub_account_id: S,
    },

    /// 查询持仓
    ///
    /// 请求立即返回当前的持仓状态。
    Query {
        sub_account_id: S,
        instrument: Option<I>,
    },
}

/// 持仓状态更新
#[derive(Debug, Clone)]
pub enum PositionUpdate<A: AssetIdentity, S: SubAccountIdentity> {
    /// 持仓快照
    ///
    /// 通常在订阅成功后或查询响应时返回全量持仓数据。
    Snapshot(Vec<Position<A, S>>),

    /// 持仓变更
    ///
    /// 单个持仓发生变化的增量更新。
    Update(Position<A, S>),

    /// 错误信息
    Error(String),
}

// --- Balance Domain ---

/// 资金/余额控制指令集
#[derive(Debug, Clone)]
pub enum BalanceCommand<S: SubAccountIdentity> {
    /// 订阅余额更新
    Subscribe { sub_account_id: S },

    /// 查询余额
    Query { sub_account_id: S },
}

/// 资金/余额状态更新
#[derive(Debug, Clone)]
pub enum BalanceUpdate<A: AssetIdentity, S: SubAccountIdentity> {
    /// 余额快照
    ///
    /// 返回账户下所有资产的余额快照。
    Snapshot(Vec<Balance<A, S>>),

    /// 余额变更
    ///
    /// 单个资产余额发生变化的增量更新。
    Update(Balance<A, S>),

    /// 错误信息
    Error(String),
}
