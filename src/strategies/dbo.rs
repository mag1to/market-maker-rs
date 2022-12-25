use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::collections::HashMap;

use crate::interfaces::{Observation, Policy};
use crate::types::*;

#[derive(Debug)]
pub struct DepthBasedOffering {
    max_exposure: Amount,
    target_depth: Amount,
}

impl DepthBasedOffering {
    pub fn new(max_exposure: Amount, target_depth: Amount) -> Self {
        Self {
            max_exposure,
            target_depth,
        }
    }

    pub fn max_exposure(&self) -> Amount {
        self.max_exposure
    }

    pub fn target_depth(&self) -> Amount {
        self.target_depth
    }
}

impl Policy for DepthBasedOffering {
    fn evaluate(&self, observation: impl Observation) -> Vec<Order> {
        if !observation.pending_orders().is_empty() {
            return Vec::new();
        }

        let mut orders = Vec::new();

        let info = observation.info();
        let orderbook = observation.orderbook();
        let inventory = observation.inventory();

        // compute new order prices
        let new_ask_price = find_price_at_depth(
            orderbook.asks(),
            self.target_depth,
            observation.open_orders(),
        )
        .map(|price| price - info.tick_size())
        .unwrap_or_else(|| info.max_order_price());
        let new_bid_price = find_price_at_depth(
            orderbook.bids(),
            self.target_depth,
            observation.open_orders(),
        )
        .map(|price| price + info.tick_size())
        .unwrap_or_else(|| info.min_order_price());

        // compute new order sizes
        let position: Amount = inventory.position();
        let new_ask_size = self.max_exposure() + position;
        let new_bid_size = self.max_exposure() - position;

        let mut ask_remaining: Amount = new_ask_size;
        for order in observation.open_orders().asks() {
            if order.price() == new_ask_price && order.amount() <= ask_remaining {
                ask_remaining -= order.amount();
            } else {
                orders.push(order.to_cancel_order().into());
            }
        }

        if ask_remaining >= info.min_order_size() {
            orders.push(Order::create(
                OrderType::Limit,
                Side::Ask,
                new_ask_price,
                ask_remaining,
            ));
        }

        let mut bid_remaining: Amount = new_bid_size;
        for order in observation.open_orders().bids() {
            if order.price() == new_bid_price && order.amount() <= bid_remaining {
                bid_remaining -= order.amount();
            } else {
                orders.push(order.to_cancel_order().into());
            }
        }

        if bid_remaining >= info.min_order_size() {
            orders.push(Order::create(
                OrderType::Limit,
                Side::Bid,
                new_bid_price,
                bid_remaining,
            ));
        }

        orders
    }
}

struct RemainingOrders {
    amounts: HashMap<Price, Amount>,
}

impl RemainingOrders {
    fn new(open_orders: &OpenOrders) -> Self {
        let mut amounts = HashMap::new();
        for order in open_orders.orders() {
            if let Some(amount) = amounts.get_mut(&order.price()) {
                *amount += order.amount();
            } else {
                amounts.insert(order.price(), order.amount());
            }
        }
        Self { amounts }
    }

    fn extract(&mut self, offer: &Offer) -> Amount {
        if let Some(amount) = self.amounts.get_mut(&offer.price()) {
            if offer.amount() > *amount {
                let ignored = *amount;
                *amount = Amount::zero();
                ignored
            } else {
                *amount -= offer.amount();
                offer.amount()
            }
        } else {
            Amount::zero()
        }
    }
}

fn find_price_at_depth<'a>(
    book: impl Iterator<Item = &'a Offer>,
    depth: Amount,
    open_orders: &OpenOrders,
) -> Option<Price> {
    let mut remaining = RemainingOrders::new(open_orders);
    let mut sum: Amount = dec!(0);
    for offer in book {
        let amount = offer.amount() - remaining.extract(offer);
        sum += amount;
        if sum >= depth {
            return Some(offer.price());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal_macros::dec;

    use crate::observation::Observation;

    fn dummy_info() -> MarketInfo {
        MarketInfo {
            max_order_size: dec!(10000000),
            min_order_size: dec!(100),
            lot_size: dec!(100),
            max_order_price: dec!(1000000),
            min_order_price: dec!(1),
            tick_size: dec!(0.5),
        }
    }

    fn dummy_observation() -> Observation {
        dummy_observation_with(dec!(0), vec![])
    }

    fn dummy_observation_with(position: Price, orders: Vec<OrderState>) -> Observation {
        Observation::new(
            dummy_info(),
            vec![],
            Orderbook::new(
                0,
                vec![
                    Offer::new(OfferId::new(160000), dec!(16000.0), dec!(1000)),
                    Offer::new(OfferId::new(170000), dec!(17000.0), dec!(1000)),
                ],
                vec![
                    Offer::new(OfferId::new(140000), dec!(14000.0), dec!(1000)),
                    Offer::new(OfferId::new(130000), dec!(13000.0), dec!(1000)),
                ],
            ),
            Inventory::Position(position),
            OpenOrders::new(0, orders),
            vec![],
        )
    }

    #[test]
    fn test_dbo_depth() {
        let observation = dummy_observation();

        // `target_depth` hits best offers
        let policy = DepthBasedOffering::new(dec!(500), dec!(1000));
        assert_eq!(
            policy.evaluate(&observation),
            vec![
                Order::create(OrderType::Limit, Side::Ask, dec!(15999.5), dec!(500)),
                Order::create(OrderType::Limit, Side::Bid, dec!(14000.5), dec!(500)),
            ],
        );

        // `target_depth` hits secondary best offers
        let policy = DepthBasedOffering::new(dec!(500), dec!(1001));
        assert_eq!(
            policy.evaluate(&observation),
            vec![
                Order::create(OrderType::Limit, Side::Ask, dec!(16999.5), dec!(500)),
                Order::create(OrderType::Limit, Side::Bid, dec!(13000.5), dec!(500)),
            ],
        );

        // overflow
        let policy = DepthBasedOffering::new(dec!(500), dec!(2001));
        assert_eq!(
            policy.evaluate(&observation),
            vec![
                Order::create(OrderType::Limit, Side::Ask, dec!(1000000), dec!(500)),
                Order::create(OrderType::Limit, Side::Bid, dec!(1), dec!(500)),
            ],
        );
    }

    #[test]
    fn test_dbo_position() {
        let policy = DepthBasedOffering::new(dec!(500), dec!(1000));

        // positive position
        let observation = dummy_observation_with(dec!(200), vec![]);
        assert_eq!(
            policy.evaluate(&observation),
            vec![
                Order::create(OrderType::Limit, Side::Ask, dec!(15999.5), dec!(700)),
                Order::create(OrderType::Limit, Side::Bid, dec!(14000.5), dec!(300)),
            ],
        );

        // negative position
        let observation = dummy_observation_with(dec!(-200), vec![]);
        assert_eq!(
            policy.evaluate(&observation),
            vec![
                Order::create(OrderType::Limit, Side::Ask, dec!(15999.5), dec!(300)),
                Order::create(OrderType::Limit, Side::Bid, dec!(14000.5), dec!(700)),
            ],
        );

        // positive position max
        let observation = dummy_observation_with(dec!(500), vec![]);
        assert_eq!(
            policy.evaluate(&observation),
            vec![Order::create(
                OrderType::Limit,
                Side::Ask,
                dec!(15999.5),
                dec!(1000)
            ),],
        );

        // negative position min
        let observation = dummy_observation_with(dec!(-500), vec![]);
        assert_eq!(
            policy.evaluate(&observation),
            vec![Order::create(
                OrderType::Limit,
                Side::Bid,
                dec!(14000.5),
                dec!(1000)
            ),],
        );

        // positive position overflow
        let observation = dummy_observation_with(dec!(600), vec![]);
        assert_eq!(
            policy.evaluate(&observation),
            vec![Order::create(
                OrderType::Limit,
                Side::Ask,
                dec!(15999.5),
                dec!(1100)
            ),],
        );

        // negative position overflow
        let observation = dummy_observation_with(dec!(-600), vec![]);
        assert_eq!(
            policy.evaluate(&observation),
            vec![Order::create(
                OrderType::Limit,
                Side::Bid,
                dec!(14000.5),
                dec!(1100)
            ),],
        );
    }

    #[test]
    fn test_dbo_orders() {
        let policy = DepthBasedOffering::new(dec!(500), dec!(1000));

        // already placed
        let observation = dummy_observation_with(
            dec!(0),
            vec![
                OrderState::new(OrderId::new(159995), Side::Ask, dec!(15999.5), dec!(500)),
                OrderState::new(OrderId::new(140005), Side::Bid, dec!(14000.5), dec!(500)),
            ],
        );
        assert_eq!(policy.evaluate(&observation), vec![]);

        // already placed (partial)
        let observation = dummy_observation_with(
            dec!(0),
            vec![
                OrderState::new(OrderId::new(159995), Side::Ask, dec!(15999.5), dec!(300)),
                OrderState::new(OrderId::new(140005), Side::Bid, dec!(14000.5), dec!(300)),
            ],
        );
        assert_eq!(
            policy.evaluate(&observation),
            vec![
                Order::create(OrderType::Limit, Side::Ask, dec!(15999.5), dec!(200)),
                Order::create(OrderType::Limit, Side::Bid, dec!(14000.5), dec!(200)),
            ],
        );

        // already placed (too large)
        let observation = dummy_observation_with(
            dec!(0),
            vec![
                OrderState::new(OrderId::new(159995), Side::Ask, dec!(15999.5), dec!(600)),
                OrderState::new(OrderId::new(140005), Side::Bid, dec!(14000.5), dec!(600)),
            ],
        );
        assert_eq!(
            policy.evaluate(&observation),
            vec![
                Order::cancel(OrderId::new(159995)),
                Order::create(OrderType::Limit, Side::Ask, dec!(15999.5), dec!(500)),
                Order::cancel(OrderId::new(140005)),
                Order::create(OrderType::Limit, Side::Bid, dec!(14000.5), dec!(500)),
            ],
        );

        // already placed (ignoring our open orders)
        let policy = DepthBasedOffering::new(dec!(1000), dec!(1000));
        let observation = Observation::new(
            dummy_info(),
            vec![],
            Orderbook::new(
                0,
                vec![
                    Offer::new(OfferId::new(159995), dec!(15999.5), dec!(1000)),
                    Offer::new(OfferId::new(160000), dec!(16000.0), dec!(1000)),
                    Offer::new(OfferId::new(170000), dec!(17000.0), dec!(1000)),
                ],
                vec![
                    Offer::new(OfferId::new(140005), dec!(14000.5), dec!(1000)),
                    Offer::new(OfferId::new(140000), dec!(14000.0), dec!(1000)),
                    Offer::new(OfferId::new(130000), dec!(13000.0), dec!(1000)),
                ],
            ),
            Inventory::Position(dec!(0)),
            OpenOrders::new(
                0,
                vec![
                    OrderState::new(OrderId::new(159995), Side::Ask, dec!(15999.5), dec!(1000)),
                    OrderState::new(OrderId::new(140005), Side::Bid, dec!(14000.5), dec!(1000)),
                ],
            ),
            vec![],
        );
        assert_eq!(policy.evaluate(&observation), vec![]);
    }
}
