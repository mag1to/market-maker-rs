use rust_decimal::prelude::*;

use super::values::{Amount, Price};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarketInfo {
    pub max_order_size: Amount,
    pub min_order_size: Amount,
    pub lot_size: Amount,
    pub max_order_price: Price,
    pub min_order_price: Price,
    pub tick_size: Decimal,
}

impl MarketInfo {
    pub fn max_order_size(&self) -> Amount {
        self.max_order_size
    }

    pub fn min_order_size(&self) -> Amount {
        self.min_order_size
    }

    pub fn lot_size(&self) -> Amount {
        self.lot_size
    }

    pub fn max_order_price(&self) -> Price {
        self.max_order_price
    }

    pub fn min_order_price(&self) -> Price {
        self.min_order_price
    }

    pub fn tick_size(&self) -> Decimal {
        self.tick_size
    }
}
