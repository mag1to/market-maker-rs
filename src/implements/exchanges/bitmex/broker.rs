use log::*;

use async_trait::async_trait;

use bitmex::rest::{BitMEXRest, DeleteOrderRequest, OrdType, PostOrderRequest, Side as RawSide};

use crate::apikey::ApiKey;
use crate::interfaces::Broker;
use crate::types::{CancelOrder, NewOrder, Order, OrderId, OrderResponse, OrderType, Side};

pub struct BitMEXBroker {
    bm: BitMEXRest,
}

impl BitMEXBroker {
    pub fn connect(apikey: &ApiKey) -> Self {
        std::env::set_var("BITMEX_TESTNET", "1");

        let bm = BitMEXRest::with_credential(apikey.key(), apikey.secret());

        Self { bm }
    }
}

#[async_trait]
impl Broker for BitMEXBroker {
    async fn submit(&self, order: Order) -> OrderResponse {
        match order {
            Order::New(new_order) => {
                let req = build_new_order_request(new_order);
                match self.bm.request(req).await {
                    Ok(response) => {
                        let id = OrderId::new(response.order_id);
                        OrderResponse::Accept(id)
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        OrderResponse::Reject
                    }
                }
            }
            Order::Cancel(cancel_order) => {
                let req = build_cancel_order_request(cancel_order);
                match self.bm.request(req).await {
                    Ok(response) => {
                        let id = OrderId::new(response.first().unwrap().order_id);
                        OrderResponse::Accept(id)
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        OrderResponse::Reject
                    }
                }
            }
        }
    }
}

pub fn build_new_order_request(order: NewOrder) -> PostOrderRequest {
    let price = order.price().try_into().unwrap();
    let order_qty = order.amount().try_into().unwrap();

    let side = match order.order_side() {
        Side::Ask => RawSide::Sell,
        Side::Bid => RawSide::Buy,
    };

    let ord_type = match order.order_type() {
        OrderType::Limit => OrdType::Limit,
        OrderType::Market => OrdType::Market,
    };

    PostOrderRequest {
        symbol: "XBTUSD".to_string(),
        side: Some(side),
        simple_order_qty: None,
        order_qty: Some(order_qty),
        price: Some(price),
        display_qty: None,
        stop_px: None,
        cl_ord_id: None,
        cl_ord_link_id: None,
        peg_offset_value: None,
        peg_price_type: None,
        ord_type: Some(ord_type),
        time_in_force: None,
        exec_inst: None,
        contingency_type: None,
        text: None,
    }
}

pub fn build_cancel_order_request(order: CancelOrder) -> DeleteOrderRequest {
    let order_id = order.id().to_string();
    DeleteOrderRequest {
        order_id: Some(order_id.into()),
        ..Default::default()
    }
}
