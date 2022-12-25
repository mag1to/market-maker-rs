use std::fmt;

use super::order::Side;
use super::values::{Amount, Price};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TradeId(String);

impl TradeId {
    pub fn new(id: impl ToString) -> Self {
        Self(id.to_string())
    }
}

impl fmt::Display for TradeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TradeId({})", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Execution {
    timestamp: u64,
    id: TradeId,
    maker_side: Side,
    price: Price,
    amount: Amount,
}

impl Execution {
    pub fn new(
        timestamp: u64,
        id: TradeId,
        maker_side: Side,
        price: Price,
        amount: Amount,
    ) -> Self {
        Self {
            timestamp,
            id,
            maker_side,
            price,
            amount,
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn id(&self) -> &TradeId {
        &self.id
    }

    pub fn maker_side(&self) -> Side {
        self.maker_side
    }

    pub fn taker_side(&self) -> Side {
        self.maker_side.opposite()
    }

    pub fn price(&self) -> Price {
        self.price
    }

    pub fn amount(&self) -> Amount {
        self.amount
    }
}
