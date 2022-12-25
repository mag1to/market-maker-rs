use thiserror::Error;

use crate::types::{Amount, Offer, OfferId, Orderbook, Price, Side};

#[derive(Error, Debug)]
pub enum UpdateOrderbookError {
    #[error("already exists: {0}")]
    AlreadyExists(OfferId),
    #[error("offer not found: {0}")]
    OfferNotFound(OfferId),
}

pub type OrderbookWriterResult<T> = Result<T, UpdateOrderbookError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderbookWriteOp {
    Snapshot(Orderbook),
    Create(CreateOp),
    Update(UpdateOp),
    Delete(DeleteOp),
}

impl OrderbookWriteOp {
    pub fn init(orderbook: Orderbook) -> Self {
        orderbook.into()
    }

    pub fn create(timestamp: u64, side: Side, id: OfferId, price: Price, amount: Amount) -> Self {
        CreateOp::new(timestamp, side, id, price, amount).into()
    }

    pub fn update(
        timestamp: u64,
        side: Side,
        id: OfferId,
        price: impl Into<Option<Price>>,
        amount: impl Into<Option<Amount>>,
    ) -> Self {
        UpdateOp::new(timestamp, side, id, price, amount).into()
    }

    pub fn delete(timestamp: u64, side: Side, id: OfferId) -> Self {
        DeleteOp::new(timestamp, side, id).into()
    }
}

impl From<Orderbook> for OrderbookWriteOp {
    fn from(data: Orderbook) -> Self {
        Self::Snapshot(data)
    }
}

impl From<CreateOp> for OrderbookWriteOp {
    fn from(op: CreateOp) -> Self {
        Self::Create(op)
    }
}

impl From<UpdateOp> for OrderbookWriteOp {
    fn from(op: UpdateOp) -> Self {
        Self::Update(op)
    }
}

impl From<DeleteOp> for OrderbookWriteOp {
    fn from(op: DeleteOp) -> Self {
        Self::Delete(op)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateOp {
    pub timestamp: u64,
    pub side: Side,
    pub id: OfferId,
    pub price: Price,
    pub amount: Amount,
}

impl CreateOp {
    pub fn new(timestamp: u64, side: Side, id: OfferId, price: Price, amount: Amount) -> Self {
        Self {
            timestamp,
            side,
            id,
            price,
            amount,
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn side(&self) -> Side {
        self.side
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

    pub fn to_delete(&self) -> DeleteOp {
        let Self {
            timestamp,
            side,
            id,
            ..
        } = self;
        DeleteOp::new(*timestamp, *side, id.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateOp {
    pub timestamp: u64,
    pub side: Side,
    pub id: OfferId,
    pub price: Option<Price>,
    pub amount: Option<Amount>,
}

impl UpdateOp {
    pub fn new(
        timestamp: u64,
        side: Side,
        id: OfferId,
        price: impl Into<Option<Price>>,
        amount: impl Into<Option<Amount>>,
    ) -> Self {
        Self {
            timestamp,
            side,
            id,
            price: price.into(),
            amount: amount.into(),
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn side(&self) -> Side {
        self.side
    }

    pub fn id(&self) -> &OfferId {
        &self.id
    }

    pub fn price(&self) -> Option<Price> {
        self.price
    }

    pub fn amount(&self) -> Option<Amount> {
        self.amount
    }

    pub fn to_delete(&self) -> DeleteOp {
        let Self {
            timestamp,
            side,
            id,
            ..
        } = self;
        DeleteOp::new(*timestamp, *side, id.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteOp {
    pub timestamp: u64,
    pub side: Side,
    pub id: OfferId,
}

impl DeleteOp {
    pub fn new(timestamp: u64, side: Side, id: OfferId) -> Self {
        Self {
            timestamp,
            side,
            id,
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn side(&self) -> Side {
        self.side
    }

    pub fn id(&self) -> &OfferId {
        &self.id
    }
}

impl From<CreateOp> for Offer {
    fn from(op: CreateOp) -> Self {
        let CreateOp {
            id, price, amount, ..
        } = op;
        Self::new(id, price, amount)
    }
}

pub struct OrderbookWriter<'a> {
    inner: &'a mut Orderbook,
}

impl<'a> OrderbookWriter<'a> {
    pub fn new(inner: &'a mut Orderbook) -> Self {
        Self { inner }
    }

    pub fn apply(&mut self, op: impl Into<OrderbookWriteOp>) -> OrderbookWriterResult<()> {
        match op.into() {
            OrderbookWriteOp::Snapshot(orderbook) => self.apply_snapshot(orderbook),
            OrderbookWriteOp::Create(op) => self.apply_create(op),
            OrderbookWriteOp::Update(op) => self.apply_update(op),
            OrderbookWriteOp::Delete(op) => self.apply_delete(op),
        }
    }

    pub fn apply_snapshot(&mut self, orderbook: Orderbook) -> OrderbookWriterResult<()> {
        *self.inner = orderbook;
        Ok(())
    }

    pub fn apply_create(&mut self, op: CreateOp) -> OrderbookWriterResult<()> {
        let timestamp = op.timestamp();

        match op.side() {
            Side::Ask => {
                let asks = &mut self.inner.asks;
                let index = asks
                    .iter()
                    .position(|offer| offer.price() > op.price())
                    .unwrap_or(asks.len());
                asks.insert(index, op.into());
            }
            Side::Bid => {
                let bids = &mut self.inner.bids;
                let index = bids
                    .iter()
                    .position(|offer| offer.price() < op.price())
                    .unwrap_or(bids.len());
                bids.insert(index, op.into());
            }
        }

        self.inner.timestamp = timestamp;

        Ok(())
    }

    pub fn apply_update(&mut self, op: UpdateOp) -> OrderbookWriterResult<()> {
        let timestamp = op.timestamp();

        let book = match op.side() {
            Side::Ask => &mut self.inner.asks,
            Side::Bid => &mut self.inner.bids,
        };

        if let Some(index) = book.iter().position(|offer| offer.id() == op.id()) {
            let mut offer = book.remove(index);

            if let Some(price) = op.price() {
                offer.price = price;
            }

            if let Some(amount) = op.amount() {
                offer.amount = amount;
            }

            let create_op = CreateOp::new(
                timestamp,
                op.side(),
                op.id().clone(),
                offer.price(),
                offer.amount(),
            );

            self.apply(create_op)?;
        } else {
            return Err(UpdateOrderbookError::OfferNotFound(op.id().clone()));
        }

        self.inner.timestamp = timestamp;

        Ok(())
    }

    pub fn apply_delete(&mut self, op: DeleteOp) -> OrderbookWriterResult<()> {
        let timestamp = op.timestamp();

        let book = match op.side() {
            Side::Ask => &mut self.inner.asks,
            Side::Bid => &mut self.inner.bids,
        };

        if let Some(index) = book.iter().position(|offer| offer.id() == op.id()) {
            book.remove(index);
        } else {
            return Err(UpdateOrderbookError::OfferNotFound(op.id().clone()));
        }

        self.inner.timestamp = timestamp;

        Ok(())
    }

    pub fn into_inner(self) -> &'a mut Orderbook {
        let Self { inner } = self;
        inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal_macros::dec;

    fn dummy_orderbook() -> Orderbook {
        Orderbook::new(
            0,
            vec![
                Offer::new(OfferId::new(260), dec!(26000), dec!(10)),
                Offer::new(OfferId::new(270), dec!(27000), dec!(10)),
                Offer::new(OfferId::new(280), dec!(28000), dec!(10)),
                Offer::new(OfferId::new(290), dec!(29000), dec!(10)),
            ],
            vec![
                Offer::new(OfferId::new(240), dec!(24000), dec!(10)),
                Offer::new(OfferId::new(230), dec!(23000), dec!(10)),
                Offer::new(OfferId::new(220), dec!(22000), dec!(10)),
                Offer::new(OfferId::new(210), dec!(21000), dec!(10)),
            ],
        )
    }

    #[test]
    fn test_orderbook_writer_create() {
        let mut orderbook = dummy_orderbook();
        let mut updater = OrderbookWriter::new(&mut orderbook);

        updater
            .apply(CreateOp::new(
                1,
                Side::Ask,
                OfferId::new(255),
                dec!(25500),
                dec!(10),
            ))
            .unwrap();

        updater
            .apply(CreateOp::new(
                2,
                Side::Ask,
                OfferId::new(275),
                dec!(27500),
                dec!(10),
            ))
            .unwrap();

        updater
            .apply(CreateOp::new(
                3,
                Side::Ask,
                OfferId::new(295),
                dec!(29500),
                dec!(10),
            ))
            .unwrap();

        updater
            .apply(CreateOp::new(
                4,
                Side::Bid,
                OfferId::new(245),
                dec!(24500),
                dec!(10),
            ))
            .unwrap();

        updater
            .apply(CreateOp::new(
                5,
                Side::Bid,
                OfferId::new(225),
                dec!(22500),
                dec!(10),
            ))
            .unwrap();

        updater
            .apply(CreateOp::new(
                6,
                Side::Bid,
                OfferId::new(205),
                dec!(20500),
                dec!(10),
            ))
            .unwrap();

        assert_eq!(
            orderbook,
            Orderbook::new(
                6,
                vec![
                    Offer::new(OfferId::new(255), dec!(25500), dec!(10)),
                    Offer::new(OfferId::new(260), dec!(26000), dec!(10)),
                    Offer::new(OfferId::new(270), dec!(27000), dec!(10)),
                    Offer::new(OfferId::new(275), dec!(27500), dec!(10)),
                    Offer::new(OfferId::new(280), dec!(28000), dec!(10)),
                    Offer::new(OfferId::new(290), dec!(29000), dec!(10)),
                    Offer::new(OfferId::new(295), dec!(29500), dec!(10)),
                ],
                vec![
                    Offer::new(OfferId::new(245), dec!(24500), dec!(10)),
                    Offer::new(OfferId::new(240), dec!(24000), dec!(10)),
                    Offer::new(OfferId::new(230), dec!(23000), dec!(10)),
                    Offer::new(OfferId::new(225), dec!(22500), dec!(10)),
                    Offer::new(OfferId::new(220), dec!(22000), dec!(10)),
                    Offer::new(OfferId::new(210), dec!(21000), dec!(10)),
                    Offer::new(OfferId::new(205), dec!(20500), dec!(10)),
                ],
            )
        );
    }

    #[test]
    fn test_orderbook_writer_update() {
        let mut orderbook = dummy_orderbook();
        let mut updater = OrderbookWriter::new(&mut orderbook);

        updater
            .apply(UpdateOp::new(
                1,
                Side::Ask,
                OfferId::new(260),
                Some(dec!(29500)),
                Some(dec!(20)),
            ))
            .unwrap();

        updater
            .apply(UpdateOp::new(
                2,
                Side::Ask,
                OfferId::new(290),
                Some(dec!(25500)),
                Some(dec!(20)),
            ))
            .unwrap();

        updater
            .apply(UpdateOp::new(
                3,
                Side::Bid,
                OfferId::new(240),
                Some(dec!(20500)),
                Some(dec!(20)),
            ))
            .unwrap();

        updater
            .apply(UpdateOp::new(
                4,
                Side::Bid,
                OfferId::new(210),
                Some(dec!(24500)),
                Some(dec!(20)),
            ))
            .unwrap();

        assert_eq!(
            orderbook,
            Orderbook::new(
                4,
                vec![
                    Offer::new(OfferId::new(290), dec!(25500), dec!(20)),
                    Offer::new(OfferId::new(270), dec!(27000), dec!(10)),
                    Offer::new(OfferId::new(280), dec!(28000), dec!(10)),
                    Offer::new(OfferId::new(260), dec!(29500), dec!(20)),
                ],
                vec![
                    Offer::new(OfferId::new(210), dec!(24500), dec!(20)),
                    Offer::new(OfferId::new(230), dec!(23000), dec!(10)),
                    Offer::new(OfferId::new(220), dec!(22000), dec!(10)),
                    Offer::new(OfferId::new(240), dec!(20500), dec!(20)),
                ],
            )
        );
    }

    #[test]
    fn test_orderbook_writer_delete() {
        let mut orderbook = dummy_orderbook();
        let mut updater = OrderbookWriter::new(&mut orderbook);

        updater
            .apply(DeleteOp::new(1, Side::Ask, OfferId::new(260)))
            .unwrap();

        updater
            .apply(DeleteOp::new(2, Side::Ask, OfferId::new(290)))
            .unwrap();

        updater
            .apply(DeleteOp::new(3, Side::Bid, OfferId::new(240)))
            .unwrap();

        updater
            .apply(DeleteOp::new(4, Side::Bid, OfferId::new(210)))
            .unwrap();

        assert_eq!(
            orderbook,
            Orderbook::new(
                4,
                vec![
                    Offer::new(OfferId::new(270), dec!(27000), dec!(10)),
                    Offer::new(OfferId::new(280), dec!(28000), dec!(10)),
                ],
                vec![
                    Offer::new(OfferId::new(230), dec!(23000), dec!(10)),
                    Offer::new(OfferId::new(220), dec!(22000), dec!(10)),
                ],
            )
        );
    }
}
