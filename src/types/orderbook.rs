use chrono::{TimeZone, Utc};
use rust_decimal_macros::dec;
use std::fmt;

use super::values::{Amount, Price};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OfferId(String);

impl OfferId {
    pub fn new(id: impl ToString) -> Self {
        Self(id.to_string())
    }
}

impl fmt::Display for OfferId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub(crate) id: OfferId,
    pub(crate) price: Price,
    pub(crate) amount: Amount,
}

impl Offer {
    pub fn new(id: OfferId, price: Price, amount: Amount) -> Self {
        Self { id, price, amount }
    }

    pub fn id(&self) -> &OfferId {
        &self.id
    }

    pub fn price(&self) -> Price {
        self.price
    }

    pub fn amount(&self) -> Amount {
        self.amount
    }

    pub fn into_inner(self) -> (OfferId, Price, Amount) {
        let Self { id, price, amount } = self;
        (id, price, amount)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Orderbook {
    pub(crate) timestamp: u64, // ms
    pub(crate) asks: Vec<Offer>,
    pub(crate) bids: Vec<Offer>,
}

impl Orderbook {
    pub fn new<A, B>(timestamp: u64, asks: A, bids: B) -> Self
    where
        A: IntoIterator<Item = Offer>,
        B: IntoIterator<Item = Offer>,
    {
        Self {
            timestamp,
            asks: asks.into_iter().collect(),
            bids: bids.into_iter().collect(),
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn asks(&self) -> impl Iterator<Item = &Offer> {
        self.asks.iter()
    }

    pub fn bids(&self) -> impl Iterator<Item = &Offer> {
        self.bids.iter()
    }

    pub fn best_ask(&self) -> Option<&Offer> {
        self.asks.first()
    }

    pub fn best_bid(&self) -> Option<&Offer> {
        self.bids.first()
    }

    pub fn best_ask_price(&self) -> Option<Price> {
        self.best_ask().map(|offer| offer.price())
    }

    pub fn best_bid_price(&self) -> Option<Price> {
        self.best_bid().map(|offer| offer.price())
    }

    pub fn mid_price(&self) -> Option<Price> {
        let ask_price = self.best_ask_price()?;
        let bid_price = self.best_bid_price()?;
        let mid_price = (ask_price + bid_price) / dec!(2);
        Some(mid_price)
    }
}

impl fmt::Display for Orderbook {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        const TAKE: usize = 9;
        const P: usize = 9;
        const I: usize = 9;
        const W: usize = P + I + 1;

        let date = Utc
            .timestamp_millis_opt(self.timestamp().try_into().unwrap())
            .unwrap();

        let id_width = self
            .asks()
            .chain(self.bids())
            .map(|o| o.id().to_string().len())
            .chain(vec![W])
            .max()
            .unwrap();

        writeln!(f, "Orderbook @ {}", date)?;
        writeln!(
            f,
            "  {id:>id_width$} {price:>W$} {amount:>W$} {total:>W$}",
            id = "Id",
            price = "Price",
            amount = "Amount",
            total = "Total",
        )?;

        let mut asum = dec!(0);
        let mut asks: Vec<_> = self
            .asks
            .iter()
            .take(TAKE)
            .map(|o| {
                asum += o.amount;
                (o.id.to_string(), o.price, o.amount, asum)
            })
            .collect();
        asks.reverse();
        for (id, price, amount, total) in asks {
            writeln!(
                f,
                "a {id:>id_width$} {price:>W$.P$} {amount:>W$.P$} {total:>W$.P$}",
            )?;
        }

        writeln!(f)?;

        let mut bsum = dec!(0);
        let bids = self.bids.iter().take(TAKE).map(|o| {
            bsum += o.amount;
            (o.id.to_string(), o.price, o.amount, bsum)
        });
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

    #[test]
    fn test_orderbook_string() {
        let orderbook = Orderbook::new(
            1671926400000,
            vec![
                Offer::new(OfferId::new(160000), dec!(16000.0), dec!(1000)),
                Offer::new(OfferId::new(170000), dec!(17000.0), dec!(1000)),
                Offer::new(OfferId::new(180000), dec!(18000.0), dec!(1000)),
                Offer::new(OfferId::new(190000), dec!(19000.0), dec!(1000)),
            ],
            vec![
                Offer::new(OfferId::new(140000), dec!(14000.0), dec!(1000)),
                Offer::new(OfferId::new(130000), dec!(13000.0), dec!(1000)),
                Offer::new(OfferId::new(120000), dec!(12000.0), dec!(1000)),
                Offer::new(OfferId::new(110000), dec!(11000.0), dec!(1000)),
            ],
        );

        println!("\n{}", orderbook);
    }
}
