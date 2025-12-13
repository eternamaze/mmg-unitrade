use crate::common::account_model::{OrderId, OrderRequest, SubAccountIdentity, AssetIdentity};
use crate::common::actor_trait::{AccessChannel, Actor, ChannelPair};
use crate::dataform::kline::KlineSeries;
use crate::dataform::orderbook::OrderBook;
use crate::dataform::tradeflow::TradeFlow;
use crate::exchange::exchange_domain::{
    BalanceDomain, BalanceUpdate, ChannelType, ConnectorControlDomain, ConnectorRequest,
    KlineDomain, NetworkCommand, NetworkControlDomain, NetworkStatus, NetworkStatusDomain,
    OrderBookDomain, OrderDomain, OrderUpdate, PositionDomain, PositionUpdate, TradeFlowDomain,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::{Mutex, broadcast, mpsc};

// --- Internal Protocol (The "Connector Language") ---

// Moved to exchange_domain.rs

/// [API] 交易所连接器 (Connector)
///
/// **角色：框架标准接口 (Framework Standard Interface)**
/// **使用者：框架用户 (Framework User)**
///
/// 负责维护与交易所的网络连接（WebSocket/REST）。
/// 它不处理具体的业务逻辑，只负责数据的收发和连接的维护。
/// 它是交易所三元组中的“网络层”。
///
/// Connector 必须是一个 Actor。
/// 同时，它必须实现标准的网络控制和状态协议，允许系统监控和干预网络层。
#[async_trait]
pub trait ExchangeConnector<I, A: AssetIdentity, S: SubAccountIdentity>:
    Actor
    + AccessChannel<NetworkControlDomain, Sender = mpsc::Sender<NetworkCommand>, Receiver = ()>
    + AccessChannel<NetworkStatusDomain, Sender = (), Receiver = broadcast::Receiver<NetworkStatus>>
{
    /// 连接器的配置/实现类型，用于构建连接器实例。
    type Config: Send + Sync + 'static;

    /// 使用配置构建连接器实例
    fn new(config: Self::Config) -> Self;

    /// 获取连接器信道集合
    fn channels(&self) -> ConnectorChannels<I, A, S>;
}

// --- Standard Implementation Framework ---

/// [SPI] 连接器实现 Trait (Service Provider Interface)
///
/// **角色：服务提供者接口 (Service Provider Interface)**
/// **使用者：交易所开发者 (Exchange Developer)**
///
/// 开发者只需要实现这个 Trait，就可以自动获得一个完整的 ExchangeConnector。
/// 这个 Trait 专注于业务逻辑（如何连接、如何订阅、如何发单），
/// 而不需要关心 Actor 生命周期、通道管理、锁等基础设施。
#[async_trait]
pub trait ConnectorImpl: Send + Sync + 'static {
    type Asset: AssetIdentity;
    type SubAccount: SubAccountIdentity;
    type Instrument: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug;

    /// 初始化并启动连接器
    /// 在这里建立 WebSocket 连接，并保存 channels 以便后续推送数据。
    async fn start(&self, channels: ConnectorChannels<Self::Instrument, Self::Asset, Self::SubAccount>) -> anyhow::Result<()>;

    /// 钩子：执行重连逻辑
    /// 框架在收到 Reconnect 指令时会调用此方法。
    /// 实现者应在此处断开旧连接、建立新连接并恢复订阅。
    async fn on_reconnect(&self) -> anyhow::Result<()>;

    /// 钩子：执行断开连接逻辑
    /// 框架在收到 Disconnect 指令时会调用此方法。
    async fn on_disconnect(&self) -> anyhow::Result<()>;

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
        sub_account_id: Self::SubAccount,
        instrument: Self::Instrument,
        req: OrderRequest,
    ) -> anyhow::Result<()>;
    async fn on_cancel_order(
        &self,
        sub_account_id: Self::SubAccount,
        instrument: Self::Instrument,
        order_id: OrderId,
    ) -> anyhow::Result<()>;
    async fn on_amend_order(
        &self,
        sub_account_id: Self::SubAccount,
        instrument: Self::Instrument,
        order_id: OrderId,
        req: OrderRequest,
    ) -> anyhow::Result<()>;

    async fn on_fetch_open_orders(
        &self,
        sub_account_id: Self::SubAccount,
        instrument: Option<Self::Instrument>,
    ) -> anyhow::Result<()>;

    async fn on_fetch_order(
        &self,
        sub_account_id: Self::SubAccount,
        instrument: Self::Instrument,
        order_id: OrderId,
    ) -> anyhow::Result<()>;

    async fn on_fetch_positions(
        &self,
        sub_account_id: Self::SubAccount,
        instrument: Option<Self::Instrument>,
    ) -> anyhow::Result<()>;

    async fn on_fetch_balances(&self, sub_account_id: Self::SubAccount) -> anyhow::Result<()>;

    // Default implementations for optional features
    async fn on_batch_submit_order(
        &self,
        _sub_account_id: Self::SubAccount,
        _instrument: Self::Instrument,
        _reqs: Vec<OrderRequest>,
        _atomic: bool,
    ) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("Batch submit not supported"))
    }
    async fn on_batch_cancel_order(
        &self,
        _sub_account_id: Self::SubAccount,
        _instrument: Self::Instrument,
        _order_ids: Vec<OrderId>,
        _atomic: bool,
    ) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("Batch cancel not supported"))
    }
    async fn on_batch_amend_order(
        &self,
        _sub_account_id: Self::SubAccount,
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
pub struct ConnectorChannels<I, A: AssetIdentity, S: SubAccountIdentity> {
    pub orderbook_txs: Arc<RwLock<HashMap<I, broadcast::Sender<OrderBook<A>>>>>,
    pub trade_txs: Arc<RwLock<HashMap<I, broadcast::Sender<TradeFlow<A>>>>>,
    pub kline_txs: Arc<RwLock<HashMap<I, broadcast::Sender<KlineSeries<A>>>>>,

    // Execution Updates
    pub order_update_tx: broadcast::Sender<OrderUpdate<I, A, S>>,
    pub position_update_tx: broadcast::Sender<PositionUpdate<A, S>>,
    pub balance_update_tx: broadcast::Sender<BalanceUpdate<A, S>>,

    // Network Status
    pub status_tx: broadcast::Sender<NetworkStatus>,
}

impl<I, A, S> ConnectorChannels<I, A, S>
where
    I: std::hash::Hash + Eq + Clone,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    /// 获取或创建 OrderBook 通道
    pub fn get_orderbook_tx(&self, instrument: &I) -> broadcast::Sender<OrderBook<A>> {
        let mut map = self.orderbook_txs.write().unwrap();
        map.entry(instrument.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .clone()
    }

    /// 获取或创建 TradeFlow 通道
    pub fn get_trade_tx(&self, instrument: &I) -> broadcast::Sender<TradeFlow<A>> {
        let mut map = self.trade_txs.write().unwrap();
        map.entry(instrument.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .clone()
    }

    /// 获取或创建 KlineSeries 通道
    pub fn get_kline_tx(&self, instrument: &I) -> broadcast::Sender<KlineSeries<A>> {
        let mut map = self.kline_txs.write().unwrap();
        map.entry(instrument.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .clone()
    }
}

/// 标准连接器模板
///
/// 这是一个泛型结构体，实现了 `ExchangeConnector`。
/// 它封装了 Actor 循环、通道管理和请求分发。
/// 开发者只需要提供 `Impl` (业务逻辑实现)。
pub struct StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    implementation: Arc<Impl>,

    // Channels
    network_tx: mpsc::Sender<NetworkCommand>,
    network_rx: Arc<Mutex<Option<mpsc::Receiver<NetworkCommand>>>>,

    request_tx: mpsc::Sender<ConnectorRequest<I, Impl::SubAccount>>,
    request_rx: Arc<Mutex<Option<mpsc::Receiver<ConnectorRequest<I, Impl::SubAccount>>>>>,

    channels: ConnectorChannels<I, Impl::Asset, Impl::SubAccount>,
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

        Self {
            implementation: Arc::new(implementation),
            network_tx,
            network_rx: Arc::new(Mutex::new(Some(network_rx))),
            request_tx,
            request_rx: Arc::new(Mutex::new(Some(request_rx))),
            channels,
        }
    }

    pub fn channels(&self) -> ConnectorChannels<I, Impl::Asset, Impl::SubAccount> {
        self.channels.clone()
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
        let channels = self.channels.clone();

        // Start the implementation (e.g., connect WS)
        implementation.start(channels).await?;

        let implementation_for_network = implementation.clone();

        // Start network control loop
        tokio::spawn(async move {
            let implementation = implementation_for_network;
            while let Some(cmd) = network_rx.recv().await {
                match cmd {
                    NetworkCommand::Reconnect => {
                        if let Err(e) = implementation.on_reconnect().await {
                            eprintln!("Reconnect failed: {}", e);
                        }
                    }
                    NetworkCommand::Disconnect => {
                        if let Err(e) = implementation.on_disconnect().await {
                            eprintln!("Disconnect failed: {}", e);
                        }
                    }
                    NetworkCommand::CircuitBreak => {
                        eprintln!("Circuit Breaker triggered! Disconnecting...");
                        if let Err(e) = implementation.on_disconnect().await {
                            eprintln!("Circuit break disconnect failed: {}", e);
                        }
                        // TODO: Consider adding a state flag to reject future requests
                    }
                }
            }
        });

        // Start the request loop
        tokio::spawn(async move {
            while let Some(req) = request_rx.recv().await {
                let res = match req {
                    ConnectorRequest::EstablishDataStream {
                        instrument,
                        channel_type,
                    } => implementation.on_subscribe(instrument, channel_type).await,
                    ConnectorRequest::StopDataStream {
                        instrument,
                        channel_type,
                    } => {
                        implementation
                            .on_unsubscribe(instrument, channel_type)
                            .await
                    }
                    ConnectorRequest::TransmitOrder {
                        sub_account_id,
                        instrument,
                        req,
                    } => {
                        implementation
                            .on_submit_order(sub_account_id, instrument, req)
                            .await
                    }
                    ConnectorRequest::TransmitCancel {
                        sub_account_id,
                        instrument,
                        order_id,
                    } => {
                        implementation
                            .on_cancel_order(sub_account_id, instrument, order_id)
                            .await
                    }
                    ConnectorRequest::TransmitAmend {
                        sub_account_id,
                        instrument,
                        order_id,
                        req,
                    } => {
                        implementation
                            .on_amend_order(sub_account_id, instrument, order_id, req)
                            .await
                    }
                    ConnectorRequest::ExecuteFetchOpenOrders {
                        sub_account_id,
                        instrument,
                    } => {
                        implementation
                            .on_fetch_open_orders(sub_account_id, instrument)
                            .await
                    }
                    ConnectorRequest::ExecuteFetchOrder {
                        sub_account_id,
                        instrument,
                        order_id,
                    } => {
                        implementation
                            .on_fetch_order(sub_account_id, instrument, order_id)
                            .await
                    }
                    ConnectorRequest::ExecuteFetchPositions {
                        sub_account_id,
                        instrument,
                    } => {
                        implementation
                            .on_fetch_positions(sub_account_id, instrument)
                            .await
                    }
                    ConnectorRequest::ExecuteFetchBalances { sub_account_id } => {
                        implementation.on_fetch_balances(sub_account_id).await
                    }
                    ConnectorRequest::TransmitBatchOrder {
                        sub_account_id,
                        instrument,
                        reqs,
                        atomic,
                    } => {
                        implementation
                            .on_batch_submit_order(sub_account_id, instrument, reqs, atomic)
                            .await
                    }
                    ConnectorRequest::TransmitBatchCancel {
                        sub_account_id,
                        instrument,
                        order_ids,
                        atomic,
                    } => {
                        implementation
                            .on_batch_cancel_order(sub_account_id, instrument, order_ids, atomic)
                            .await
                    }
                    ConnectorRequest::TransmitBatchAmend {
                        sub_account_id,
                        instrument,
                        amends,
                        atomic,
                    } => {
                        implementation
                            .on_batch_amend_order(sub_account_id, instrument, amends, atomic)
                            .await
                    }
                };

                if let Err(e) = res {
                    eprintln!("Connector request failed: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        self.implementation.on_disconnect().await?;
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
            rx: Some(self.channels.status_tx.subscribe()),
        })
    }
}

// --- Data Channels ---

impl<I, Impl> AccessChannel<OrderBookDomain> for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    type Id = I;
    type Sender = ();
    type Receiver = broadcast::Receiver<OrderBook<Impl::Asset>>;

    fn access_channel(
        &self,
        _domain: OrderBookDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        let tx = self.channels.get_orderbook_tx(&id);
        Ok(ChannelPair {
            tx: None,
            rx: Some(tx.subscribe()),
        })
    }
}

impl<I, Impl> AccessChannel<TradeFlowDomain> for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    type Id = I;
    type Sender = ();
    type Receiver = broadcast::Receiver<TradeFlow<Impl::Asset>>;

    fn access_channel(
        &self,
        _domain: TradeFlowDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        let tx = self.channels.get_trade_tx(&id);
        Ok(ChannelPair {
            tx: None,
            rx: Some(tx.subscribe()),
        })
    }
}

impl<I, Impl> AccessChannel<KlineDomain> for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    type Id = I;
    type Sender = ();
    type Receiver = broadcast::Receiver<KlineSeries<Impl::Asset>>;

    fn access_channel(
        &self,
        _domain: KlineDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        let tx = self.channels.get_kline_tx(&id);
        Ok(ChannelPair {
            tx: None,
            rx: Some(tx.subscribe()),
        })
    }
}

// --- Execution Channels ---

impl<I, Impl> AccessChannel<ConnectorControlDomain> for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    type Id = ();
    type Sender = mpsc::Sender<ConnectorRequest<I, Impl::SubAccount>>;
    type Receiver = ();

    fn access_channel(
        &self,
        _domain: ConnectorControlDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: Some(self.request_tx.clone()),
            rx: None,
        })
    }
}

impl<I, Impl> AccessChannel<OrderDomain> for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    type Id = ();
    type Sender = ();
    type Receiver = broadcast::Receiver<OrderUpdate<I, Impl::Asset, Impl::SubAccount>>;

    fn access_channel(
        &self,
        _domain: OrderDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: None,
            rx: Some(self.channels.order_update_tx.subscribe()),
        })
    }
}

impl<I, Impl> AccessChannel<PositionDomain> for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    type Id = ();
    type Sender = ();
    type Receiver = broadcast::Receiver<PositionUpdate<Impl::Asset, Impl::SubAccount>>;

    fn access_channel(
        &self,
        _domain: PositionDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: None,
            rx: Some(self.channels.position_update_tx.subscribe()),
        })
    }
}

impl<I, Impl> AccessChannel<BalanceDomain> for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    type Id = ();
    type Sender = ();
    type Receiver = broadcast::Receiver<BalanceUpdate<Impl::Asset, Impl::SubAccount>>;

    fn access_channel(
        &self,
        _domain: BalanceDomain,
        _id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        Ok(ChannelPair {
            tx: None,
            rx: Some(self.channels.balance_update_tx.subscribe()),
        })
    }
}

impl<I, Impl> ExchangeConnector<I, Impl::Asset, Impl::SubAccount> for StandardConnector<I, Impl>
where
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    Impl: ConnectorImpl<Instrument = I>,
{
    type Config = Impl;

    fn new(config: Self::Config) -> Self {
        Self::new(config)
    }

    fn channels(&self) -> ConnectorChannels<I, Impl::Asset, Impl::SubAccount> {
        self.channels.clone()
    }
}
