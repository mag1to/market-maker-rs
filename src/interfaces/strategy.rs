use crate::types::{Execution, Inventory, MarketInfo, OpenOrders, Order, Orderbook};

pub trait Policy {
    fn evaluate(&self, observation: impl Observation) -> Vec<Order>;
}

pub trait Observation {
    fn info(&self) -> &MarketInfo;
    fn executions(&self) -> &[Execution];
    fn orderbook(&self) -> &Orderbook;
    fn inventory(&self) -> &Inventory;
    fn open_orders(&self) -> &OpenOrders;
    fn pending_orders(&self) -> &[Order];
}

impl<'a, S> Observation for &'a S
where
    S: Observation,
{
    fn info(&self) -> &MarketInfo {
        (*self).info()
    }

    fn executions(&self) -> &[Execution] {
        (*self).executions()
    }

    fn orderbook(&self) -> &Orderbook {
        (*self).orderbook()
    }

    fn inventory(&self) -> &Inventory {
        (*self).inventory()
    }

    fn open_orders(&self) -> &OpenOrders {
        (*self).open_orders()
    }

    fn pending_orders(&self) -> &[Order] {
        (*self).pending_orders()
    }
}
