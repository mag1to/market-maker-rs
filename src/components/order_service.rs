use chrono::Utc;
use log::*;
use std::sync::{Arc, RwLock};

use tokio::runtime::Runtime;
use tokio::time::Duration;

use crate::interfaces::Broker;
use crate::types::Order;

const EXPIRES_MS: u64 = 20_000;
const GC_TICK_MS: u64 = 1_000;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingId(u64);

impl From<u64> for PendingId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<PendingId> for u64 {
    fn from(id: PendingId) -> Self {
        id.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingOrder {
    timestamp: u64,
    id: PendingId,
    order: Order,
}

impl PendingOrder {
    pub fn new(timestamp: u64, id: PendingId, order: Order) -> Self {
        Self {
            timestamp,
            id,
            order,
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn id(&self) -> PendingId {
        self.id
    }

    pub fn inner(&self) -> &Order {
        &self.order
    }

    pub fn into_inner(self) -> Order {
        let Self { order, .. } = self;
        order
    }
}

pub struct OrderService<B> {
    nonce: u64,
    broker: Arc<B>,
    pendings: Arc<RwLock<Vec<PendingOrder>>>,
    rt: Runtime,
}

impl<B> OrderService<B>
where
    B: Broker + Send + Sync + 'static,
{
    pub fn start(broker: B) -> Self {
        let broker = Arc::new(broker);
        let pendings = Arc::new(RwLock::new(Vec::<PendingOrder>::new()));

        // start gc-like cleanup task
        let rt = Runtime::new().unwrap();
        rt.spawn({
            let pendings = pendings.clone();
            async move {
                let mut interval = tokio::time::interval(Duration::from_millis(GC_TICK_MS));
                loop {
                    interval.tick().await;
                    {
                        let mut guard = pendings.write().unwrap();
                        let now: u64 = Utc::now().timestamp_millis().try_into().unwrap();

                        let prev = guard.len();
                        guard.retain(|po| po.timestamp() + EXPIRES_MS > now);

                        debug!("gc: {} -> {}", prev, guard.len());
                    }
                }
            }
        });

        Self {
            nonce: 0,
            broker,
            pendings,
            rt,
        }
    }

    pub fn submit(&mut self, order: Order) {
        let timestamp: u64 = Utc::now().timestamp_millis().try_into().unwrap();
        let id = PendingId(self.nonce);
        let pending_order = PendingOrder::new(timestamp, id, order.clone());
        self.nonce += 1;

        self.rt.spawn({
            let broker = self.broker.clone();
            let pendings = self.pendings.clone();
            async move {
                let id = pending_order.id();
                {
                    let mut guard = pendings.write().unwrap();
                    guard.push(pending_order);
                }

                debug!("{id:?} send: {order:?}");
                let response = broker.submit(order).await;
                debug!("{id:?} recv: {response:?}");

                {
                    let mut guard = pendings.write().unwrap();
                    guard.retain(|po| po.id() != id);
                }
            }
        });
    }

    pub fn get_pending_orders(&self) -> Vec<PendingOrder> {
        let guard = self.pendings.read().unwrap();
        (*guard).clone()
    }
}
