use anyhow::Result;
use chrono::{Duration, Utc};
use crossbeam_channel::{unbounded, Receiver, Sender};
use log::*;
use std::thread;

use futures::sink::SinkExt;
use futures::stream::StreamExt;
use tokio::runtime::Runtime;

use bitmex::websocket::{BitMEXWebsocket, Command, Topic};

use super::parser::{self, ParsedMessage};
use crate::apikey::ApiKey;
use crate::implements::writers::{OpenOrdersWriteOp, OpenOrdersWriter, OpenOrdersWriterResult};
use crate::interfaces::Status;
use crate::pubsub::{PubSub, Subscription};
use crate::types::{Inventory, OpenOrders};

pub struct BitMEXStatus {
    _runtime: Runtime,
    _updater: Option<thread::JoinHandle<()>>,
    pubsub_inventory: PubSub<Inventory>,
    pubsub_open_orders: PubSub<OpenOrders>,
}

impl Status for BitMEXStatus {
    fn inventory(&self) -> Subscription<Inventory> {
        self.pubsub_inventory.subscribe()
    }

    fn open_orders(&self) -> Subscription<OpenOrders> {
        self.pubsub_open_orders.subscribe()
    }
}

impl BitMEXStatus {
    pub fn connect(apikey: &ApiKey) -> Self {
        std::env::set_var("BITMEX_TESTNET", "1");

        let pubsub_inventory = PubSub::new();
        let pubsub_open_orders = PubSub::new();

        let (sender, receiver) = unbounded();

        let runtime = Runtime::new().unwrap();
        runtime.spawn(start_websocket(
            sender,
            apikey.key().to_string(),
            apikey.secret().to_string(),
        ));

        let updater = {
            let pubsub_inventory = pubsub_inventory.clone();
            let pubsub_open_orders = pubsub_open_orders.clone();
            thread::spawn(move || {
                let mut open_orders = receive_open_orders(&receiver).unwrap();
                pubsub_open_orders.publish(open_orders.clone());

                for parsed in receiver {
                    match parsed {
                        ParsedMessage::OpenOrders(ops) => {
                            let mut writer = OpenOrdersWriter::new(&mut open_orders);
                            for op in ops {
                                if let Err(e) = writer.apply(op) {
                                    error!("{:?}", e);
                                }
                            }
                            pubsub_open_orders.publish(open_orders.clone());
                        }
                        ParsedMessage::Position(position) => {
                            let inventory = Inventory::Position(position);
                            pubsub_inventory.publish(inventory);
                        }
                        _ => {
                            error!("unexpected message");
                        }
                    }
                }
            })
        };

        Self {
            _runtime: runtime,
            _updater: Some(updater),
            pubsub_inventory,
            pubsub_open_orders,
        }
    }
}

async fn start_websocket(
    sender: Sender<ParsedMessage>,
    api_key: String,
    api_secret: String,
) -> Result<()> {
    loop {
        let mut client = BitMEXWebsocket::with_credential(&api_key, &api_secret)
            .await
            .unwrap();

        let expires = (Utc::now() + Duration::seconds(1_000_000_000))
            .timestamp()
            .try_into()
            .unwrap();
        client.send(Command::authenticate(expires)).await.unwrap();

        client
            .send(Command::Subscribe(vec![Topic::Order, Topic::Position]))
            .await
            .unwrap();

        while let Some(result) = client.next().await {
            match result {
                Ok(message) => {
                    if let Some(parsed) = parser::parse_message(&message) {
                        sender.send(parsed).unwrap();
                    } else {
                        debug!("parse failed: {:?}", message);
                    }
                }
                Err(e) => {
                    error!("{:?}", e);
                }
            }
        }
    }
}

fn receive_open_orders(receiver: &Receiver<ParsedMessage>) -> OpenOrdersWriterResult<OpenOrders> {
    loop {
        if let Some(ParsedMessage::OpenOrders(ops)) = receiver.iter().next() {
            let mut iter = ops.into_iter();
            while let Some(op) = iter.next() {
                if let OpenOrdersWriteOp::Snapshot(mut open_orders) = op {
                    let mut writer = OpenOrdersWriter::new(&mut open_orders);
                    for op in iter {
                        writer.apply(op)?;
                    }
                    return Ok(open_orders);
                }
            }
        }
    }
}
