use crate::common::account_model::{
    Balance, ClientOrderId, Order, OrderId, OrderRequest, Position, SubAccountId,
};
use crate::common::actor_trait::Actor;
use thiserror::Error;

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

pub use crate::exchange::exchange_domain::{BalanceDomain, OrderDomain, PositionDomain};

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
    OrderNew(Order), // Order struct might need to be generic too? Let's check Order definition later.
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

/// 交易网关 (Gateway)
///
/// 负责账户模型的 CRUD 代理。
/// 它是交易所三元组中的“执行层”。
pub trait ExchangeGateway: Actor {
    type Id: Send + Sync + 'static + Clone + std::hash::Hash + Eq;
    /// 连接器提供的挂钩 (Hook)
    type ConnectorHook: Clone + Send + Sync + 'static;

    /// 使用连接器提供的挂钩创建 Gateway 实例
    fn new(hook: Self::ConnectorHook) -> Self;
}

// 这里的 AccessChannel 定义需要移到 Exchange 模板中或者作为 where 子句，
// 因为 trait 定义中包含泛型关联类型比较复杂。
// 但为了保持模板完整性，我们可以在这里定义约束，或者让 ExchangeGateway 继承 AccessChannel。
// 由于 OrderCommand<I> 依赖 Self::Id，我们需要像 ExchangeFeed 那样处理。
