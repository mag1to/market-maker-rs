use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use log::*;
use rust_decimal_macros::dec;
use std::thread;

use futures::sink::SinkExt;
use futures::stream::StreamExt;
use tokio::runtime::Runtime;

use bitmex::websocket::{BitMEXWebsocket, Command, Topic};

use super::parser::{self, ParsedMessage};
use crate::implements::writers::{OrderbookWriteOp, OrderbookWriter, OrderbookWriterResult};
use crate::interfaces::Market;
use crate::pubsub::{PubSub, Subscription};
use crate::types::{Execution, MarketInfo, Orderbook};

pub struct BitMEXMarket {
    _runtime: Runtime,
    _updater: Option<thread::JoinHandle<()>>,
    pubsub_orderbook: PubSub<Orderbook>,
    pubsub_execution: PubSub<Execution>,
}

impl Market for BitMEXMarket {
    fn info(&self) -> MarketInfo {
        MarketInfo {
            max_order_size: dec!(10000000),
            min_order_size: dec!(100),
            lot_size: dec!(100),
            max_order_price: dec!(1000000),
            min_order_price: dec!(1),
            tick_size: dec!(0.5),
        }
    }

    fn orderbook(&self) -> Subscription<Orderbook> {
        self.pubsub_orderbook.subscribe()
    }

    fn execution(&self) -> Subscription<Execution> {
        self.pubsub_execution.subscribe()
    }
}

impl BitMEXMarket {
    pub fn connect() -> Self {
        std::env::set_var("BITMEX_TESTNET", "1");

        let pubsub_orderbook = PubSub::new();
        let pubsub_execution = PubSub::new();

        let (sender, receiver) = unbounded();

        let runtime = Runtime::new().unwrap();
        runtime.spawn(start_websocket(sender));

        let updater = {
            let pubsub_orderbook = pubsub_orderbook.clone();
            let pubsub_execution = pubsub_execution.clone();
            thread::spawn(move || {
                let mut orderbook = receive_orderbook(&receiver).unwrap();
                pubsub_orderbook.publish(orderbook.clone());

                for parsed in receiver {
                    match parsed {
                        ParsedMessage::Orderbook(ops) => {
                            let mut writer = OrderbookWriter::new(&mut orderbook);
                            for op in ops {
                                writer.apply(op).unwrap();
                            }
                            pubsub_orderbook.publish(orderbook.clone());
                        }
                        ParsedMessage::Execution(executions) => {
                            for execution in executions {
                                // TODO: maybe too expensive
                                pubsub_execution.publish(execution);
                            }
                        }
                        _ => {}
                    }
                }
            })
        };

        Self {
            _runtime: runtime,
            _updater: Some(updater),
            pubsub_orderbook,
            pubsub_execution,
        }
    }
}

fn receive_orderbook(receiver: &Receiver<ParsedMessage>) -> OrderbookWriterResult<Orderbook> {
    loop {
        if let Some(ParsedMessage::Orderbook(messages)) = receiver.iter().next() {
            let mut iter = messages.into_iter();
            while let Some(message) = iter.next() {
                if let OrderbookWriteOp::Snapshot(mut orderbook) = message {
                    let mut writer = OrderbookWriter::new(&mut orderbook);
                    for op in iter {
                        writer.apply(op)?;
                    }
                    return Ok(orderbook);
                }
            }
        }
    }
}

async fn start_websocket(sender: Sender<ParsedMessage>) -> Result<()> {
    loop {
        let mut client = BitMEXWebsocket::new().await.unwrap();

        client
            .send(Command::Subscribe(vec![
                Topic::OrderBookL2(Some("XBTUSD".to_string())),
                Topic::Trade(Some("XBTUSD".to_string())),
            ]))
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
