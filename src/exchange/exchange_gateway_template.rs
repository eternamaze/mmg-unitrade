use crate::common::account_model::{SubAccountId};
use crate::common::actor_trait::{AccessChannel, Actor, ChannelPair};
use crate::exchange::exchange_domain::{
    BalanceCommand, BalanceDomain, BalanceUpdate, ConnectorRequest,
    OrderCommand, OrderDomain, OrderUpdate, PositionCommand, PositionDomain, PositionUpdate,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};

// --- Order Domain ---

// Moved to exchange_domain.rs

/// 交易网关 (Gateway)
///
/// 负责账户模型的 CRUD 代理。
/// 它是交易所三元组中的“执行层”。
pub trait ExchangeGateway: Actor {
    type Id: Send + Sync + 'static + Clone + std::hash::Hash + Eq;

    /// 使用连接器信道创建 Gateway 实例
    fn new(connector_tx: mpsc::Sender<ConnectorRequest<Self::Id>>) -> Self;
}

// --- Standard Implementation ---

/// 标准交易网关模板
///
/// 这是一个泛型结构体，实现了 `ExchangeGateway`。
/// 它自动处理订单路由和账户状态查询。
pub struct StandardGateway<I> {
    connector_tx: mpsc::Sender<ConnectorRequest<I>>,

    order_tx: mpsc::Sender<OrderCommand<I>>,
    order_rx: Arc<Mutex<Option<mpsc::Receiver<OrderCommand<I>>>>>,

    position_tx: mpsc::Sender<PositionCommand<I>>,
    position_rx: Arc<Mutex<Option<mpsc::Receiver<PositionCommand<I>>>>>,

    balance_tx: mpsc::Sender<BalanceCommand>,
    balance_rx: Arc<Mutex<Option<mpsc::Receiver<BalanceCommand>>>>,
}

impl<I> StandardGateway<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    pub fn new(connector_tx: mpsc::Sender<ConnectorRequest<I>>) -> Self {
        let (order_tx, order_rx) = mpsc::channel(100);
        let (position_tx, position_rx) = mpsc::channel(100);
        let (balance_tx, balance_rx) = mpsc::channel(100);

        Self {
            connector_tx,
            order_tx,
            order_rx: Arc::new(Mutex::new(Some(order_rx))),
            position_tx,
            position_rx: Arc::new(Mutex::new(Some(position_rx))),
            balance_tx,
            balance_rx: Arc::new(Mutex::new(Some(balance_rx))),
        }
    }
}

impl<I> ExchangeGateway for StandardGateway<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    type Id = I;

    fn new(connector_tx: mpsc::Sender<ConnectorRequest<Self::Id>>) -> Self {
        Self::new(connector_tx)
    }
}

#[async_trait]
impl<I> Actor for StandardGateway<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    async fn start(&self) -> anyhow::Result<()> {
        let mut order_rx = self
            .order_rx
            .lock()
            .await
            .take()
            .ok_or(anyhow::anyhow!("Order RX already taken"))?;
        let mut position_rx = self
            .position_rx
            .lock()
            .await
            .take()
            .ok_or(anyhow::anyhow!("Position RX already taken"))?;
        let mut balance_rx = self
            .balance_rx
            .lock()
            .await
            .take()
            .ok_or(anyhow::anyhow!("Balance RX already taken"))?;

        let request_tx = self.connector_tx.clone();

        // Order Loop
        let request_tx_clone = request_tx.clone();
        tokio::spawn(async move {
            while let Some(cmd) = order_rx.recv().await {
                match cmd {
                    OrderCommand::Create {
                        sub_account_id,
                        instrument,
                        req,
                    } => {
                        let req = ConnectorRequest::SubmitOrder {
                            sub_account_id,
                            instrument,
                            req,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    OrderCommand::Cancel {
                        sub_account_id,
                        instrument,
                        id,
                    } => {
                        let req = ConnectorRequest::CancelOrder {
                            sub_account_id,
                            instrument,
                            order_id: id,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    OrderCommand::Amend {
                        sub_account_id,
                        instrument,
                        id,
                        req,
                    } => {
                        let req = ConnectorRequest::AmendOrder {
                            sub_account_id,
                            instrument,
                            order_id: id,
                            req,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    OrderCommand::BatchCreate {
                        sub_account_id,
                        instrument,
                        reqs,
                        atomic,
                    } => {
                        let req = ConnectorRequest::BatchSubmitOrder {
                            sub_account_id,
                            instrument,
                            reqs,
                            atomic,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    OrderCommand::BatchCancel {
                        sub_account_id,
                        instrument,
                        ids,
                        atomic,
                    } => {
                        let req = ConnectorRequest::BatchCancelOrder {
                            sub_account_id,
                            instrument,
                            order_ids: ids,
                            atomic,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    OrderCommand::BatchAmend {
                        sub_account_id,
                        instrument,
                        amends,
                        atomic,
                    } => {
                        let req = ConnectorRequest::BatchAmendOrder {
                            sub_account_id,
                            instrument,
                            amends,
                            atomic,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    OrderCommand::GetOpenOrders {
                        sub_account_id,
                        instrument,
                    } => {
                        let req = ConnectorRequest::FetchOpenOrders {
                            sub_account_id,
                            instrument,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    OrderCommand::GetOrder {
                        sub_account_id,
                        instrument,
                        id,
                    } => {
                        let req = ConnectorRequest::FetchOrder {
                            sub_account_id,
                            instrument,
                            order_id: id,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                }
            }
        });

        // Position Loop
        let request_tx_clone = request_tx.clone();
        tokio::spawn(async move {
            while let Some(cmd) = position_rx.recv().await {
                match cmd {
                    PositionCommand::Query {
                        sub_account_id,
                        instrument,
                    } => {
                        let req = ConnectorRequest::FetchPositions {
                            sub_account_id,
                            instrument,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    PositionCommand::Subscribe { .. } => {
                        // TODO: Handle position subscription if needed
                    }
                }
            }
        });

        // Balance Loop
        let request_tx_clone = request_tx.clone();
        tokio::spawn(async move {
            while let Some(cmd) = balance_rx.recv().await {
                match cmd {
                    BalanceCommand::Query { sub_account_id } => {
                        let req = ConnectorRequest::FetchBalances { sub_account_id };
                        let _ = request_tx_clone.send(req).await;
                    }
                    BalanceCommand::Subscribe { .. } => {
                        // TODO: Handle balance subscription if needed
                    }
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

// --- Access Channels ---

impl<I> AccessChannel<OrderDomain> for StandardGateway<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    type Id = SubAccountId;
    type Sender = mpsc::Sender<OrderCommand<I>>;
    type Receiver = broadcast::Receiver<OrderUpdate<I>>;

    fn access_channel(
        &self,
        _domain: OrderDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: Some(self.order_tx.clone()),
            rx: None,
        })
    }
}

impl<I> AccessChannel<PositionDomain> for StandardGateway<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    type Id = SubAccountId;
    type Sender = mpsc::Sender<PositionCommand<I>>;
    type Receiver = broadcast::Receiver<PositionUpdate>;

    fn access_channel(
        &self,
        _domain: PositionDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: Some(self.position_tx.clone()),
            rx: None,
        })
    }
}

impl<I> AccessChannel<BalanceDomain> for StandardGateway<I>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
{
    type Id = SubAccountId;
    type Sender = mpsc::Sender<BalanceCommand>;
    type Receiver = broadcast::Receiver<BalanceUpdate>;

    fn access_channel(
        &self,
        _domain: BalanceDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: Some(self.balance_tx.clone()),
            rx: None,
        })
    }
}
