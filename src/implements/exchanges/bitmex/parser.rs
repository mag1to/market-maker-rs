use chrono::{DateTime, Utc};
use log::*;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use bitmex::rest::{Position, Side as RawSide, Trade};
use bitmex::websocket::{Action, BitMEXWsMessage, TableMessage};

use crate::implements::writers::{OpenOrdersWriteOp, OrderbookWriteOp};
use crate::types::{
    Amount, Execution, Offer, OfferId, OpenOrders, OrderId, OrderState, Orderbook, Side, TradeId,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderBookL2 {
    pub timestamp: String,
    pub symbol: String,
    pub id: i64,
    pub side: RawSide,
    pub size: Option<i64>,
    pub price: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Order {
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "orderID")]
    pub order_id: String,
    #[serde(rename = "ordStatus")]
    pub ord_status: String,
    #[serde(rename = "orderQty")]
    pub order_qty: Option<i64>,
    pub price: Option<f64>,
    #[serde(rename = "leavesQty")]
    pub leaves_qty: Option<i64>,
    #[serde(rename = "cumQty")]
    pub cum_qty: Option<i64>,
    pub side: Option<RawSide>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParsedMessage {
    Orderbook(Vec<OrderbookWriteOp>),
    Execution(Vec<Execution>),
    OpenOrders(Vec<OpenOrdersWriteOp>),
    Position(Amount),
}

pub fn parse_message(message: &BitMEXWsMessage) -> Option<ParsedMessage> {
    match message {
        BitMEXWsMessage::Table(table) => match table.table.as_str() {
            "orderBookL2" => {
                let parsed = parse_orderbook_ops(table)?;
                Some(ParsedMessage::Orderbook(parsed))
            }
            "trade" => {
                let parsed = parse_executions(table)?;
                Some(ParsedMessage::Execution(parsed))
            }
            "order" => {
                let parsed = parse_open_orders_ops(table)?;
                Some(ParsedMessage::OpenOrders(parsed))
            }
            "position" => {
                let parsed = parse_position(table)?;
                Some(ParsedMessage::Position(parsed))
            }
            _ => None,
        },
        _ => None,
    }
}

pub fn parse_orderbook_ops(table: &TableMessage<Value>) -> Option<Vec<OrderbookWriteOp>> {
    let mut data = Vec::new();
    for v in table.data.clone() {
        let parsed: OrderBookL2 = serde_json::from_value(v).ok()?;
        data.push(parsed);
    }

    let dt: DateTime<Utc> = data.first()?.timestamp.parse().unwrap();
    let timestamp: u64 = dt.timestamp_millis().try_into().unwrap();

    let mut ops = Vec::new();
    match table.action {
        Action::Partial => {
            let mut asks = Vec::new();
            let mut bids = Vec::new();
            for o in data {
                let side = match o.side {
                    RawSide::Buy => Side::Bid,
                    RawSide::Sell => Side::Ask,
                    _ => return None,
                };

                let price = o.price.unwrap();
                let size = o.size.unwrap();

                let offer = Offer::new(
                    OfferId::new(o.id),
                    Decimal::from_f64(price).unwrap(),
                    Decimal::from_i64(size).unwrap(),
                );

                match side {
                    Side::Ask => asks.push(offer),
                    Side::Bid => bids.push(offer),
                }
            }

            asks.sort_by_key(|offer| offer.price());
            bids.sort_by_key(|offer| -offer.price());

            let orderbook = Orderbook::new(timestamp, asks, bids);
            ops.push(OrderbookWriteOp::Snapshot(orderbook));
        }
        Action::Insert => {
            for o in data {
                let side = match o.side {
                    RawSide::Buy => Side::Bid,
                    RawSide::Sell => Side::Ask,
                    _ => return None,
                };

                let price = o.price.unwrap();
                let size = o.size.unwrap();

                ops.push(OrderbookWriteOp::create(
                    timestamp,
                    side,
                    OfferId::new(o.id),
                    Decimal::from_f64(price).unwrap(),
                    Decimal::from_i64(size).unwrap(),
                ));
            }
        }
        Action::Update => {
            for o in data {
                let side = match o.side {
                    RawSide::Buy => Side::Bid,
                    RawSide::Sell => Side::Ask,
                    _ => return None,
                };

                let price = o.price.map(|p| Decimal::from_f64(p).unwrap());
                let amount = o.size.map(|s| Decimal::from_i64(s).unwrap());

                ops.push(OrderbookWriteOp::update(
                    timestamp,
                    side,
                    OfferId::new(o.id),
                    price,
                    amount,
                ));
            }
        }
        Action::Delete => {
            for o in data {
                let side = match o.side {
                    RawSide::Buy => Side::Bid,
                    RawSide::Sell => Side::Ask,
                    _ => return None,
                };
                ops.push(OrderbookWriteOp::delete(
                    timestamp,
                    side,
                    OfferId::new(o.id),
                ));
            }
        }
    }

    Some(ops)
}

pub fn parse_executions(table: &TableMessage<Value>) -> Option<Vec<Execution>> {
    let mut executions = Vec::new();
    for v in table.data.clone() {
        let parsed: Trade = serde_json::from_value(v).ok()?;

        let maker_side = match parsed.side? {
            RawSide::Buy => Side::Ask,
            RawSide::Sell => Side::Bid,
            _ => return None,
        };

        let timestamp: u64 = parsed.timestamp.timestamp_millis().try_into().unwrap();
        let id = TradeId::new(parsed.trd_match_id?);
        let price = Decimal::from_f64(parsed.price?)?;
        let amount = Decimal::from_i64(parsed.size?)?;

        executions.push(Execution::new(timestamp, id, maker_side, price, amount));
    }

    Some(executions)
}

pub fn parse_open_orders_ops(table: &TableMessage<Value>) -> Option<Vec<OpenOrdersWriteOp>> {
    debug!("{:#?}", table);
    let mut ops = Vec::new();
    match table.action {
        Action::Partial => {
            let mut latest = 0;
            let mut orders = Vec::new();
            for v in table.data.clone() {
                let parsed: Order = serde_json::from_value(v).ok()?;
                let timestamp: u64 = parsed.timestamp.timestamp_millis().try_into().unwrap();
                if timestamp > latest {
                    latest = timestamp;
                }

                orders.push(parse_order_state(parsed)?);
            }

            ops.push(OpenOrdersWriteOp::init(OpenOrders::new(latest, orders)));
        }
        Action::Update | Action::Insert => {
            for v in table.data.clone() {
                let parsed: Order = serde_json::from_value(v).ok()?;
                let timestamp: u64 = parsed.timestamp.timestamp_millis().try_into().unwrap();
                match parsed.ord_status.as_ref() {
                    "New" => {
                        let OrderState {
                            id,
                            price,
                            amount,
                            side,
                        } = parse_order_state(parsed).unwrap();
                        ops.push(OpenOrdersWriteOp::create(
                            timestamp, id, side, price, amount,
                        ));
                    }
                    "Canceled" | "Filled" => {
                        let id = OrderId::new(parsed.order_id);
                        ops.push(OpenOrdersWriteOp::delete(timestamp, id));
                    }
                    "PartiallyFilled" => match table.action {
                        Action::Insert => {
                            let OrderState {
                                id,
                                price,
                                amount,
                                side,
                            } = parse_order_state(parsed).unwrap();
                            ops.push(OpenOrdersWriteOp::create(
                                timestamp, id, side, price, amount,
                            ));
                        }
                        Action::Update => {
                            let id = OrderId::new(parsed.order_id);
                            let amount = Decimal::from_i64(parsed.leaves_qty.unwrap()).unwrap();
                            ops.push(OpenOrdersWriteOp::update(timestamp, id, None, None, amount));
                        }
                        _ => {
                            unreachable!()
                        }
                    },
                    _ => {}
                }
            }
        }
        _ => return None,
    }

    Some(ops)
}

pub fn parse_order_state(order: Order) -> Option<OrderState> {
    let id = OrderId::new(order.order_id);
    let price = Decimal::from_f64(order.price?)?;
    let amount = Decimal::from_i64(order.leaves_qty?)?;
    let side = match order.side? {
        RawSide::Buy => Side::Bid,
        RawSide::Sell => Side::Ask,
        _ => return None,
    };
    Some(OrderState::new(id, side, price, amount))
}

pub fn parse_position(table: &TableMessage<Value>) -> Option<Amount> {
    let mut values = Vec::new();
    match table.action {
        Action::Partial | Action::Update => {
            for v in table.data.clone() {
                let parsed: Position = serde_json::from_value(v).ok()?;
                let _timestamp: u64 = parsed.timestamp?.timestamp_millis().try_into().unwrap();
                values.push(Amount::from_i64(parsed.current_qty?)?);
            }
        }
        _ => return None,
    }
    values.last().copied()
}
