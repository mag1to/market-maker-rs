use super::values::Amount;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inventory {
    Position(Amount),
    Balances(Balances),
}

impl Inventory {
    pub fn position(&self) -> Amount {
        match self {
            Self::Position(position) => *position,
            Self::Balances(balances) => balances.base_amount(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Balances {
    ba: Amount,
    qa: Amount,
}

impl Balances {
    pub fn new(ba: Amount, qa: Amount) -> Self {
        Self { ba, qa }
    }

    pub fn base_amount(&self) -> Amount {
        self.ba
    }

    pub fn quote_amount(&self) -> Amount {
        self.qa
    }
}
