use thiserror::Error;

use crate::types::{Amount, OpenOrders, OrderId, OrderState, Price, Side};

#[derive(Error, Debug, PartialEq, Eq)]
pub enum OpenOrdersWriterError {
    #[error("already exists: {0}")]
    AlreadyExists(OrderId),
    #[error("offer not found: {0}")]
    OrderNotFound(OrderId),
    #[error("insufficient amount")]
    InsufficientAmount,
}

pub type OpenOrdersWriterResult<T> = Result<T, OpenOrdersWriterError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenOrdersWriteOp {
    Snapshot(OpenOrders),
    Create(CreateOp),
    Update(UpdateOp),
    Delete(DeleteOp),
    Execution(ExecutionOp),
}

impl OpenOrdersWriteOp {
    pub fn init(orders: OpenOrders) -> Self {
        orders.into()
    }

    pub fn create(timestamp: u64, id: OrderId, side: Side, price: Price, amount: Amount) -> Self {
        CreateOp::new(timestamp, id, side, price, amount).into()
    }

    pub fn update(
        timestamp: u64,
        id: OrderId,
        side: impl Into<Option<Side>>,
        price: impl Into<Option<Price>>,
        amount: impl Into<Option<Amount>>,
    ) -> Self {
        UpdateOp::new(timestamp, id, side, price, amount).into()
    }

    pub fn delete(timestamp: u64, id: OrderId) -> Self {
        DeleteOp::new(timestamp, id).into()
    }

    pub fn execution(timestamp: u64, id: OrderId, amount: Amount) -> Self {
        ExecutionOp::new(timestamp, id, amount).into()
    }
}

impl From<OpenOrders> for OpenOrdersWriteOp {
    fn from(open_orders: OpenOrders) -> Self {
        Self::Snapshot(open_orders)
    }
}

impl From<CreateOp> for OpenOrdersWriteOp {
    fn from(op: CreateOp) -> Self {
        Self::Create(op)
    }
}

impl From<UpdateOp> for OpenOrdersWriteOp {
    fn from(op: UpdateOp) -> Self {
        Self::Update(op)
    }
}

impl From<DeleteOp> for OpenOrdersWriteOp {
    fn from(op: DeleteOp) -> Self {
        Self::Delete(op)
    }
}

impl From<ExecutionOp> for OpenOrdersWriteOp {
    fn from(op: ExecutionOp) -> Self {
        Self::Execution(op)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateOp {
    pub timestamp: u64,
    pub order: OrderState,
}

impl CreateOp {
    pub fn new(timestamp: u64, id: OrderId, side: Side, price: Price, amount: Amount) -> Self {
        let order = OrderState::new(id, side, price, amount);
        Self { timestamp, order }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateOp {
    pub timestamp: u64,
    pub id: OrderId,
    pub side: Option<Side>,
    pub price: Option<Price>,
    pub amount: Option<Amount>,
}

impl UpdateOp {
    pub fn new(
        timestamp: u64,
        id: OrderId,
        side: impl Into<Option<Side>>,
        price: impl Into<Option<Price>>,
        amount: impl Into<Option<Amount>>,
    ) -> Self {
        Self {
            timestamp,
            id,
            side: side.into(),
            price: price.into(),
            amount: amount.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteOp {
    pub timestamp: u64,
    pub id: OrderId,
}

impl DeleteOp {
    pub fn new(timestamp: u64, id: OrderId) -> Self {
        Self { timestamp, id }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionOp {
    pub timestamp: u64,
    pub id: OrderId,
    pub amount: Amount,
}

impl ExecutionOp {
    pub fn new(timestamp: u64, id: OrderId, amount: Amount) -> Self {
        Self {
            timestamp,
            id,
            amount,
        }
    }
}

pub struct OpenOrdersWriter<'a> {
    inner: &'a mut OpenOrders,
}

impl<'a> OpenOrdersWriter<'a> {
    pub fn new(inner: &'a mut OpenOrders) -> Self {
        Self { inner }
    }

    pub fn apply(&mut self, op: impl Into<OpenOrdersWriteOp>) -> OpenOrdersWriterResult<()> {
        match op.into() {
            OpenOrdersWriteOp::Snapshot(orders) => self.apply_snapshot(orders),
            OpenOrdersWriteOp::Create(op) => self.apply_create(op),
            OpenOrdersWriteOp::Update(op) => self.apply_update(op),
            OpenOrdersWriteOp::Delete(op) => self.apply_delete(op),
            OpenOrdersWriteOp::Execution(op) => self.apply_execution(op),
        }
    }

    pub fn apply_snapshot(&mut self, orders: OpenOrders) -> OpenOrdersWriterResult<()> {
        *self.inner = orders;
        Ok(())
    }

    pub fn apply_create(&mut self, op: CreateOp) -> OpenOrdersWriterResult<()> {
        let CreateOp { timestamp, order } = op;

        if self.inner.orders.iter().any(|o| o.id() == order.id()) {
            return Err(OpenOrdersWriterError::AlreadyExists(order.id));
        }

        self.inner.timestamp = timestamp;
        self.inner.orders.push(order);

        Ok(())
    }

    pub fn apply_update(&mut self, op: UpdateOp) -> OpenOrdersWriterResult<()> {
        let UpdateOp {
            timestamp,
            id,
            side,
            price,
            amount,
        } = op;

        if let Some(order) = self.inner.orders.iter_mut().find(|o| o.id() == &id) {
            if let Some(side) = side {
                order.side = side;
            }

            if let Some(price) = price {
                order.price = price;
            }

            if let Some(amount) = amount {
                order.amount = amount;
            }

            self.inner.timestamp = timestamp;

            Ok(())
        } else {
            Err(OpenOrdersWriterError::OrderNotFound(id))
        }
    }

    pub fn apply_delete(&mut self, op: DeleteOp) -> OpenOrdersWriterResult<()> {
        let DeleteOp { timestamp, id } = op;
        if let Some(index) = self.inner.orders.iter().position(|o| o.id() == &id) {
            self.inner.orders.remove(index);
            self.inner.timestamp = timestamp;
            Ok(())
        } else {
            Err(OpenOrdersWriterError::OrderNotFound(id))
        }
    }

    pub fn apply_execution(&mut self, op: ExecutionOp) -> OpenOrdersWriterResult<()> {
        let ExecutionOp {
            timestamp,
            id,
            amount: execused_amount,
        } = op;

        if let Some(index) = self.inner.orders.iter().position(|o| o.id() == &id) {
            let order = self.inner.orders.get_mut(index).unwrap();

            if order.amount < execused_amount {
                return Err(OpenOrdersWriterError::InsufficientAmount);
            }

            order.amount -= execused_amount;
            self.inner.timestamp = timestamp;

            if order.amount.is_zero() {
                self.inner.orders.remove(index);
            }

            Ok(())
        } else {
            Err(OpenOrdersWriterError::OrderNotFound(id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal_macros::dec;

    use crate::types::Side;

    fn dummy_open_orders() -> OpenOrders {
        OpenOrders::new(
            0,
            vec![
                OrderState::new(OrderId::new(260), Side::Ask, dec!(26000), dec!(10)),
                OrderState::new(OrderId::new(270), Side::Ask, dec!(27000), dec!(10)),
                OrderState::new(OrderId::new(240), Side::Bid, dec!(24000), dec!(10)),
                OrderState::new(OrderId::new(230), Side::Bid, dec!(23000), dec!(10)),
            ],
        )
    }

    #[test]
    fn test_open_orders_writer_create() {
        let mut orders = dummy_open_orders();
        let mut updater = OpenOrdersWriter::new(&mut orders);

        updater
            .apply(CreateOp::new(
                1,
                OrderId::new(300),
                Side::Ask,
                dec!(30000),
                dec!(10),
            ))
            .unwrap();

        updater
            .apply(CreateOp::new(
                2,
                OrderId::new(200),
                Side::Bid,
                dec!(20000),
                dec!(10),
            ))
            .unwrap();

        let result = updater.apply(CreateOp::new(
            3,
            OrderId::new(200),
            Side::Bid,
            dec!(20000),
            dec!(10),
        ));
        assert_eq!(
            result,
            Err(OpenOrdersWriterError::AlreadyExists(OrderId::new(200)))
        );

        assert_eq!(
            orders,
            OpenOrders::new(
                2,
                vec![
                    OrderState::new(OrderId::new(260), Side::Ask, dec!(26000), dec!(10)),
                    OrderState::new(OrderId::new(270), Side::Ask, dec!(27000), dec!(10)),
                    OrderState::new(OrderId::new(240), Side::Bid, dec!(24000), dec!(10)),
                    OrderState::new(OrderId::new(230), Side::Bid, dec!(23000), dec!(10)),
                    OrderState::new(OrderId::new(300), Side::Ask, dec!(30000), dec!(10)),
                    OrderState::new(OrderId::new(200), Side::Bid, dec!(20000), dec!(10)),
                ],
            )
        );
    }

    #[test]
    fn test_open_orders_writer_update() {
        let mut open_orders = dummy_open_orders();
        let mut updater = OpenOrdersWriter::new(&mut open_orders);

        updater
            .apply(UpdateOp::new(
                1,
                OrderId::new(260),
                Side::Ask,
                dec!(26001),
                dec!(20),
            ))
            .unwrap();

        updater
            .apply(UpdateOp::new(
                2,
                OrderId::new(240),
                Side::Bid,
                dec!(24001),
                dec!(30),
            ))
            .unwrap();

        let result = updater.apply(UpdateOp::new(
            3,
            OrderId::new(999),
            Side::Ask,
            dec!(27000),
            dec!(50),
        ));
        assert_eq!(
            result,
            Err(OpenOrdersWriterError::OrderNotFound(OrderId::new(999)))
        );

        assert_eq!(
            open_orders,
            OpenOrders::new(
                2,
                vec![
                    OrderState::new(OrderId::new(260), Side::Ask, dec!(26001), dec!(20)),
                    OrderState::new(OrderId::new(270), Side::Ask, dec!(27000), dec!(10)),
                    OrderState::new(OrderId::new(240), Side::Bid, dec!(24001), dec!(30)),
                    OrderState::new(OrderId::new(230), Side::Bid, dec!(23000), dec!(10)),
                ],
            )
        );
    }

    #[test]
    fn test_open_orders_writer_delete() {
        let mut open_orders = dummy_open_orders();
        let mut updater = OpenOrdersWriter::new(&mut open_orders);

        updater.apply(DeleteOp::new(1, OrderId::new(260))).unwrap();

        updater.apply(DeleteOp::new(2, OrderId::new(240))).unwrap();

        let result = updater.apply(DeleteOp::new(3, OrderId::new(999)));
        assert_eq!(
            result,
            Err(OpenOrdersWriterError::OrderNotFound(OrderId::new(999)))
        );

        assert_eq!(
            open_orders,
            OpenOrders::new(
                2,
                vec![
                    OrderState::new(OrderId::new(270), Side::Ask, dec!(27000), dec!(10)),
                    OrderState::new(OrderId::new(230), Side::Bid, dec!(23000), dec!(10)),
                ],
            )
        );
    }

    #[test]
    fn test_open_orders_writer_execution() {
        let mut open_orders = dummy_open_orders();
        let mut updater = OpenOrdersWriter::new(&mut open_orders);

        updater
            .apply(ExecutionOp::new(1, OrderId::new(260), dec!(6.5)))
            .unwrap();

        updater
            .apply(ExecutionOp::new(2, OrderId::new(240), dec!(10)))
            .unwrap();

        let result = updater.apply(ExecutionOp::new(3, OrderId::new(999), dec!(10)));
        assert_eq!(
            result,
            Err(OpenOrdersWriterError::OrderNotFound(OrderId::new(999)))
        );

        let result = updater.apply(ExecutionOp::new(4, OrderId::new(270), dec!(20)));
        assert_eq!(result, Err(OpenOrdersWriterError::InsufficientAmount));

        assert_eq!(
            open_orders,
            OpenOrders::new(
                2,
                vec![
                    OrderState::new(OrderId::new(260), Side::Ask, dec!(26000), dec!(3.5)),
                    OrderState::new(OrderId::new(270), Side::Ask, dec!(27000), dec!(10)),
                    OrderState::new(OrderId::new(230), Side::Bid, dec!(23000), dec!(10)),
                ],
            )
        );
    }
}
