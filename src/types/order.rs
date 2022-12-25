use chrono::{TimeZone, Utc};
use rust_decimal_macros::dec;
use std::fmt;

use super::values::{Amount, Price};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OrderId(String);

impl OrderId {
    pub fn new(id: impl ToString) -> Self {
        Self(id.to_string())
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Side {
    Ask,
    Bid,
}

impl Side {
    pub fn opposite(&self) -> Self {
        match self {
            Self::Ask => Self::Bid,
            Self::Bid => Self::Ask,
        }
    }

    pub fn is_ask(&self) -> bool {
        matches!(self, Self::Ask)
    }

    pub fn is_bid(&self) -> bool {
        matches!(self, Self::Bid)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Order {
    New(NewOrder),
    Cancel(CancelOrder),
}

impl Order {
    pub fn create(order_type: OrderType, order_side: Side, price: Price, amount: Amount) -> Self {
        NewOrder::new(order_type, order_side, price, amount).into()
    }

    pub fn cancel(id: OrderId) -> Self {
        CancelOrder::new(id).into()
    }
}

impl From<NewOrder> for Order {
    fn from(order: NewOrder) -> Self {
        Self::New(order)
    }
}

impl From<CancelOrder> for Order {
    fn from(order: CancelOrder) -> Self {
        Self::Cancel(order)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewOrder {
    order_type: OrderType,
    order_side: Side,
    price: Price,
    amount: Amount,
}

impl NewOrder {
    pub fn new(order_type: OrderType, order_side: Side, price: Price, amount: Amount) -> Self {
        Self {
            order_type,
            order_side,
            price,
            amount,
        }
    }

    pub fn order_side(&self) -> Side {
        self.order_side
    }

    pub fn order_type(&self) -> OrderType {
        self.order_type
    }

    pub fn price(&self) -> Price {
        self.price
    }

    pub fn amount(&self) -> Amount {
        self.amount
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateOrder {
    id: OrderId,
    new_order: NewOrder,
}

impl UpdateOrder {
    pub fn new(id: OrderId, new_order: NewOrder) -> Self {
        Self { id, new_order }
    }

    pub fn id(&self) -> &OrderId {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CancelOrder {
    id: OrderId,
}

impl CancelOrder {
    pub fn new(id: OrderId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &OrderId {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenOrders {
    pub(crate) timestamp: u64,
    pub(crate) orders: Vec<OrderState>,
}

impl OpenOrders {
    pub fn new<I>(timestamp: u64, orders: I) -> Self
    where
        I: IntoIterator<Item = OrderState>,
    {
        Self {
            timestamp,
            orders: orders.into_iter().collect(),
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn orders(&self) -> impl Iterator<Item = &OrderState> {
        self.orders.iter()
    }

    pub fn asks(&self) -> impl Iterator<Item = &OrderState> {
        self.orders().filter(|os| os.side().is_ask())
    }

    pub fn bids(&self) -> impl Iterator<Item = &OrderState> {
        self.orders().filter(|os| os.side().is_bid())
    }

    pub fn ask_amount(&self) -> Amount {
        self.asks().map(|os| os.amount()).sum()
    }

    pub fn bid_amount(&self) -> Amount {
        self.bids().map(|os| os.amount()).sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderState {
    pub(crate) id: OrderId,
    pub(crate) side: Side,
    pub(crate) price: Price,
    pub(crate) amount: Amount,
}

impl OrderState {
    pub fn new(id: OrderId, side: Side, price: Price, amount: Amount) -> Self {
        Self {
            id,
            side,
            price,
            amount,
        }
    }

    pub fn id(&self) -> &OrderId {
        &self.id
    }

    pub fn side(&self) -> Side {
        self.side
    }

    pub fn price(&self) -> Price {
        self.price
    }

    pub fn amount(&self) -> Amount {
        self.amount
    }

    pub fn to_update_order(&self, new_order: NewOrder) -> UpdateOrder {
        UpdateOrder::new(self.id.clone(), new_order)
    }

    pub fn to_cancel_order(&self) -> CancelOrder {
        CancelOrder::new(self.id.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderResponse {
    Accept(OrderId),
    Reject,
}

impl fmt::Display for OpenOrders {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        const P: usize = 9;
        const I: usize = 9;
        const W: usize = P + I + 1;

        let date = Utc
            .timestamp_millis_opt(self.timestamp().try_into().unwrap())
            .unwrap();

        let id_width = self
            .orders()
            .map(|o| o.id().to_string().len())
            .chain(vec![W])
            .max()
            .unwrap();

        writeln!(f, "OpenOrders @ {}", date)?;
        writeln!(
            f,
            "  {id:>id_width$} {price:>W$} {amount:>W$} {total:>W$}",
            id = "Id",
            price = "Price",
            amount = "Amount",
            total = "Total",
        )?;

        let mut asks = self
            .asks()
            .map(|o| (o.id.to_string(), o.price, o.amount))
            .collect::<Vec<_>>();
        asks.sort_by_key(|(_, p, _)| *p);

        let mut asum = dec!(0);
        let mut asks = asks
            .into_iter()
            .map(|(id, price, amount)| {
                asum += amount;
                (id, price, amount, asum)
            })
            .collect::<Vec<_>>();
        asks.reverse();
        for (id, price, amount, total) in &asks {
            writeln!(
                f,
                "a {id:>id_width$} {price:>W$.P$} {amount:>W$.P$} {total:>W$.P$}",
            )?;
        }

        let mut bids = self
            .bids()
            .map(|o| (o.id.to_string(), o.price, o.amount))
            .collect::<Vec<_>>();
        bids.sort_by_key(|(_, p, _)| -(*p));

        if !asks.is_empty() && !bids.is_empty() {
            writeln!(f)?;
        }

        let mut bsum = dec!(0);
        let bids = bids
            .into_iter()
            .map(|(id, price, amount)| {
                bsum += amount;
                (id, price, amount, bsum)
            })
            .collect::<Vec<_>>();
        for (id, price, amount, total) in bids {
            writeln!(
                f,
                "b {id:>id_width$} {price:>W$.P$} {amount:>W$.P$} {total:>W$.P$}",
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal_macros::dec;

    #[test]
    fn test_open_orders_string() {
        let open_orders = OpenOrders::new(
            1671926400000,
            vec![
                OrderState::new(OrderId::new(180000000), Side::Ask, dec!(18000.0), dec!(200)),
                OrderState::new(OrderId::new(170000000), Side::Ask, dec!(17000.0), dec!(300)),
                OrderState::new(OrderId::new(160000000), Side::Ask, dec!(16000.0), dec!(100)),
                OrderState::new(OrderId::new(140000000), Side::Bid, dec!(14000.0), dec!(500)),
                OrderState::new(OrderId::new(130000000), Side::Bid, dec!(13000.0), dec!(500)),
                OrderState::new(OrderId::new(120000000), Side::Bid, dec!(12000.0), dec!(500)),
            ],
        );

        println!("{}", open_orders);
    }
}
