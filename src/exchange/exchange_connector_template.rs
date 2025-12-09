use crate::common::account_model::{
    KlineInterval, OrderId, OrderRequest, SubAccountId,
};
use crate::common::actor_trait::{AccessChannel, Actor};
use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc};

pub use crate::exchange::exchange_domain::{NetworkControlDomain, NetworkStatusDomain};

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

// --- Internal Protocol (The "Connector Language") ---

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

/// 交易所连接器 (Connector)
///
/// 负责维护与交易所的网络连接（WebSocket/REST）。
/// 它不处理具体的业务逻辑，只负责数据的收发和连接的维护。
/// 它是交易所三元组中的“网络层”。
///
/// Connector 必须是一个 Actor，并且暴露一个“挂钩” (Hook) 供 Feed 和 Gateway 使用。
/// 同时，它必须实现标准的网络控制和状态协议，允许系统监控和干预网络层。
#[async_trait]
pub trait ExchangeConnector:
    Actor
    + AccessChannel<NetworkControlDomain, Sender = mpsc::Sender<NetworkCommand>, Receiver = ()>
    + AccessChannel<NetworkStatusDomain, Sender = (), Receiver = broadcast::Receiver<NetworkStatus>>
{
    /// 连接器暴露给内部组件 (Feed/Gateway) 的挂钩。
    /// 这是一个句柄，用于让组件挂载到连接器上。
    /// 具体的挂钩类型由实现者定义（例如包含特定 SDK 的 Sender）。
    ///
    /// 强烈建议 Hook 内部持有一个发送端，用于发送 `ConnectorRequest`。
    type Hook: Clone + Send + Sync + 'static;

    /// 创建一个挂钩实例，供 Feed 和 Gateway 使用。
    fn create_hook(&self) -> Self::Hook;
}
