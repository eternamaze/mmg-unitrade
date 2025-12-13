use crate::common::account_model::{AssetIdentity, SubAccountIdentity};
use crate::common::actor_trait::{AccessChannel, Actor, ChannelPair};
use crate::dataform::kline::KlineSeries;
use crate::dataform::orderbook::OrderBook;
use crate::dataform::tradeflow::TradeFlow;
use crate::exchange::exchange_connector_template::ExchangeConnector;
use crate::exchange::exchange_domain::{
    BalanceCommand, BalanceDomain, BalanceUpdate, ConnectorControlDomain, ConnectorRequest,
    FeedCommand, KlineDomain, OrderBookDomain, OrderCommand, OrderDomain, OrderUpdate,
    PositionCommand, PositionDomain, PositionUpdate, TradeFlowDomain,
};
use crate::exchange::exchange_feed_template::ExchangeFeed;
use crate::exchange::exchange_gateway_template::ExchangeGateway;
use async_trait::async_trait;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

/// 交易所三元组 (Exchange Triad)
///
/// 这是一个泛型结构体，用于组合 Connector, Feed, Gateway。
/// 它本身也是一个 Actor，负责管理这三个组件的生命周期。
/// 它是外部交互的唯一入口（Facade）。
///
/// 外部使用者通过 Exchange 获取所有业务域的信道，
/// Exchange 内部会将请求转发给 Feed 或 Gateway。
///
/// 泛型 `I` 代表 Instrument (标的) 的标识符类型。
pub struct Exchange<C, F, G, I, A, S> {
    /// 连接器是内部基础设施，不对外暴露。
    /// 外部只能通过 Feed 和 Gateway 与系统交互。
    connector: Option<Arc<C>>,
    feed: Option<F>,
    gateway: Option<G>,
    _marker: PhantomData<(I, A, S)>,
}

impl<C, F, G, I, A, S> Exchange<C, F, G, I, A, S>
where
    C: ExchangeConnector<I, A, S>
        + AccessChannel<
            ConnectorControlDomain,
            Id = (),
            Sender = mpsc::Sender<ConnectorRequest<I, S>>,
            Receiver = (),
        >,
    F: ExchangeFeed<A, S, Id = I>,
    G: ExchangeGateway<A, S, Id = I>,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq + std::fmt::Debug,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    pub fn new(connector: C, feed: F, gateway: G) -> Self {
        Self {
            connector: Some(Arc::new(connector)),
            feed: Some(feed),
            gateway: Some(gateway),
            _marker: PhantomData,
        }
    }

    /// 自动构建并连线
    ///
    /// 这是推荐的构建方式。它会自动创建 Connector，并将其注入到 Feed 和 Gateway 中。
    pub fn build(config: C::Config) -> Self {
        let connector = Arc::new(C::new(config));
        
        // 获取连接器控制信道
        let connector_tx = connector
            .access_channel(ConnectorControlDomain, ())
            .expect("Failed to access connector control channel")
            .tx
            .expect("Connector control channel TX is missing");

        let connector_channels = connector.channels();

        // 注入信道而非 Connector 本体
        let feed = F::new(connector_tx.clone(), connector_channels.clone());
        let gateway = G::new(connector_tx, connector_channels);

        Self {
            connector: Some(connector),
            feed: Some(feed),
            gateway: Some(gateway),
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<C, F, G, I, A, S> Actor for Exchange<C, F, G, I, A, S>
where
    C: ExchangeConnector<I, A, S>,
    F: ExchangeFeed<A, S, Id = I>,
    G: ExchangeGateway<A, S>,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    async fn start(&self) -> anyhow::Result<()> {
        if let Some(c) = &self.connector {
            c.start().await?;
        }
        if let Some(f) = &self.feed {
            f.start().await?;
        }
        if let Some(g) = &self.gateway {
            g.start().await?;
        }
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        // Stop in reverse order
        if let Some(g) = &self.gateway {
            g.stop().await?;
        }
        if let Some(f) = &self.feed {
            f.stop().await?;
        }
        if let Some(c) = &self.connector {
            c.stop().await?;
        }
        Ok(())
    }
}

// --- Feed Channel Forwarding ---

impl<C, F, G, I, A, S> AccessChannel<OrderBookDomain> for Exchange<C, F, G, I, A, S>
where
    C: ExchangeConnector<I, A, S>,
    F: ExchangeFeed<A, S, Id = I> + AccessChannel<OrderBookDomain, Id = I, Sender = mpsc::Sender<FeedCommand<I>>, Receiver = broadcast::Receiver<OrderBook<A>>>,
    G: ExchangeGateway<A, S>,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<OrderBook<A>>;

    fn access_channel(
        &self,
        domain: OrderBookDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        if let Some(feed) = &self.feed {
            feed.access_channel(domain, id)
        } else {
            Ok(ChannelPair { tx: None, rx: None })
        }
    }
}

impl<C, F, G, I, A, S> AccessChannel<TradeFlowDomain> for Exchange<C, F, G, I, A, S>
where
    C: ExchangeConnector<I, A, S>,
    F: ExchangeFeed<A, S, Id = I> + AccessChannel<TradeFlowDomain, Id = I, Sender = mpsc::Sender<FeedCommand<I>>, Receiver = broadcast::Receiver<TradeFlow<A>>>,
    G: ExchangeGateway<A, S>,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<TradeFlow<A>>;

    fn access_channel(
        &self,
        domain: TradeFlowDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        if let Some(feed) = &self.feed {
            feed.access_channel(domain, id)
        } else {
            Ok(ChannelPair { tx: None, rx: None })
        }
    }
}

impl<C, F, G, I, A, S> AccessChannel<KlineDomain> for Exchange<C, F, G, I, A, S>
where
    C: ExchangeConnector<I, A, S>,
    F: ExchangeFeed<A, S, Id = I> + AccessChannel<KlineDomain, Id = I, Sender = mpsc::Sender<FeedCommand<I>>, Receiver = broadcast::Receiver<KlineSeries<A>>>,
    G: ExchangeGateway<A, S>,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = I;
    type Sender = mpsc::Sender<FeedCommand<I>>;
    type Receiver = broadcast::Receiver<KlineSeries<A>>;

    fn access_channel(
        &self,
        domain: KlineDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        if let Some(feed) = &self.feed {
            feed.access_channel(domain, id)
        } else {
            Ok(ChannelPair { tx: None, rx: None })
        }
    }
}



// --- Gateway Channel Forwarding ---

impl<C, F, G, I, A, S> AccessChannel<OrderDomain> for Exchange<C, F, G, I, A, S>
where
    C: ExchangeConnector<I, A, S>,
    F: ExchangeFeed<A, S, Id = I>,
    G: ExchangeGateway<A, S, Id = I> + AccessChannel<OrderDomain, Id = S, Sender = mpsc::Sender<OrderCommand<I, S>>, Receiver = broadcast::Receiver<OrderUpdate<I, A, S>>>,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = S;
    type Sender = mpsc::Sender<OrderCommand<I, S>>;
    type Receiver = broadcast::Receiver<OrderUpdate<I, A, S>>;

    fn access_channel(
        &self,
        domain: OrderDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        if let Some(gateway) = &self.gateway {
            gateway.access_channel(domain, id)
        } else {
            Ok(ChannelPair { tx: None, rx: None })
        }
    }
}

impl<C, F, G, I, A, S> AccessChannel<PositionDomain> for Exchange<C, F, G, I, A, S>
where
    C: ExchangeConnector<I, A, S>,
    F: ExchangeFeed<A, S, Id = I>,
    G: ExchangeGateway<A, S, Id = I> + AccessChannel<PositionDomain, Id = S, Sender = mpsc::Sender<PositionCommand<I, S>>, Receiver = broadcast::Receiver<PositionUpdate<A, S>>>,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = S;
    type Sender = mpsc::Sender<PositionCommand<I, S>>;
    type Receiver = broadcast::Receiver<PositionUpdate<A, S>>;

    fn access_channel(
        &self,
        domain: PositionDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        if let Some(gateway) = &self.gateway {
            gateway.access_channel(domain, id)
        } else {
            Ok(ChannelPair { tx: None, rx: None })
        }
    }
}

impl<C, F, G, I, A, S> AccessChannel<BalanceDomain> for Exchange<C, F, G, I, A, S>
where
    C: ExchangeConnector<I, A, S>,
    F: ExchangeFeed<A, S, Id = I>,
    G: ExchangeGateway<A, S, Id = I> + AccessChannel<BalanceDomain, Id = S, Sender = mpsc::Sender<BalanceCommand<S>>, Receiver = broadcast::Receiver<BalanceUpdate<A, S>>>,
    I: Send + Sync + 'static + Clone + std::hash::Hash + Eq,
    A: AssetIdentity,
    S: SubAccountIdentity,
{
    type Id = S;
    type Sender = mpsc::Sender<BalanceCommand<S>>;
    type Receiver = broadcast::Receiver<BalanceUpdate<A, S>>;

    fn access_channel(
        &self,
        domain: BalanceDomain,
        id: Self::Id,
    ) -> anyhow::Result<ChannelPair<Self::Sender, Self::Receiver>> {
        if let Some(gateway) = &self.gateway {
            gateway.access_channel(domain, id)
        } else {
            Ok(ChannelPair { tx: None, rx: None })
        }
    }
}
