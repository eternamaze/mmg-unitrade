use crate::common::account_model::{AssetIdentity, SubAccountIdentity};
use crate::common::actor_trait::{AccessChannel, Actor, ChannelPair};
use crate::exchange::exchange_domain::{
    BalanceCommand, BalanceDomain, BalanceUpdate, ConnectorRequest,
    OrderCommand, OrderDomain, OrderUpdate, PositionCommand, PositionDomain, PositionUpdate,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};

use crate::exchange::exchange_connector_template::ConnectorChannels;

// --- Order Domain ---

// Moved to exchange_domain.rs

/// [API] 交易网关 (Gateway)
///
/// **角色：框架标准接口 (Framework Standard Interface)**
/// **使用者：框架用户 (Framework User)**
///
/// 负责账户模型的 CRUD 代理。
/// 它是交易所三元组中的“执行层”。
pub trait ExchangeGateway<A: AssetIdentity, S: SubAccountIdentity>: Actor {
    type Id: Send + Sync + 'static + Clone + std::hash::Hash + Eq;

    /// 使用连接器信道创建 Gateway 实例
    fn new(
        connector_tx: mpsc::Sender<ConnectorRequest<Self::Id, S>>,
        connector_channels: ConnectorChannels<Self::Id, A, S>,
    ) -> Self;
}

// --- Standard Implementation ---

/// 标准交易网关模板
///
/// 这是一个泛型结构体，实现了 `ExchangeGateway`。
/// 它自动处理订单路由和账户状态查询。
pub struct StandardGateway<I, A, S>
where
    S: SubAccountIdentity,
    A: AssetIdentity,
{
    connector_tx: mpsc::Sender<ConnectorRequest<I, S>>,
    connector_channels: ConnectorChannels<I, A, S>,

    order_tx: mpsc::Sender<OrderCommand<I, S>>,
    order_rx: Arc<Mutex<Option<mpsc::Receiver<OrderCommand<I, S>>>>>,

    position_tx: mpsc::Sender<PositionCommand<I, S>>,
    position_rx: Arc<Mutex<Option<mpsc::Receiver<PositionCommand<I, S>>>>>,

    balance_tx: mpsc::Sender<BalanceCommand<S>>,
    balance_rx: Arc<Mutex<Option<mpsc::Receiver<BalanceCommand<S>>>>>,

    // User-facing channels (owned by Gateway)
    order_update_tx: broadcast::Sender<OrderUpdate<I, A, S>>,
    position_update_tx: broadcast::Sender<PositionUpdate<A, S>>,
    balance_update_tx: broadcast::Sender<BalanceUpdate<A, S>>,
}

impl<I, A, S> StandardGateway<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    pub fn new(
        connector_tx: mpsc::Sender<ConnectorRequest<I, S>>,
        connector_channels: ConnectorChannels<I, A, S>,
    ) -> Self {
        let (order_tx, order_rx) = mpsc::channel(100);
        let (position_tx, position_rx) = mpsc::channel(100);
        let (balance_tx, balance_rx) = mpsc::channel(100);
        
        let (order_update_tx, _) = broadcast::channel(100);
        let (position_update_tx, _) = broadcast::channel(100);
        let (balance_update_tx, _) = broadcast::channel(100);

        Self {
            connector_tx,
            connector_channels,
            order_tx,
            order_rx: Arc::new(Mutex::new(Some(order_rx))),
            position_tx,
            position_rx: Arc::new(Mutex::new(Some(position_rx))),
            balance_tx,
            balance_rx: Arc::new(Mutex::new(Some(balance_rx))),
            order_update_tx,
            position_update_tx,
            balance_update_tx,
        }
    }
}

impl<I, A, S> ExchangeGateway<A, S> for StandardGateway<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = I;

    fn new(
        connector_tx: mpsc::Sender<ConnectorRequest<Self::Id, S>>,
        connector_channels: ConnectorChannels<Self::Id, A, S>,
    ) -> Self {
        Self::new(connector_tx, connector_channels)
    }
}

#[async_trait]
impl<I, A, S> Actor for StandardGateway<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
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
        let connector_channels = self.connector_channels.clone();
        
        let user_order_update_tx = self.order_update_tx.clone();
        let user_position_update_tx = self.position_update_tx.clone();
        let user_balance_update_tx = self.balance_update_tx.clone();

        // Forwarding Tasks
        // 1. Order Updates
        {
            let mut rx = connector_channels.order_update_tx.subscribe();
            let tx = user_order_update_tx.clone();
            tokio::spawn(async move {
                while let Ok(update) = rx.recv().await {
                    let _ = tx.send(update);
                }
            });
        }
        // 2. Position Updates
        {
            let mut rx = connector_channels.position_update_tx.subscribe();
            let tx = user_position_update_tx.clone();
            tokio::spawn(async move {
                while let Ok(update) = rx.recv().await {
                    let _ = tx.send(update);
                }
            });
        }
        // 3. Balance Updates
        {
            let mut rx = connector_channels.balance_update_tx.subscribe();
            let tx = user_balance_update_tx.clone();
            tokio::spawn(async move {
                while let Ok(update) = rx.recv().await {
                    let _ = tx.send(update);
                }
            });
        }

        // Order Loop
        let request_tx_clone = request_tx.clone();
        tokio::spawn(async move {
            while let Some(cmd) = order_rx.recv().await {
                match cmd {
                    OrderCommand::Place {
                        sub_account_id,
                        instrument,
                        req,
                    } => {
                        let req = ConnectorRequest::TransmitOrder {
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
                        let req = ConnectorRequest::TransmitCancel {
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
                        let req = ConnectorRequest::TransmitAmend {
                            sub_account_id,
                            instrument,
                            order_id: id,
                            req,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    OrderCommand::BatchPlace {
                        sub_account_id,
                        instrument,
                        reqs,
                        atomic,
                    } => {
                        let req = ConnectorRequest::TransmitBatchOrder {
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
                        let req = ConnectorRequest::TransmitBatchCancel {
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
                        let req = ConnectorRequest::TransmitBatchAmend {
                            sub_account_id,
                            instrument,
                            amends,
                            atomic,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    OrderCommand::QueryOpenOrders {
                        sub_account_id,
                        instrument,
                    } => {
                        let req = ConnectorRequest::ExecuteFetchOpenOrders {
                            sub_account_id,
                            instrument,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    OrderCommand::QueryOrder {
                        sub_account_id,
                        instrument,
                        id,
                    } => {
                        let req = ConnectorRequest::ExecuteFetchOrder {
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
                        let req = ConnectorRequest::ExecuteFetchPositions {
                            sub_account_id,
                            instrument,
                        };
                        let _ = request_tx_clone.send(req).await;
                    }
                    PositionCommand::Monitor { .. } => {
                        // TODO: Handle position monitoring if needed
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
                        let req = ConnectorRequest::ExecuteFetchBalances { sub_account_id };
                        let _ = request_tx_clone.send(req).await;
                    }
                    BalanceCommand::Monitor { .. } => {
                        // TODO: Handle balance monitoring if needed
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

impl<I, A, S> AccessChannel<OrderDomain> for StandardGateway<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = S;
    type Sender = mpsc::Sender<OrderCommand<I, S>>;
    type Receiver = broadcast::Receiver<OrderUpdate<I, A, S>>;

    fn access_channel(
        &self,
        _domain: OrderDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: Some(self.order_tx.clone()),
            rx: Some(self.order_update_tx.subscribe()),
        })
    }
}

impl<I, A, S> AccessChannel<PositionDomain> for StandardGateway<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = S;
    type Sender = mpsc::Sender<PositionCommand<I, S>>;
    type Receiver = broadcast::Receiver<PositionUpdate<A, S>>;

    fn access_channel(
        &self,
        _domain: PositionDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: Some(self.position_tx.clone()),
            rx: Some(self.position_update_tx.subscribe()),
        })
    }
}

impl<I, A, S> AccessChannel<BalanceDomain> for StandardGateway<I, A, S>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = S;
    type Sender = mpsc::Sender<BalanceCommand<S>>;
    type Receiver = broadcast::Receiver<BalanceUpdate<A, S>>;

    fn access_channel(
        &self,
        _domain: BalanceDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: Some(self.balance_tx.clone()),
            rx: Some(self.balance_update_tx.subscribe()),
        })
    }
}
