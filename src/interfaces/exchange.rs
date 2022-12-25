use async_trait::async_trait;

use crate::pubsub::Subscription;
use crate::types::{Execution, Inventory, MarketInfo, OpenOrders, Order, OrderResponse, Orderbook};

pub trait Market {
    fn info(&self) -> MarketInfo;
    fn orderbook(&self) -> Subscription<Orderbook>;
    fn execution(&self) -> Subscription<Execution>;
}

pub trait Status {
    fn inventory(&self) -> Subscription<Inventory>;
    fn open_orders(&self) -> Subscription<OpenOrders>;
}

#[async_trait]
pub trait Broker {
    async fn submit(&self, order: Order) -> OrderResponse;
}
