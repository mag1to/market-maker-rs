use crossbeam_channel::{select, Receiver, RecvError};

use crate::interfaces::Observation as ObservationInterface;
use crate::types::{Execution, Inventory, MarketInfo, OpenOrders, Order, Orderbook};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    info: MarketInfo,
    executions: Vec<Execution>,
    orderbook: Orderbook,
    inventory: Inventory,
    open_orders: OpenOrders,
    pending_orders: Vec<Order>,
}

impl Observation {
    pub fn new(
        info: MarketInfo,
        executions: Vec<Execution>,
        orderbook: Orderbook,
        inventory: Inventory,
        open_orders: OpenOrders,
        pending_orders: Vec<Order>,
    ) -> Self {
        Self {
            info,
            executions,
            orderbook,
            inventory,
            open_orders,
            pending_orders,
        }
    }

    pub fn warmup(
        info: MarketInfo,
        execution_receiver: &Receiver<Execution>,
        orderbook_receiver: &Receiver<Orderbook>,
        inventory_receiver: &Receiver<Inventory>,
        open_orders_receiver: &Receiver<OpenOrders>,
    ) -> Result<Self, RecvError> {
        let mut executions = Vec::new();
        let mut orderbook = None;
        let mut inventory = None;
        let mut open_orders = None;

        loop {
            select! {
                recv(execution_receiver) -> msg => {
                    executions.push(msg?);
                },
                recv(orderbook_receiver) -> msg => {
                    orderbook = Some(msg?);
                },
                recv(inventory_receiver) -> msg => {
                    inventory = Some(msg?);
                },
                recv(open_orders_receiver) -> msg => {
                    open_orders = Some(msg?);
                },
            }

            if orderbook.is_some() && inventory.is_some() && open_orders.is_some() {
                break;
            }
        }

        let orderbook = orderbook.expect("must exists");
        let inventory = inventory.expect("must exists");
        let open_orders = open_orders.expect("must exists");

        Ok(Observation::new(
            info,
            executions,
            orderbook,
            inventory,
            open_orders,
            Vec::new(),
        ))
    }

    pub fn insert_execution(&mut self, execution: Execution) {
        self.executions.push(execution);
    }

    pub fn update_orderbook(&mut self, orderbook: Orderbook) {
        self.orderbook = orderbook;
    }

    pub fn update_inventory(&mut self, inventory: Inventory) {
        self.inventory = inventory;
    }

    pub fn update_open_orders(&mut self, open_orders: OpenOrders) {
        self.open_orders = open_orders;
    }

    pub fn update_pending_orders(&mut self, pending_orders: Vec<Order>) {
        self.pending_orders = pending_orders;
    }
}

impl ObservationInterface for Observation {
    fn info(&self) -> &MarketInfo {
        &self.info
    }

    fn executions(&self) -> &[Execution] {
        &self.executions
    }

    fn orderbook(&self) -> &Orderbook {
        &self.orderbook
    }

    fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    fn open_orders(&self) -> &OpenOrders {
        &self.open_orders
    }

    fn pending_orders(&self) -> &[Order] {
        &self.pending_orders
    }
}
