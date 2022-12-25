use anyhow::Result;
use crossbeam_channel::select;
use log::*;

use crate::components::order_service::OrderService;
use crate::interfaces::{Broker, Market, Observation as ObservationInterface, Policy, Status};
use crate::observation::Observation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub num_iteration: usize,
    pub test: bool, // no submission
}

pub struct Bot<M, S, B, P> {
    config: Config,
    market: M,
    status: S,
    policy: P,
    order_service: OrderService<B>,
}

impl<M, S, B, P> Bot<M, S, B, P>
where
    M: Market,
    S: Status,
    B: Broker + Send + Sync + 'static,
    P: Policy,
{
    pub fn new(config: Config, market: M, status: S, broker: B, policy: P) -> Self {
        let order_service = OrderService::start(broker);
        Self {
            config,
            market,
            status,
            policy,
            order_service,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        info!("Start running!");
        info!("\n{:#?}", self.config);

        let info = self.market.info();
        info!("\n{:#?}", info);

        let execution = self.market.execution();
        let orderbook = self.market.orderbook();
        let inventory = self.status.inventory();
        let open_orders = self.status.open_orders();

        info!("Warmingup observation..");
        let mut observation = Observation::warmup(
            info,
            execution.as_receiver(),
            orderbook.as_receiver(),
            inventory.as_receiver(),
            open_orders.as_receiver(),
        )?;

        for i in 0..self.config.num_iteration {
            let mut target = false;
            select! {
                recv(execution.as_receiver()) -> msg => {
                    info!("iteration[{i}] receive execution!");
                    observation.insert_execution(msg?);
                },
                recv(orderbook.as_receiver()) -> msg => {
                    info!("iteration[{i}] receive orderbook!");
                    observation.update_orderbook(msg?);
                    target = true;
                },
                recv(inventory.as_receiver()) -> msg => {
                    info!("iteration[{i}] receive inventory!");
                    observation.update_inventory(msg?);
                },
                recv(open_orders.as_receiver()) -> msg => {
                    info!("iteration[{i}] receive orders!");
                    observation.update_open_orders(msg?);
                },
            }

            if target {
                let pending_orders = self
                    .order_service
                    .get_pending_orders()
                    .into_iter()
                    .map(|po| po.into_inner())
                    .collect();
                observation.update_pending_orders(pending_orders);

                info!("orderbook:\n{}", observation.orderbook());
                info!("open_orders:\n{}", observation.open_orders());
                info!("inventory:\n{:?}", observation.inventory());
                info!("pending_orders:\n{:?}", observation.pending_orders());

                info!("iteration[{i}] evaluating..");
                let orders = self.policy.evaluate(&observation);
                info!("output:\n{:#?}", orders);

                if !orders.is_empty() && !self.config.test {
                    for order in orders {
                        self.order_service.submit(order);
                    }
                }
            }
        }

        Ok(())
    }
}
