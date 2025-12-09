use crate::common::account_model::{OrderId, OrderRequest, SubAccountId};
use crate::common::actor_trait::{AccessChannel, Actor, ChannelPair};
use crate::dataform::kline::KlineSeries;
use crate::dataform::orderbook::OrderBook;
use crate::dataform::tradeflow::TradeFlow;
use crate::exchange::exchange_domain::{
    BalanceUpdate, ChannelType, ConnectorRequest, NetworkCommand, NetworkControlDomain,
    NetworkStatus, NetworkStatusDomain, OrderUpdate, PositionUpdate,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::{Mutex, broadcast, mpsc};

// --- Internal Protocol (The "Connector Language") ---

// Moved to exchange_domain.rs

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
    /// 连接器的配置/实现类型，用于构建连接器实例。
    type Config: Send + Sync + 'static;

    /// 连接器暴露给内部组件 (Feed/Gateway) 的挂钩。
    /// 这是一个句柄，用于让组件挂载到连接器上。
    /// 具体的挂钩类型由实现者定义（例如包含特定 SDK 的 Sender）。
    ///
    /// 强烈建议 Hook 内部持有一个发送端，用于发送 `ConnectorRequest`。
    type Hook: Clone + Send + Sync + 'static;

    /// 使用配置构建连接器实例
    fn new(config: Self::Config) -> Self;

    /// 创建一个挂钩实例，供 Feed 和 Gateway 使用。
    fn create_hook(&self) -> Self::Hook;
}

// --- Standard Implementation Framework ---

/// 连接器实现 Trait
///
/// 开发者只需要实现这个 Trait，就可以自动获得一个完整的 ExchangeConnector。
/// 这个 Trait 专注于业务逻辑（如何连接、如何订阅、如何发单），
/// 而不需要关心 Actor 生命周期、通道管理、锁等基础设施。
#[async_trait]
pub trait ConnectorImpl: Send + Sync + 'static {
    type Instrument: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug;

    /// 初始化并启动连接器
    /// 在这里建立 WebSocket 连接，并保存 channels 以便后续推送数据。
    async fn start(&self, channels: ConnectorChannels<Self::Instrument>) -> anyhow::Result<()>;

    // --- Request Handlers ---

    async fn on_subscribe(
        &self,
        instrument: Self::Instrument,
        channel_type: ChannelType,
    ) -> anyhow::Result<()>;
    async fn on_unsubscribe(
        &self,
        instrument: Self::Instrument,
        channel_type: ChannelType,
    ) -> anyhow::Result<()>;

    async fn on_submit_order(
        &self,
        sub_account_id: SubAccountId,
        instrument: Self::Instrument,
        req: OrderRequest,
    ) -> anyhow::Result<()>;
    async fn on_cancel_order(
        &self,
        sub_account_id: SubAccountId,
        instrument: Self::Instrument,
        order_id: OrderId,
    ) -> anyhow::Result<()>;
    async fn on_amend_order(
        &self,
        sub_account_id: SubAccountId,
        instrument: Self::Instrument,
        order_id: OrderId,
        req: OrderRequest,
    ) -> anyhow::Result<()>;

    // Default implementations for optional features
    async fn on_batch_submit_order(
        &self,
        _sub_account_id: SubAccountId,
        _instrument: Self::Instrument,
        _reqs: Vec<OrderRequest>,
        _atomic: bool,
    ) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("Batch submit not supported"))
    }
    async fn on_batch_cancel_order(
        &self,
        _sub_account_id: SubAccountId,
        _instrument: Self::Instrument,
        _order_ids: Vec<OrderId>,
        _atomic: bool,
    ) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("Batch cancel not supported"))
    }
    async fn on_batch_amend_order(
        &self,
        _sub_account_id: SubAccountId,
        _instrument: Self::Instrument,
        _amends: Vec<(OrderId, OrderRequest)>,
        _atomic: bool,
    ) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("Batch amend not supported"))
    }
}

/// 连接器数据通道集合
///
/// 这是一个包含所有数据推送通道的容器。
/// `ConnectorImpl` 在 `start` 时会收到这个结构体。
/// 当收到 WebSocket 消息时，通过这些通道将数据推送到系统内部。
#[derive(Clone)]
pub struct ConnectorChannels<I> {
    pub orderbook_txs: Arc<RwLock<HashMap<I, broadcast::Sender<OrderBook>>>>,
    pub trade_txs: Arc<RwLock<HashMap<I, broadcast::Sender<TradeFlow>>>>,
    pub kline_txs: Arc<RwLock<HashMap<I, broadcast::Sender<KlineSeries>>>>,

    // Execution Updates
    pub order_update_tx: broadcast::Sender<OrderUpdate<I>>,
    pub position_update_tx: broadcast::Sender<PositionUpdate>,
    pub balance_update_tx: broadcast::Sender<BalanceUpdate>,

    // Network Status
    pub status_tx: broadcast::Sender<NetworkStatus>,
}

impl<I> ConnectorChannels<I>
where
    I: std::hash::Hash + Eq + Clone,
{
    /// 获取或创建 OrderBook 通道
    pub fn get_orderbook_tx(&self, instrument: &I) -> broadcast::Sender<OrderBook> {
        let mut map = self.orderbook_txs.write().unwrap();
        map.entry(instrument.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .clone()
    }

    /// 获取或创建 TradeFlow 通道
    pub fn get_trade_tx(&self, instrument: &I) -> broadcast::Sender<TradeFlow> {
        let mut map = self.trade_txs.write().unwrap();
        map.entry(instrument.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .clone()
    }

    /// 获取或创建 KlineSeries 通道
    pub fn get_kline_tx(&self, instrument: &I) -> broadcast::Sender<KlineSeries> {
        let mut map = self.kline_txs.write().unwrap();
        map.entry(instrument.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .clone()
    }
}

/// 标准连接器挂钩
///
/// 这是 `StandardConnector` 暴露给 Feed 和 Gateway 的句柄。
#[derive(Clone)]
pub struct StandardHook<I> {
    pub request_tx: mpsc::Sender<ConnectorRequest<I>>,
    pub channels: ConnectorChannels<I>,
}

/// 标准连接器模板
///
/// 这是一个泛型结构体，实现了 `ExchangeConnector`。
/// 它封装了 Actor 循环、通道管理和请求分发。
/// 开发者只需要提供 `Impl` (业务逻辑实现)。
pub struct StandardConnector<I, Impl> {
    implementation: Arc<Impl>,

    // Channels
    network_tx: mpsc::Sender<NetworkCommand>,
    network_rx: Arc<Mutex<Option<mpsc::Receiver<NetworkCommand>>>>,

    request_rx: Arc<Mutex<Option<mpsc::Receiver<ConnectorRequest<I>>>>>,

    hook: StandardHook<I>,
}

impl<I, Impl> StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    pub fn new(implementation: Impl) -> Self {
        let (network_tx, network_rx) = mpsc::channel(100);
        let (request_tx, request_rx) = mpsc::channel(100);
        let (status_tx, _) = broadcast::channel(100);
        let (order_update_tx, _) = broadcast::channel(100);
        let (position_update_tx, _) = broadcast::channel(100);
        let (balance_update_tx, _) = broadcast::channel(100);

        let channels = ConnectorChannels {
            orderbook_txs: Arc::new(RwLock::new(HashMap::new())),
            trade_txs: Arc::new(RwLock::new(HashMap::new())),
            kline_txs: Arc::new(RwLock::new(HashMap::new())),
            order_update_tx,
            position_update_tx,
            balance_update_tx,
            status_tx,
        };

        let hook = StandardHook {
            request_tx: request_tx.clone(),
            channels: channels.clone(),
        };

        Self {
            implementation: Arc::new(implementation),
            network_tx,
            network_rx: Arc::new(Mutex::new(Some(network_rx))),
            request_rx: Arc::new(Mutex::new(Some(request_rx))),
            hook,
        }
    }
}

#[async_trait]
impl<I, Impl> Actor for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    async fn start(&self) -> anyhow::Result<()> {
        let mut request_rx = self
            .request_rx
            .lock()
            .await
            .take()
            .ok_or(anyhow::anyhow!("Request RX already taken"))?;
        let mut network_rx = self
            .network_rx
            .lock()
            .await
            .take()
            .ok_or(anyhow::anyhow!("Network RX already taken"))?;

        let implementation = self.implementation.clone();
        let channels = self.hook.channels.clone();

        // Start the implementation (e.g., connect WS)
        implementation.start(channels).await?;

        // Start network control loop
        tokio::spawn(async move {
            while let Some(cmd) = network_rx.recv().await {
                match cmd {
                    NetworkCommand::Reconnect => {
                        // TODO: Implement reconnect
                    }
                    NetworkCommand::Disconnect => {
                        // TODO: Implement disconnect
                    }
                    NetworkCommand::CircuitBreak => {
                        // TODO: Implement circuit break
                    }
                }
            }
        });

        // Start the request loop
        tokio::spawn(async move {
            while let Some(req) = request_rx.recv().await {
                let res = match req {
                    ConnectorRequest::Subscribe {
                        instrument,
                        channel_type,
                    } => implementation.on_subscribe(instrument, channel_type).await,
                    ConnectorRequest::Unsubscribe {
                        instrument,
                        channel_type,
                    } => {
                        implementation
                            .on_unsubscribe(instrument, channel_type)
                            .await
                    }
                    ConnectorRequest::SubmitOrder {
                        sub_account_id,
                        instrument,
                        req,
                    } => {
                        implementation
                            .on_submit_order(sub_account_id, instrument, req)
                            .await
                    }
                    ConnectorRequest::CancelOrder {
                        sub_account_id,
                        instrument,
                        order_id,
                    } => {
                        implementation
                            .on_cancel_order(sub_account_id, instrument, order_id)
                            .await
                    }
                    ConnectorRequest::AmendOrder {
                        sub_account_id,
                        instrument,
                        order_id,
                        req,
                    } => {
                        implementation
                            .on_amend_order(sub_account_id, instrument, order_id, req)
                            .await
                    }
                    // ... handle other requests
                    _ => Ok(()), // TODO: Implement others
                };

                if let Err(e) = res {
                    eprintln!("Connector request failed: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        // TODO: Notify implementation to stop
        Ok(())
    }
}

impl<I, Impl> AccessChannel<NetworkControlDomain> for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    type Id = ();
    type Sender = mpsc::Sender<NetworkCommand>;
    type Receiver = ();

    fn access_channel(
        &self,
        _domain: NetworkControlDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: Some(self.network_tx.clone()),
            rx: None,
        })
    }
}

impl<I, Impl> AccessChannel<NetworkStatusDomain> for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    type Id = ();
    type Sender = ();
    type Receiver = broadcast::Receiver<NetworkStatus>;

    fn access_channel(
        &self,
        _domain: NetworkStatusDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: None,
            rx: Some(self.hook.channels.status_tx.subscribe()),
        })
    }
}

impl<I, Impl> ExchangeConnector for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    type Config = Impl;
    type Hook = StandardHook<I>;

    fn new(config: Self::Config) -> Self {
        Self::new(config)
    }

    fn create_hook(&self) -> Self::Hook {
        self.hook.clone()
    }
}
